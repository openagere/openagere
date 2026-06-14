use crate::ChatOptions;
use crate::ToolDefinition;
use crate::sse::process_chat_sse;
use crate::translate::request::build_chat_messages_with_system;
use crate::translate::tools::to_chat_tools;
use crate::types::ChatRequest;
use crate::types::StreamOptions;
use agere_api::ApiError;
use agere_api::Provider;
use agere_api::ResponseEvent;
use agere_api::ResponseStream;
use agere_api::SharedAuthProvider;
use agere_client::HttpTransport;
use agere_client::RequestBody;
use agere_client::RequestCompression;
use agere_client::TransportError;
use agere_protocol::config_types::ReasoningSummary;
use agere_protocol::models::ResponseItem;
use agere_protocol::openai_models::ReasoningEffort;
use http::HeaderValue;
use http::Method;
use tokio::sync::mpsc;
use tracing::debug;

/// Client for the OpenAI Chat Completions API.
///
/// Takes internal protocol types (ResponseInputItem, ToolDefinition, etc.)
/// and orchestrates: translation -> authentication -> HTTP request -> SSE parsing.
pub struct ChatCompletionsClient<T: HttpTransport> {
    transport: T,
    provider: Provider,
    auth: SharedAuthProvider,
}

impl<T: HttpTransport> ChatCompletionsClient<T> {
    pub fn new(transport: T, provider: Provider, auth: SharedAuthProvider) -> Self {
        Self {
            transport,
            provider,
            auth,
        }
    }

    /// Send a streaming request to the Chat Completions API.
    pub async fn stream_request(
        &self,
        model: &str,
        system: &str,
        messages: &[ResponseItem],
        tools: &[ToolDefinition],
        reasoning_effort: Option<ReasoningEffort>,
        reasoning_summary: ReasoningSummary,
        options: ChatOptions,
    ) -> Result<ResponseStream, ApiError> {
        use agere_api::Compression;

        debug!(
            input_item_count = messages.len(),
            "Chat Completions: processing {} input items",
            messages.len()
        );
        for (i, item) in messages.iter().enumerate() {
            match item {
                ResponseItem::Message { role, content, .. } => {
                    let content_preview = if content.is_empty() {
                        "EMPTY_VEC"
                    } else {
                        match &content[0] {
                            agere_protocol::models::ContentItem::InputText { text }
                            | agere_protocol::models::ContentItem::OutputText { text } => {
                                // Use char boundary safe truncation to avoid panic
                                // when byte 120 lands inside a multi-byte UTF-8 char.
                                truncate_utf8(text, 120)
                            }
                            _ => "(non-text)",
                        }
                    };
                    debug!(
                        "Chat input[{}] role={} type=Message content={}",
                        i, role, content_preview
                    );
                }
                ResponseItem::FunctionCall { call_id, name, .. } => {
                    debug!(
                        "Chat input[{}] type=FunctionCall name={} call_id={}",
                        i, name, call_id
                    );
                }
                ResponseItem::FunctionCallOutput { call_id, .. } => {
                    debug!(
                        "Chat input[{}] type=FunctionCallOutput call_id={}",
                        i, call_id
                    );
                }
                ResponseItem::Reasoning { .. } => {
                    debug!("Chat input[{}] type=Reasoning", i);
                }
                other => {
                    debug!("Chat input[{}] type={:?}", i, std::mem::discriminant(other));
                }
            }
        }

        let chat_messages = build_chat_messages_with_system(system, messages);
        let chat_tools = to_chat_tools(tools);

        // Convert public ChatToolChoice to internal crate type
        let chat_tool_choice = options.tool_choice.as_ref().map(|tc| match tc {
            crate::ChatToolChoice::None => crate::types::ChatToolChoice::None,
            crate::ChatToolChoice::Auto => crate::types::ChatToolChoice::Auto,
            crate::ChatToolChoice::Required => crate::types::ChatToolChoice::Required,
            crate::ChatToolChoice::Function { name } => crate::types::ChatToolChoice::Function {
                function: crate::types::ChatFunctionChoiceName { name: name.clone() },
            },
        });

        let reasoning_str = reasoning_effort.map(|e| match e {
            ReasoningEffort::Low => "low".to_string(),
            ReasoningEffort::Medium => "medium".to_string(),
            ReasoningEffort::High => "high".to_string(),
            ReasoningEffort::None => "none".to_string(),
            ReasoningEffort::Minimal => "minimal".to_string(),
            ReasoningEffort::XHigh => "xhigh".to_string(),
            ReasoningEffort::Max => "max".to_string(),
        });
        let response_format = options.output_schema.clone().map(|schema| {
            crate::types::ChatResponseFormat::JsonSchema {
                json_schema: crate::types::ChatJsonSchemaFormat {
                    name: "agere_output_schema".to_string(),
                    schema,
                    strict: options.output_schema_strict,
                },
            }
        });

        let tool_count = chat_tools.len();
        let request_body = ChatRequest {
            model: model.to_string(),
            messages: chat_messages,
            tools: chat_tools,
            tool_choice: chat_tool_choice,
            parallel_tool_calls: options.parallel_tool_calls,
            temperature: options.temperature,
            top_p: options.top_p,
            max_tokens: Some(options.max_tokens),
            reasoning_effort: reasoning_str,
            response_format,
            stream: true,
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
        };

        let body_json = serde_json::to_value(&request_body)
            .map_err(|e| ApiError::Stream(format!("encode error: {e}")))?;

        // Log message details for debugging upstream API errors
        for (i, msg) in request_body.messages.iter().enumerate() {
            let content_preview = match &msg.content {
                None => "null".to_string(),
                Some(super::types::ChatContent::Text(t)) => {
                    if t.len() > 120 {
                        format!("text:{}...", truncate_utf8(t, 120))
                    } else {
                        format!("text:{t}")
                    }
                }
                Some(super::types::ChatContent::Blocks(b)) => format!("blocks({})", b.len()),
            };
            debug!(
                "Chat message[{}] role={} content={}",
                i, msg.role, content_preview
            );
        }

        debug!(
            model = %request_body.model,
            msg_count = request_body.messages.len(),
            tool_count = tool_count,
            stream = request_body.stream,
            "Chat Completions request built"
        );

        let mut headers = options.extra_headers;
        headers.insert(
            http::header::ACCEPT,
            HeaderValue::from_static("text/event-stream"),
        );
        headers.insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        let compression = match options.compression {
            Compression::None => RequestCompression::None,
            Compression::Zstd => RequestCompression::Zstd,
        };

        let make_req = || {
            let mut req = self
                .provider
                .build_request(Method::POST, chat_path(&self.provider));
            req.headers.extend(headers.clone());
            req.body = Some(RequestBody::Json(body_json.clone()));
            req.compression = compression;
            req
        };

        let retry_policy = self.provider.retry.to_policy();
        let auth = self.auth.clone();
        let transport = &self.transport;

        let stream_response =
            agere_client::run_with_retry(retry_policy, make_req, |req, _attempt| {
                let auth = auth.clone();
                async move {
                    let req = auth.apply_auth(req).await.map_err(TransportError::from)?;
                    transport.stream(req).await
                }
            })
            .await?;

        let (tx_event, rx_event) = mpsc::channel::<Result<ResponseEvent, ApiError>>(1600);

        // Chat Completions has no protocol-level reasoning summary stream. Raw
        // `delta.reasoning` is handled as reasoning content, not synthesized into
        // user-visible summaries; OpenAI Responses owns true summary support.
        let _ = reasoning_summary;
        tokio::spawn(process_chat_sse(
            stream_response.bytes,
            tx_event,
            self.provider.stream_idle_timeout,
        ));

        Ok(ResponseStream {
            rx_event,
            upstream_request_id: None,
        })
    }
}

fn chat_path(provider: &Provider) -> &'static str {
    if provider
        .base_url
        .trim_end_matches('/')
        .to_ascii_lowercase()
        .ends_with("/v1")
    {
        "chat/completions"
    } else {
        "v1/chat/completions"
    }
}

/// Truncate a UTF-8 string to at most `max_bytes` bytes, ensuring
/// the truncation never splits a multi-byte character (safe for &str slicing).
fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    // Find the last character that completely fits within max_bytes
    let boundary = s
        .char_indices()
        .take_while(|(i, ch)| i + ch.len_utf8() <= max_bytes)
        .last()
        .map(|(i, ch)| i + ch.len_utf8())
        .unwrap_or(0);
    &s[..boundary]
}

#[cfg(test)]
mod tests {
    use super::*;
    use agere_api::Compression;
    use agere_api::ReqwestTransport;
    use agere_api::RetryConfig;
    use agere_protocol::models::ContentItem;
    use agere_protocol::models::ResponseItem;
    use futures::StreamExt;
    use http::HeaderMap;

    fn test_provider(base_url: String) -> Provider {
        Provider {
            name: "chat".into(),
            base_url,
            query_params: None,
            headers: HeaderMap::new(),
            retry: RetryConfig {
                max_attempts: 1,
                base_delay: std::time::Duration::from_millis(10),
                retry_429: false,
                retry_5xx: false,
                retry_transport: false,
            },
            stream_idle_timeout: std::time::Duration::from_secs(5),
        }
    }

    fn test_auth() -> SharedAuthProvider {
        use std::sync::Arc;
        struct NoAuth;
        impl agere_api::AuthProvider for NoAuth {
            fn add_auth_headers(&self, _headers: &mut http::HeaderMap) {}
        }
        Arc::new(NoAuth)
    }

    #[test]
    fn chat_path_avoids_duplicate_v1_when_base_url_already_has_v1() {
        let provider = test_provider("https://api.example.com/v1".to_string());
        assert_eq!(chat_path(&provider), "chat/completions");
        assert_eq!(
            provider.url_for_path(chat_path(&provider)),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[test]
    fn chat_path_avoids_duplicate_v1_with_trailing_slash_and_case() {
        let provider = test_provider("https://api.example.com/V1/".to_string());
        assert_eq!(chat_path(&provider), "chat/completions");
        assert_eq!(
            provider.url_for_path(chat_path(&provider)),
            "https://api.example.com/V1/chat/completions"
        );
    }

    #[test]
    fn chat_path_adds_v1_when_base_url_has_no_v1() {
        let provider = test_provider("https://api.example.com".to_string());
        assert_eq!(chat_path(&provider), "v1/chat/completions");
        assert_eq!(
            provider.url_for_path(chat_path(&provider)),
            "https://api.example.com/v1/chat/completions"
        );
    }

    #[tokio::test]
    async fn sends_request_and_receives_stream() {
        use wiremock::Mock;
        use wiremock::MockServer;
        use wiremock::ResponseTemplate;
        use wiremock::matchers::header;
        use wiremock::matchers::method;
        use wiremock::matchers::path;

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("accept", "text/event-stream"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n\
                 data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hi\"},\"finish_reason\":null}]}\n\n\
                 data: {\"id\":\"chatcmpl-123\",\"object\":\"chat.completion.chunk\",\"created\":1234567890,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1,\"total_tokens\":2}}\n\n\
                 data: [DONE]\n\n",
            ))
            .mount(&mock_server)
            .await;

        let provider = test_provider(mock_server.uri());
        let transport = ReqwestTransport::new(reqwest::Client::new());
        let client = ChatCompletionsClient::new(transport, provider, test_auth());

        let items = vec![ResponseItem::Message {
            id: None,
            role: "user".into(),
            content: vec![ContentItem::InputText {
                text: "Hello".into(),
            }],
            phase: None,
        }];

        let options = ChatOptions {
            extra_headers: HeaderMap::new(),
            max_tokens: 4096,
            temperature: None,
            top_p: None,
            tool_choice: None,
            parallel_tool_calls: None,
            output_schema: None,
            output_schema_strict: true,
            compression: Compression::None,
        };

        let mut stream = client
            .stream_request(
                "gpt-4o",
                "Be helpful.",
                &items,
                &[],
                None,
                ReasoningSummary::None,
                options,
            )
            .await
            .expect("request should succeed");

        let mut events: Vec<ResponseEvent> = Vec::new();
        while let Some(Ok(ev)) = stream.next().await {
            let done = matches!(ev, ResponseEvent::Completed { .. });
            events.push(ev);
            if done {
                break;
            }
        }

        assert!(!events.is_empty());
        assert!(matches!(
            events.last(),
            Some(ResponseEvent::Completed { .. })
        ));
    }

    #[tokio::test]
    async fn sends_response_format_when_output_schema_is_set() {
        use wiremock::Mock;
        use wiremock::MockServer;
        use wiremock::ResponseTemplate;
        use wiremock::matchers::method;
        use wiremock::matchers::path;

        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string("data: [DONE]\n\n"))
            .mount(&mock_server)
            .await;

        let provider = test_provider(mock_server.uri());
        let transport = ReqwestTransport::new(reqwest::Client::new());
        let client = ChatCompletionsClient::new(transport, provider, test_auth());
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "answer": { "type": "string" } },
            "required": ["answer"],
            "additionalProperties": false,
        });
        let options = ChatOptions {
            extra_headers: HeaderMap::new(),
            max_tokens: 4096,
            temperature: None,
            top_p: None,
            tool_choice: None,
            parallel_tool_calls: None,
            output_schema: Some(schema.clone()),
            output_schema_strict: true,
            compression: Compression::None,
        };

        let _stream = client
            .stream_request(
                "gpt-4o",
                "Be helpful.",
                &[],
                &[],
                None,
                ReasoningSummary::None,
                options,
            )
            .await
            .expect("request should succeed");

        let requests = mock_server.received_requests().await.unwrap_or_default();
        assert_eq!(requests.len(), 1);
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["response_format"]["type"], "json_schema");
        assert_eq!(
            body["response_format"]["json_schema"]["name"],
            "agere_output_schema"
        );
        assert_eq!(body["response_format"]["json_schema"]["schema"], schema);
        assert_eq!(body["response_format"]["json_schema"]["strict"], true);
    }

    #[test]
    fn truncate_utf8_ascii() {
        let s = "Hello World!";
        assert_eq!(truncate_utf8(s, 5), "Hello");
    }

    #[test]
    fn truncate_utf8_chinese() {
        let s = "中文测试字符串";
        // Each Chinese char is 3 bytes: 中(0..3), 文(3..6), 测(6..9), ...
        // byte 2 is inside "中", no complete char fits within 2 bytes
        assert_eq!(truncate_utf8(s, 2), "");
        // byte 5 is inside "文", only "中" fits within 5 bytes
        assert_eq!(truncate_utf8(s, 5), "中");
        // byte 6 is exactly at the boundary after "中文"
        assert_eq!(truncate_utf8(s, 6), "中文");
        // byte 7 is inside "测"
        assert_eq!(truncate_utf8(s, 7), "中文");
        // byte 9 is exactly after "中文测"
        assert_eq!(truncate_utf8(s, 9), "中文测");
    }

    #[test]
    fn truncate_utf8_shorter_than_max() {
        let s = "Hi";
        assert_eq!(truncate_utf8(s, 120), "Hi");
    }
}
