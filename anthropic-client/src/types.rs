use serde::Deserialize;
use serde::Serialize;

// ─── Request types ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MessagesRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemPrompt>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_config: Option<OutputConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<RequestMetadata>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Message {
    pub role: String,
    pub content: Vec<MessageContent>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(untagged)]
pub enum SystemPrompt {
    Text(String),
    #[allow(dead_code)]
    Blocks(Vec<SystemTextBlock>),
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SystemTextBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum MessageContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: ToolResultContent,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking {
        data: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(untagged)]
pub enum ToolResultContent {
    Text(String),
    Blocks(Vec<ToolResultBlock>),
}

#[cfg(test)]
impl ToolResultContent {
    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Blocks(_) => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum ToolResultBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    pub media_type: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ThinkingConfig {
    #[serde(rename = "type")]
    pub thinking_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
}

/// Controls thinking depth and token spend via the effort parameter.
/// Only supported on Opus 4.5+, Opus 4.6+, and Sonnet 4.6+.
/// `max` is Opus-tier only (not Sonnet or Haiku).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OutputConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<OutputFormat>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum OutputFormat {
    #[serde(rename = "json_schema")]
    JsonSchema { schema: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AnthropicTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type")]
pub enum ToolChoice {
    #[allow(dead_code)]
    #[serde(rename = "auto")]
    Auto,
    #[allow(dead_code)]
    #[serde(rename = "any")]
    Any,
    #[allow(dead_code)]
    #[serde(rename = "tool")]
    Tool { name: String },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RequestMetadata {
    pub user_id: Option<String>,
}

// ─── SSE event types ────────────────────────────────────

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub(crate) enum SseEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: MessageStartInfo },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: u32,
        content_block: ContentBlockStartInfo,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: u32, delta: Delta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: u32 },
    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: MessageDeltaInfo,
        usage: UsageInfo,
    },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "error")]
    Error { error: AnthropicErrorInfo },
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct MessageStartInfo {
    pub id: String,
    pub model: String,
    pub usage: UsageInfo,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub(crate) enum ContentBlockStartInfo {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking { data: String },
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub(crate) enum Delta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },
    #[serde(rename = "signature_delta")]
    SignatureDelta { signature: String },
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct MessageDeltaInfo {
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct UsageInfo {
    #[serde(default)]
    pub input_tokens: Option<i64>,
    #[serde(default)]
    pub output_tokens: Option<i64>,
    #[serde(default)]
    pub cache_creation_input_tokens: Option<i64>,
    #[serde(default)]
    pub cache_read_input_tokens: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct AnthropicErrorInfo {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn serialize_messages_request_minimal() {
        let req = MessagesRequest {
            model: "claude-sonnet-4-6".into(),
            messages: vec![Message {
                role: "user".into(),
                content: vec![MessageContent::Text {
                    text: "Hello".into(),
                }],
            }],
            system: Some(SystemPrompt::Text("Be helpful.".into())),
            max_tokens: 4096,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            thinking: None,
            output_config: None,
            tools: None,
            tool_choice: None,
            stream: true,
            metadata: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deser: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(deser["model"], "claude-sonnet-4-6");
        assert_eq!(deser["max_tokens"], 4096);
        assert_eq!(deser["stream"], true);
        assert_eq!(deser["system"], "Be helpful.");
        assert_eq!(deser["messages"][0]["role"], "user");
        assert_eq!(deser["messages"][0]["content"][0]["text"], "Hello");
    }

    #[test]
    fn serialize_tool_use_and_tool_result() {
        let req = MessagesRequest {
            model: "claude-sonnet-4-6".into(),
            messages: vec![
                Message {
                    role: "assistant".into(),
                    content: vec![MessageContent::ToolUse {
                        id: "toolu_001".into(),
                        name: "get_weather".into(),
                        input: serde_json::json!({"city": "SF"}),
                    }],
                },
                Message {
                    role: "user".into(),
                    content: vec![MessageContent::ToolResult {
                        tool_use_id: "toolu_001".into(),
                        content: ToolResultContent::Text("Sunny".into()),
                        is_error: None,
                    }],
                },
            ],
            system: None,
            max_tokens: 4096,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            thinking: None,
            output_config: None,
            tools: None,
            tool_choice: None,
            stream: true,
            metadata: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        let msg0_content = &json["messages"][0]["content"][0];
        assert_eq!(msg0_content["type"], "tool_use");
        assert_eq!(msg0_content["id"], "toolu_001");
        assert_eq!(msg0_content["name"], "get_weather");

        let msg1_content = &json["messages"][1]["content"][0];
        assert_eq!(msg1_content["type"], "tool_result");
        assert_eq!(msg1_content["tool_use_id"], "toolu_001");
    }

    #[test]
    fn serialize_output_config_json_schema_format() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "answer": { "type": "string" } },
            "required": ["answer"],
            "additionalProperties": false,
        });
        let req = MessagesRequest {
            model: "claude-test".to_string(),
            messages: vec![],
            system: None,
            max_tokens: 100,
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: None,
            thinking: None,
            output_config: Some(OutputConfig {
                effort: Some("low".to_string()),
                format: Some(OutputFormat::JsonSchema {
                    schema: schema.clone(),
                }),
            }),
            tools: None,
            tool_choice: None,
            stream: true,
            metadata: None,
        };

        let json = serde_json::to_value(req).unwrap();
        assert_eq!(json["output_config"]["effort"], "low");
        assert_eq!(json["output_config"]["format"]["type"], "json_schema");
        assert_eq!(json["output_config"]["format"]["schema"], schema);
    }

    #[test]
    fn deserialize_sse_message_start() {
        let json = r#"{"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","model":"claude-sonnet-4-6","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":100,"output_tokens":1}}}"#;
        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::MessageStart { message } => {
                assert_eq!(message.id, "msg_1");
                assert_eq!(message.model, "claude-sonnet-4-6");
                assert_eq!(message.usage.input_tokens, Some(100));
            }
            other => panic!("expected MessageStart, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_sse_content_block_delta_text() {
        let json = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                assert_matches::assert_matches!(delta, Delta::TextDelta { text } if text == "Hello");
            }
            other => panic!("expected ContentBlockDelta, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_sse_content_block_start_tool_use() {
        let json = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_001","name":"get_weather"}}"#;
        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                assert_eq!(index, 1);
                assert_matches::assert_matches!(content_block, ContentBlockStartInfo::ToolUse { id, name }
                    if id == "toolu_001" && name == "get_weather");
            }
            other => panic!("expected ContentBlockStart, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_sse_error() {
        let json =
            r#"{"type":"error","error":{"type":"overloaded_error","message":"Server busy"}}"#;
        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::Error { error } => {
                assert_eq!(error.error_type, "overloaded_error");
                assert_eq!(error.message, "Server busy");
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_message_delta_with_usage() {
        let json = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"input_tokens":100,"output_tokens":50}}"#;
        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::MessageDelta { delta, usage } => {
                assert_eq!(delta.stop_reason.as_deref(), Some("end_turn"));
                assert_eq!(usage.output_tokens, Some(50));
            }
            other => panic!("expected MessageDelta, got {other:?}"),
        }
    }

    #[test]
    fn deserialize_sse_redacted_thinking_content_block_start() {
        let json = r#"{"type":"content_block_start","index":0,"content_block":{"type":"redacted_thinking","data":"ZW5jcnlwdGVk"}}"#;
        let event: SseEvent = serde_json::from_str(json).unwrap();
        match event {
            SseEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                assert_eq!(index, 0);
                assert_matches::assert_matches!(content_block, ContentBlockStartInfo::RedactedThinking { data }
                    if data == "ZW5jcnlwdGVk");
            }
            other => panic!("expected ContentBlockStart for redacted thinking, got {other:?}"),
        }
    }

    #[test]
    fn serialize_redacted_thinking_message_content() {
        let msg = Message {
            role: "assistant".into(),
            content: vec![MessageContent::RedactedThinking {
                data: "ZW5jcnlwdGVk".into(),
                signature: Some("sig_xyz".into()),
            }],
        };
        let json = serde_json::to_value(&msg).unwrap();
        let block = &json["content"][0];
        assert_eq!(block["type"], "redacted_thinking");
        assert_eq!(block["data"], "ZW5jcnlwdGVk");
        assert_eq!(block["signature"], "sig_xyz");
    }
}
