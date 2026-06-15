pub(crate) mod client;
pub(crate) mod error;
pub(crate) mod sse;
pub(crate) mod translate;
pub(crate) mod types;

pub use client::ChatCompletionsClient;

use agere_api::Compression;
use http::HeaderMap;

/// Tool definition compatible with Chat Completions API.
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Options for Chat Completions API requests.
#[derive(Debug, Clone)]
pub struct ChatOptions {
    pub extra_headers: HeaderMap,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub tool_choice: Option<ChatToolChoice>,
    pub parallel_tool_calls: Option<bool>,
    pub output_schema: Option<serde_json::Value>,
    pub output_schema_strict: bool,
    pub compression: Compression,
}

/// Tool choice for Chat Completions API.
#[derive(Debug, Clone)]
pub enum ChatToolChoice {
    None,
    Auto,
    Required,
    Function { name: String },
}

/// Default ChatOptions for use with `ChatCompletionsClient`.
pub fn default_chat_options() -> ChatOptions {
    ChatOptions {
        extra_headers: HeaderMap::new(),
        max_tokens: 4096,
        temperature: None,
        top_p: None,
        tool_choice: None,
        parallel_tool_calls: None,
        output_schema: None,
        output_schema_strict: true,
        compression: Compression::None,
    }
}
