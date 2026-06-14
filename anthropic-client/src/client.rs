use crate::AnthropicOptions;
use crate::ToolDefinition;
use crate::config::ANTHROPIC_API_VERSION;
use crate::config::beta_header;
use crate::sse::process_anthropic_sse;
use crate::translate::request::build_anthropic_request;
use agere_api::ApiError;
use agere_api::Compression;
use agere_api::Provider;
use agere_api::ResponseEvent;
use agere_api::ResponseStream;
use agere_api::SharedAuthProvider;
use agere_client::HttpTransport;
use agere_client::RequestBody;
use agere_client::RequestCompression;
use agere_client::TransportError;
use agere_protocol::config_types::ReasoningSummary;
use agere_protocol::models::ResponseInputItem;
use agere_protocol::openai_models::ReasoningEffort;
use http::HeaderValue;
use http::Method;
use tokio::sync::mpsc;
use tracing::debug;

/// Client for the Anthropic Messages API.
///
/// Takes internal protocol types (ResponseInputItem, ToolDefinition, etc.)
/// and orchestrates: translation -> authentication -> HTTP request -> SSE parsing.
pub struct AnthropicClient<T: HttpTransport> {
    transport: T,
    provider: Provider,
    auth: SharedAuthProvider,
}

impl<T: HttpTransport> AnthropicClient<T> {
    pub fn new(transport: T, provider: Provider, auth: SharedAuthProvider) -> Self {
        Self {
            transport,
            provider,
            auth,
        }
    }

    /// Send a streaming request with pre-built Anthropic messages.
    /// Use this when you have already converted from ResponseItem → Anthropic Message format.
    pub async fn stream_request_with_messages(
        &self,
        model: &str,
        system: &str,
        messages: Vec<crate::types::Message>,
        tools: &[ToolDefinition],
        thinking: Option<ReasoningEffort>,
        reasoning_summary: ReasoningSummary,
        options: AnthropicOptions,
    ) -> Result<ResponseStream, ApiError> {
        use crate::translate::thinking::to_anthropic_output_config;
        use crate::translate::thinking::to_anthropic_thinking;
        use crate::translate::tools::to_anthropic_tools;
        use crate::types::SystemPrompt;

        let request_body = crate::types::MessagesRequest {
            model: model.to_string(),
            messages,
            system: if system.is_empty() {
                None
            } else {
                Some(SystemPrompt::Text(system.to_string()))
            },
            max_tokens: options.max_tokens,
            temperature: options.temperature,
            top_p: options.top_p,
            top_k: options.top_k,
            stop_sequences: None,
            thinking: to_anthropic_thinking(thinking),
            output_config: output_config_with_schema(
                to_anthropic_output_config(thinking),
                options.output_schema.clone(),
            ),
            tools: to_anthropic_tools(tools),
            tool_choice: None,
            stream: true,
            metadata: None,
        };

        self.send_request(request_body, options, reasoning_summary)
            .await
    }

    /// Send a streaming request to the Anthropic Messages API
    /// (with ResponseInputItem → Message conversion).
    pub async fn stream_request(
        &self,
        model: &str,
        system: &str,
        messages: &[ResponseInputItem],
        tools: &[ToolDefinition],
        thinking: Option<ReasoningEffort>,
        reasoning_summary: ReasoningSummary,
        options: AnthropicOptions,
    ) -> Result<ResponseStream, ApiError> {
        let mut request_body = build_anthropic_request(
            model,
            system,
            messages,
            tools,
            thinking,
            options.max_tokens,
            options.temperature,
            options.top_p,
            options.top_k,
            None, // tool_choice — derived from options if needed
        );
        request_body.output_config =
            output_config_with_schema(request_body.output_config, options.output_schema.clone());

        self.send_request(request_body, options, reasoning_summary)
            .await
    }

    async fn send_request(
        &self,
        request_body: crate::types::MessagesRequest,
        options: AnthropicOptions,
        reasoning_summary: ReasoningSummary,
    ) -> Result<ResponseStream, ApiError> {
        let body_json = serde_json::to_value(&request_body)
            .map_err(|e| ApiError::Stream(format!("encode error: {e}")))?;

        // Log request for debugging thinking-related issues
        debug!(
            model = %request_body.model,
            msg_count = request_body.messages.len(),
            thinking = ?request_body.thinking,
            stream = request_body.stream,
            "Anthropic request built"
        );
        for (i, msg) in request_body.messages.iter().enumerate() {
            let content_types: Vec<&str> = msg
                .content
                .iter()
                .map(|c| match c {
                    crate::types::MessageContent::Text { .. } => "text",
                    crate::types::MessageContent::Thinking { .. } => "thinking",
                    crate::types::MessageContent::RedactedThinking { .. } => "redacted_thinking",
                    crate::types::MessageContent::ToolUse { .. } => "tool_use",
                    crate::types::MessageContent::ToolResult { .. } => "tool_result",
                    crate::types::MessageContent::Image { .. } => "image",
                })
                .collect();
            debug!(
                "  msg[{}] role={} content_types={:?}",
                i, msg.role, content_types
            );
        }
        if request_body.thinking.is_some() {
            // Log full body for thinking requests to help debug signature issues
            let body_str = serde_json::to_string(&body_json).unwrap_or_default();
            debug!("Anthropic request body (thinking enabled): {}", body_str);
        }

        let mut headers = options.extra_headers;
        headers.insert(
            "anthropic-version",
            HeaderValue::from_static(ANTHROPIC_API_VERSION),
        );
        let beta = beta_header(&options.beta_features);
        headers.insert(
            "anthropic-beta",
            HeaderValue::from_str(&beta)
                .map_err(|e| ApiError::Stream(format!("invalid beta: {e}")))?,
        );
        headers.insert(
            http::header::ACCEPT,
            HeaderValue::from_static("text/event-stream"),
        );

        let compression = match options.compression {
            Compression::None => RequestCompression::None,
            Compression::Zstd => RequestCompression::Zstd,
        };

        let make_req = || {
            let mut req = self.provider.build_request(Method::POST, "v1/messages");
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

        // Anthropic Messages streams raw thinking, not a separate summary event.
        // Do not synthesize user-visible summaries from raw thinking here.
        let _ = reasoning_summary;
        tokio::spawn(process_anthropic_sse(
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

fn output_config_with_schema(
    mut output_config: Option<crate::types::OutputConfig>,
    output_schema: Option<serde_json::Value>,
) -> Option<crate::types::OutputConfig> {
    if let Some(schema) = output_schema {
        let format = crate::types::OutputFormat::JsonSchema { schema };
        match output_config.as_mut() {
            Some(config) => config.format = Some(format),
            None => {
                output_config = Some(crate::types::OutputConfig {
                    effort: None,
                    format: Some(format),
                });
            }
        }
    }
    output_config
}

#[cfg(test)]
mod tests {
    use super::*;
    use agere_api::ReqwestTransport;
    use agere_api::RetryConfig;
    use agere_protocol::models::ContentItem;
    use futures::StreamExt;
    use http::HeaderMap;

    fn test_provider(base_url: String) -> Provider {
        Provider {
            name: "anthropic".into(),
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
            .and(path("/v1/messages"))
            .and(header("anthropic-version", "2023-06-01"))
            .and(header("accept", "text/event-stream"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-sonnet-4-6\",\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n\
                 event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
                 event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\n\
                 event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n\
                 event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}\n\n\
                 event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
            ))
            .mount(&mock_server)
            .await;

        let provider = test_provider(mock_server.uri());
        let transport = ReqwestTransport::new(reqwest::Client::new());
        let client = AnthropicClient::new(transport, provider, test_auth());

        let items = vec![ResponseInputItem::Message {
            role: "user".into(),
            content: vec![ContentItem::InputText {
                text: "Hello".into(),
            }],
            phase: None,
        }];

        let options = AnthropicOptions {
            extra_headers: HeaderMap::new(),
            beta_features: vec![],
            max_tokens: 4096,
            temperature: None,
            top_p: None,
            top_k: None,
            output_schema: None,
            compression: Compression::None,
        };

        let mut stream = client
            .stream_request(
                "claude-sonnet-4-6",
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
    async fn sends_output_config_format_when_output_schema_is_set() {
        use wiremock::Mock;
        use wiremock::MockServer;
        use wiremock::ResponseTemplate;
        use wiremock::matchers::method;
        use wiremock::matchers::path;

        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string("event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"),
            )
            .mount(&mock_server)
            .await;

        let provider = test_provider(mock_server.uri());
        let transport = ReqwestTransport::new(reqwest::Client::new());
        let client = AnthropicClient::new(transport, provider, test_auth());
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "answer": { "type": "string" } },
            "required": ["answer"],
            "additionalProperties": false,
        });
        let options = AnthropicOptions {
            extra_headers: HeaderMap::new(),
            beta_features: vec![],
            max_tokens: 4096,
            temperature: None,
            top_p: None,
            top_k: None,
            output_schema: Some(schema.clone()),
            compression: Compression::None,
        };

        let _stream = client
            .stream_request(
                "claude-sonnet-4-6",
                "Be helpful.",
                &[],
                &[],
                Some(ReasoningEffort::Low),
                ReasoningSummary::None,
                options,
            )
            .await
            .expect("request should succeed");

        let requests = mock_server.received_requests().await.unwrap_or_default();
        assert_eq!(requests.len(), 1);
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(body["output_config"]["effort"], "low");
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert_eq!(body["output_config"]["format"]["schema"], schema);
    }
}
