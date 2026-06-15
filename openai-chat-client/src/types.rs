use serde::Deserialize;
use serde::Serialize;

// ─── Request types ───────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ChatTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ChatToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ChatResponseFormat>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum ChatResponseFormat {
    #[serde(rename = "json_schema")]
    JsonSchema { json_schema: ChatJsonSchemaFormat },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChatJsonSchemaFormat {
    pub name: String,
    pub schema: serde_json::Value,
    pub strict: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<ChatContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
}

/// Chat Completions content can be either plain text or a list of content blocks.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(untagged)]
pub enum ChatContent {
    Text(String),
    Blocks(Vec<ChatContentBlock>),
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChatContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<ChatImageUrl>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChatImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ChatFunctionDef,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatFunctionDef {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum ChatToolChoice {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "required")]
    Required,
    #[serde(rename = "function")]
    Function { function: ChatFunctionChoiceName },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ChatFunctionChoiceName {
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ChatFunctionCall,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamOptions {
    pub include_usage: bool,
}

// ─── SSE event types (parsed from Chat Completions stream) ────────────

/// A single parsed SSE event from the Chat Completions stream.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum ChatSseEvent {
    #[serde(rename = "")]
    Done,
    Chunk {
        id: String,
        model: String,
        choices: Vec<ChatChoice>,
        #[serde(default)]
        usage: Option<ChatUsage>,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChatChoice {
    #[allow(dead_code)]
    pub index: usize,
    pub delta: ChatDelta,
    #[serde(default)]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub(crate) struct ChatDelta {
    #[allow(dead_code)]
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ChatDeltaToolCall>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChatDeltaToolCall {
    pub index: usize,
    #[serde(default)]
    pub id: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    #[serde(default)]
    pub call_type: Option<String>,
    #[serde(default)]
    pub function: Option<ChatDeltaFunctionCall>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChatDeltaFunctionCall {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChatUsage {
    #[serde(default)]
    pub prompt_tokens: Option<i64>,
    #[serde(default)]
    pub completion_tokens: Option<i64>,
    #[serde(default)]
    pub total_tokens: Option<i64>,
    #[serde(default)]
    pub prompt_tokens_details: Option<ChatUsageDetails>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ChatUsageDetails {
    #[serde(default)]
    pub cached_tokens: Option<i64>,
}

// ─── Tests ─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_chat_request_minimal() {
        let req = ChatRequest {
            model: "gpt-4o".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: Some(ChatContent::Text("Hello".into())),
                tool_calls: None,
                tool_call_id: None,
                reasoning: None,
            }],
            tools: vec![],
            tool_choice: None,
            parallel_tool_calls: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            reasoning_effort: None,
            response_format: None,
            stream: true,
            stream_options: Some(StreamOptions {
                include_usage: true,
            }),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["model"], "gpt-4o");
        assert_eq!(json["messages"][0]["role"], "user");
        assert_eq!(json["messages"][0]["content"], "Hello");
        assert_eq!(json["stream"], true);
        assert_eq!(json["stream_options"]["include_usage"], true);
    }

    #[test]
    fn serialize_tools_in_request() {
        let req = ChatRequest {
            model: "gpt-4o".into(),
            messages: vec![],
            tools: vec![ChatTool {
                tool_type: "function".into(),
                function: ChatFunctionDef {
                    name: "get_weather".into(),
                    description: Some("Get weather".into()),
                    parameters: serde_json::json!({"type": "object"}),
                },
            }],
            tool_choice: Some(ChatToolChoice::Auto),
            parallel_tool_calls: Some(true),
            temperature: None,
            top_p: None,
            max_tokens: None,
            reasoning_effort: None,
            response_format: None,
            stream: true,
            stream_options: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["tools"][0]["type"], "function");
        assert_eq!(json["tools"][0]["function"]["name"], "get_weather");
        assert_eq!(json["tool_choice"]["type"], "auto");
        assert_eq!(json["parallel_tool_calls"], true);
    }

    #[test]
    fn serialize_response_format_json_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "answer": { "type": "string" } },
            "required": ["answer"],
            "additionalProperties": false,
        });
        let req = ChatRequest {
            model: "gpt-test".to_string(),
            messages: vec![],
            tools: vec![],
            tool_choice: None,
            parallel_tool_calls: None,
            temperature: None,
            top_p: None,
            max_tokens: Some(100),
            reasoning_effort: None,
            response_format: Some(ChatResponseFormat::JsonSchema {
                json_schema: ChatJsonSchemaFormat {
                    name: "agere_output_schema".to_string(),
                    schema: schema.clone(),
                    strict: true,
                },
            }),
            stream: true,
            stream_options: None,
        };

        let json = serde_json::to_value(req).unwrap();
        assert_eq!(
            json["response_format"],
            serde_json::json!({
                "type": "json_schema",
                "json_schema": {
                    "name": "agere_output_schema",
                    "schema": schema,
                    "strict": true,
                },
            })
        );
    }

    #[test]
    fn serialize_tool_choice_function() {
        let choice = ChatToolChoice::Function {
            function: ChatFunctionChoiceName {
                name: "specific_tool".into(),
            },
        };
        let json = serde_json::to_value(&choice).unwrap();
        assert_eq!(json["type"], "function");
        assert_eq!(json["function"]["name"], "specific_tool");
    }

    #[test]
    fn serialize_message_with_tool_calls() {
        let msg = ChatMessage {
            role: "assistant".into(),
            content: None,
            tool_calls: Some(vec![ChatToolCall {
                id: "call_abc123".into(),
                call_type: "function".into(),
                function: ChatFunctionCall {
                    name: "get_weather".into(),
                    arguments: r#"{"city":"NYC"}"#.into(),
                },
            }]),
            tool_call_id: None,
            reasoning: None,
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "assistant");
        assert_eq!(json["tool_calls"][0]["id"], "call_abc123");
        assert_eq!(json["tool_calls"][0]["function"]["name"], "get_weather");
        assert!(json.get("content").is_none());
    }

    #[test]
    fn deserialize_chat_sse_chunk() {
        let json = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"role":"assistant","content":"Hello"},"finish_reason":null}]}"#;
        let event: ChatSseEvent = serde_json::from_str(json).unwrap();
        match event {
            ChatSseEvent::Chunk { id, choices, .. } => {
                assert_eq!(id, "chatcmpl-123");
                assert_eq!(choices.len(), 1);
                assert_eq!(choices[0].delta.role, Some("assistant".into()));
                assert_eq!(choices[0].delta.content, Some("Hello".into()));
            }
            _ => panic!("expected Chunk variant"),
        }
    }

    #[test]
    fn deserialize_tool_call_delta() {
        let json = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_abc","type":"function","function":{"name":"get_weather"}}]},"finish_reason":null}]}"#;
        let event: ChatSseEvent = serde_json::from_str(json).unwrap();
        match event {
            ChatSseEvent::Chunk { choices, .. } => {
                let delta = &choices[0].delta;
                assert!(delta.tool_calls.is_some());
                let tc = &delta.tool_calls.as_ref().unwrap()[0];
                assert_eq!(tc.id.as_deref(), Some("call_abc"));
                assert_eq!(
                    tc.function.as_ref().unwrap().name.as_deref(),
                    Some("get_weather")
                );
            }
            _ => panic!("expected Chunk variant"),
        }
    }

    #[test]
    fn deserialize_usage_chunk() {
        let json = r#"{"id":"chatcmpl-123","object":"chat.completion.chunk","created":1234567890,"model":"gpt-4o","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
        let event: ChatSseEvent = serde_json::from_str(json).unwrap();
        match event {
            ChatSseEvent::Chunk { usage, choices, .. } => {
                assert!(usage.is_some());
                let u = usage.unwrap();
                assert_eq!(u.prompt_tokens, Some(10));
                assert_eq!(u.completion_tokens, Some(5));
                assert_eq!(u.total_tokens, Some(15));
                assert_eq!(choices[0].finish_reason, Some("stop".into()));
            }
            _ => panic!("expected Chunk variant"),
        }
    }
}
