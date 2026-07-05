use crate::AnthropicClient;
use crate::AnthropicOptions;
use crate::config::DEFAULT_MAX_TOKENS;
use agere_api::Compression;
use agere_api::Provider;
use agere_api::RetryConfig;
use agere_client::HttpTransport;
use agere_client::ReqwestTransport;
use http::HeaderMap;
use http::HeaderValue;
use std::sync::Arc;
use std::time::Duration;

/// Anthropic provider configuration loaded from `.openagere/config.toml`.
#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub max_tokens: u32,
    pub stream_idle_timeout_ms: u64,
    pub max_retries: u64,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "claude-sonnet-4-6".into(),
            base_url: "https://api.anthropic.com".into(),
            max_tokens: DEFAULT_MAX_TOKENS,
            stream_idle_timeout_ms: 30_000,
            max_retries: 3,
        }
    }
}

/// API key-based auth provider for Anthropic.
struct AnthropicAuthProvider {
    api_key: String,
}

impl agere_api::AuthProvider for AnthropicAuthProvider {
    fn add_auth_headers(&self, headers: &mut http::HeaderMap) {
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&self.api_key).unwrap_or_else(|_| HeaderValue::from_static("")),
        );
    }
}

/// Build an AnthropicClient from configuration.
///
/// Reads the API key from:
/// 1. `ANTHROPIC_API_KEY` environment variable (highest priority)
/// 2. The provided config's `api_key` field
pub fn build_anthropic_client<T: HttpTransport>(
    config: AnthropicConfig,
    transport: T,
) -> AnthropicClient<T> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| config.api_key.clone());

    let provider = Provider {
        name: "anthropic".into(),
        base_url: config.base_url.clone(),
        query_params: None,
        headers: HeaderMap::new(),
        retry: RetryConfig {
            max_attempts: config.max_retries,
            base_delay: Duration::from_secs(1),
            retry_429: true,
            retry_5xx: true,
            retry_transport: true,
        },
        stream_idle_timeout: Duration::from_millis(config.stream_idle_timeout_ms),
    };

    let auth: Arc<dyn agere_api::AuthProvider> = Arc::new(AnthropicAuthProvider { api_key });

    AnthropicClient::new(transport, provider, auth)
}

/// Convenience function to build a fully configured AnthropicClient with default transport.
pub fn build_anthropic_client_with_default_transport(
    config: AnthropicConfig,
) -> AnthropicClient<ReqwestTransport> {
    let client = reqwest::Client::builder()
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let transport = ReqwestTransport::new(client);
    build_anthropic_client(config, transport)
}

/// Parse a minimal `.openagere/config.toml` for the `[anthropic]` section.
///
/// This is a lightweight parser that only reads the fields relevant to the
/// Anthropic client. It does not depend on the full config loading stack.
pub fn load_anthropic_config_from_file(path: &std::path::Path) -> std::io::Result<AnthropicConfig> {
    let contents = std::fs::read_to_string(path)?;
    let value: toml::Value = toml::from_str(&contents).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to parse TOML: {e}"),
        )
    })?;

    let mut config = AnthropicConfig::default();

    if let Some(anthropic) = value.get("anthropic").and_then(|v| v.as_table()) {
        if let Some(api_key) = anthropic.get("api_key").and_then(|v| v.as_str()) {
            config.api_key = api_key.to_string();
        }
        if let Some(model) = anthropic.get("model").and_then(|v| v.as_str()) {
            config.model = model.to_string();
        }
        if let Some(base_url) = anthropic.get("base_url").and_then(|v| v.as_str()) {
            config.base_url = base_url.to_string();
        }
        if let Some(max_tokens) = anthropic
            .get("max_tokens")
            .and_then(toml::Value::as_integer)
        {
            config.max_tokens = max_tokens as u32;
        }
        if let Some(timeout_ms) = anthropic
            .get("stream_idle_timeout_ms")
            .and_then(toml::Value::as_integer)
        {
            config.stream_idle_timeout_ms = timeout_ms as u64;
        }
        if let Some(retries) = anthropic
            .get("max_retries")
            .and_then(toml::Value::as_integer)
        {
            config.max_retries = retries as u64;
        }
    }

    Ok(config)
}

/// Build AnthropicOptions with sensible defaults.
pub fn default_anthropic_options() -> AnthropicOptions {
    AnthropicOptions {
        extra_headers: HeaderMap::new(),
        beta_features: vec![],
        max_tokens: DEFAULT_MAX_TOKENS,
        temperature: None,
        top_p: None,
        top_k: None,
        output_schema: None,
        compression: Compression::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn loads_anthropic_config_from_toml() {
        let mut file = NamedTempFile::new().expect("create temp file");
        writeln!(
            file,
            r#"
[anthropic]
api_key = "sk-test-key"
model = "claude-opus-4-7"
base_url = "https://api.anthropic.com"
max_tokens = 8192
stream_idle_timeout_ms = 60000
max_retries = 5
"#
        )
        .expect("write config");

        let config = load_anthropic_config_from_file(file.path()).expect("load config");
        assert_eq!(config.api_key, "sk-test-key");
        assert_eq!(config.model, "claude-opus-4-7");
        assert_eq!(config.base_url, "https://api.anthropic.com");
        assert_eq!(config.max_tokens, 8192);
        assert_eq!(config.stream_idle_timeout_ms, 60000);
        assert_eq!(config.max_retries, 5);
    }

    #[test]
    fn loads_defaults_when_section_missing() {
        let mut file = NamedTempFile::new().expect("create temp file");
        writeln!(file, "[other]\nfoo = \"bar\"").expect("write config");

        let config = load_anthropic_config_from_file(file.path()).expect("load config");
        assert_eq!(config.api_key, "");
        assert_eq!(config.model, "claude-sonnet-4-6");
        assert_eq!(config.base_url, "https://api.anthropic.com");
    }

    #[test]
    fn default_config_has_sane_values() {
        let config = AnthropicConfig::default();
        assert_eq!(config.model, "claude-sonnet-4-6");
        assert_eq!(config.base_url, "https://api.anthropic.com");
        assert_eq!(config.max_tokens, DEFAULT_MAX_TOKENS);
        assert_eq!(config.stream_idle_timeout_ms, 30_000);
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn default_options_builds() {
        let options = default_anthropic_options();
        assert_eq!(options.max_tokens, DEFAULT_MAX_TOKENS);
        assert!(options.temperature.is_none());
    }
}
