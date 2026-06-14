pub(crate) mod client;
pub(crate) mod config;
pub(crate) mod error;
pub(crate) mod sse;
pub(crate) mod translate;
pub mod types;

pub mod provider_config;

pub use client::AnthropicClient;
pub use translate::request::MessageBuildContext;
pub use translate::request::build_anthropic_messages_from_response_items;
pub use translate::request::response_item_to_input;

use agere_api::Compression;
use http::HeaderMap;

#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct AnthropicOptions {
    pub extra_headers: HeaderMap,
    pub beta_features: Vec<String>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub top_k: Option<u32>,
    pub output_schema: Option<serde_json::Value>,
    pub compression: Compression,
}
