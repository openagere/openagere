//! Registry of model providers supported by Agere.
//!
//! Providers can be defined in two places:
//!   1. Built-in defaults compiled into the binary so Agere works out-of-the-box.
//!   2. User-defined entries inside `~/.openagere/config.toml` under the `model_providers`
//!      key. These override or extend the defaults at runtime.

use agere_api::Provider as ApiProvider;
use agere_api::RetryConfig as ApiRetryConfig;
use agere_api::is_azure_responses_provider;
use agere_app_server_protocol::AuthMode;
use agere_protocol::error::AgereErr;
use agere_protocol::error::EnvVarError;
use agere_protocol::error::Result as AgereResult;
use http::HeaderMap;
use http::header::HeaderName;
use http::header::HeaderValue;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

const DEFAULT_STREAM_IDLE_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_STREAM_MAX_RETRIES: u64 = 5;
const DEFAULT_REQUEST_MAX_RETRIES: u64 = 4;
pub const DEFAULT_WEBSOCKET_CONNECT_TIMEOUT_MS: u64 = 15_000;
/// Hard cap for user-configured `stream_max_retries`.
const MAX_STREAM_MAX_RETRIES: u64 = 100;
/// Hard cap for user-configured `request_max_retries`.
const MAX_REQUEST_MAX_RETRIES: u64 = 100;

pub const OPENAI_PROVIDER_ID: &str = "openai";
pub const AMAZON_BEDROCK_PROVIDER_ID: &str = "amazon-bedrock";

/// List of built-in provider IDs that are compiled into the binary.
/// These providers have bundled model catalogs and do not require
/// per-provider configuration in `config.toml`.
pub const BUILT_IN_PROVIDERS: &[&str] = &[OPENAI_PROVIDER_ID, AMAZON_BEDROCK_PROVIDER_ID];
pub const AMAZON_BEDROCK_DEFAULT_BASE_URL: &str =
    "https://bedrock-mantle.us-east-1.api.aws/openai/v1";
pub const LEGACY_OLLAMA_CHAT_PROVIDER_ID: &str = "ollama-chat";
pub const OLLAMA_CHAT_PROVIDER_REMOVED_ERROR: &str = "`ollama-chat` is no longer supported.\nHow to fix: replace `ollama-chat` with `ollama` in `model_provider`, `oss_provider`, or `--local-provider`.";

/// Wire protocol that the provider speaks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WireApi {
    /// The Responses API exposed by OpenAI at `/v1/responses`.
    #[default]
    Responses,
    /// The Anthropic Messages API at `/v1/messages`.
    Anthropic,
    /// The OpenAI Chat Completions API at `/v1/chat/completions`.
    Chat,
}

impl fmt::Display for WireApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Responses => "responses",
            Self::Anthropic => "anthropic",
            Self::Chat => "chat",
        };
        f.write_str(value)
    }
}

impl<'de> Deserialize<'de> for WireApi {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "responses" => Ok(Self::Responses),
            "anthropic" => Ok(Self::Anthropic),
            "chat" => Ok(Self::Chat),
            _ => Err(serde::de::Error::unknown_variant(
                &value,
                &["responses", "anthropic", "chat"],
            )),
        }
    }
}

/// Hidden placeholder used only to reject unsupported provider auth config with a clear error.
#[doc(hidden)]
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct UnsupportedModelProviderAuthInfo {}

/// Serializable representation of a provider definition.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ModelProviderInfo {
    /// Base URL for the provider's OpenAI-compatible API.
    pub base_url: Option<String>,
    /// Environment variable that stores the user's API key for this provider.
    pub env_key: Option<String>,

    /// Optional instructions to help the user get a valid value for the
    /// variable and set it.
    pub env_key_instructions: Option<String>,
    /// Value to use with `Authorization: Bearer <token>` header. Use of this
    /// config is discouraged in favor of `env_key` for security reasons, but
    /// this may be necessary when using this programmatically.
    #[serde(default)]
    pub experimental_bearer_token: Option<String>,
    /// Unsupported command-backed bearer-token configuration for this provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub auth: Option<UnsupportedModelProviderAuthInfo>,
    /// AWS SigV4 auth configuration for this provider.
    pub aws: Option<ModelProviderAwsAuthInfo>,
    /// Which wire protocol this provider expects.
    #[serde(default)]
    pub wire_api: WireApi,
    /// Optional query parameters to append to the base URL.
    pub query_params: Option<HashMap<String, String>>,
    /// Additional HTTP headers to include in requests to this provider where
    /// the (key, value) pairs are the header name and value.
    pub http_headers: Option<HashMap<String, String>>,
    /// Optional HTTP headers to include in requests to this provider where the
    /// (key, value) pairs are the header name and _environment variable_ whose
    /// value should be used. If the environment variable is not set, or the
    /// value is empty, the header will not be included in the request.
    pub env_http_headers: Option<HashMap<String, String>>,
    /// Maximum number of times to retry a failed HTTP request to this provider.
    pub request_max_retries: Option<u64>,
    /// Number of times to retry reconnecting a dropped streaming response before failing.
    pub stream_max_retries: Option<u64>,
    /// Idle timeout (in milliseconds) to wait for activity on a streaming response before treating
    /// the connection as lost.
    pub stream_idle_timeout_ms: Option<u64>,
    /// Maximum time (in milliseconds) to wait for a websocket connection attempt before treating
    /// it as failed.
    pub websocket_connect_timeout_ms: Option<u64>,
    /// Does this provider require authentication? If true,
    /// user is presented with login screen on first run, and login preference and token/key
    /// are stored in auth.json. If false (which is the default), login screen is skipped,
    /// and API key (if needed) comes from the "env_key" environment variable.
    #[serde(default)]
    pub requires_provider_auth: bool,
    /// Whether this provider supports the Responses API WebSocket transport.
    #[serde(default)]
    pub supports_websockets: bool,
}

/// AWS SigV4 auth configuration for a model provider.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ModelProviderAwsAuthInfo {
    /// AWS profile name to use. When unset, the AWS SDK default chain decides.
    pub profile: Option<String>,
    /// AWS region to use for provider-specific endpoints.
    pub region: Option<String>,
}

impl ModelProviderInfo {
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.auth.is_some() {
            return Err(
                "provider auth is not supported; use env_key or experimental_bearer_token"
                    .to_string(),
            );
        }

        if self.aws.is_some() {
            if self.supports_websockets {
                // TODO(celia-oai): Support AWS SigV4 signing for WebSocket
                // upgrade requests before allowing AWS-authenticated providers
                // to enable Responses-over-WebSocket.
                return Err("provider aws cannot be combined with supports_websockets".to_string());
            }

            let mut conflicts = Vec::new();
            if self.env_key.is_some() {
                conflicts.push("env_key");
            }
            if self.experimental_bearer_token.is_some() {
                conflicts.push("experimental_bearer_token");
            }
            if self.requires_provider_auth {
                conflicts.push("requires_provider_auth");
            }

            if !conflicts.is_empty() {
                return Err(format!(
                    "provider aws cannot be combined with {}",
                    conflicts.join(", ")
                ));
            }
        }

        Ok(())
    }

    fn build_header_map(&self) -> AgereResult<HeaderMap> {
        let capacity = self.http_headers.as_ref().map_or(0, HashMap::len)
            + self.env_http_headers.as_ref().map_or(0, HashMap::len);
        let mut headers = HeaderMap::with_capacity(capacity);
        if let Some(extra) = &self.http_headers {
            for (k, v) in extra {
                if let (Ok(name), Ok(value)) = (HeaderName::try_from(k), HeaderValue::try_from(v)) {
                    headers.insert(name, value);
                }
            }
        }

        if let Some(env_headers) = &self.env_http_headers {
            for (header, env_var) in env_headers {
                if let Ok(val) = std::env::var(env_var)
                    && !val.trim().is_empty()
                    && let (Ok(name), Ok(value)) =
                        (HeaderName::try_from(header), HeaderValue::try_from(val))
                {
                    headers.insert(name, value);
                }
            }
        }

        Ok(headers)
    }

    pub fn to_api_provider(&self, auth_mode: Option<AuthMode>) -> AgereResult<ApiProvider> {
        let default_base_url = if matches!(
            auth_mode,
            Some(AuthMode::Chatgpt | AuthMode::ChatgptAuthTokens | AuthMode::AgentIdentity)
        ) {
            "https://chatgpt.com/backend-api/agere"
        } else {
            "https://api.openai.com/v1"
        };
        let base_url = self
            .base_url
            .clone()
            .unwrap_or_else(|| default_base_url.to_string());

        let headers = self.build_header_map()?;
        let retry = ApiRetryConfig {
            max_attempts: self.request_max_retries(),
            base_delay: Duration::from_millis(200),
            retry_429: false,
            retry_5xx: true,
            retry_transport: true,
        };

        Ok(ApiProvider {
            name: match self.wire_api {
                WireApi::Anthropic => "anthropic".to_string(),
                WireApi::Chat => "chat".to_string(),
                WireApi::Responses => {
                    if self.is_amazon_bedrock() {
                        "amazon-bedrock".to_string()
                    } else {
                        "openai".to_string()
                    }
                }
            },
            base_url,
            query_params: self.query_params.clone(),
            headers,
            retry,
            stream_idle_timeout: self.stream_idle_timeout(),
        })
    }

    /// If `env_key` is Some, returns the API key for this provider if present
    /// (and non-empty) in the environment. If `env_key` is required but
    /// cannot be found, returns an error.
    pub fn api_key(&self) -> AgereResult<Option<String>> {
        match &self.env_key {
            Some(env_key) => {
                let api_key = std::env::var(env_key)
                    .ok()
                    .filter(|v| !v.trim().is_empty())
                    .ok_or_else(|| {
                        AgereErr::EnvVar(EnvVarError {
                            var: env_key.clone(),
                            instructions: self.env_key_instructions.clone(),
                        })
                    })?;
                Ok(Some(api_key))
            }
            None => Ok(None),
        }
    }

    /// Effective maximum number of request retries for this provider.
    pub fn request_max_retries(&self) -> u64 {
        self.request_max_retries
            .unwrap_or(DEFAULT_REQUEST_MAX_RETRIES)
            .min(MAX_REQUEST_MAX_RETRIES)
    }

    /// Effective maximum number of stream reconnection attempts for this provider.
    pub fn stream_max_retries(&self) -> u64 {
        self.stream_max_retries
            .unwrap_or(DEFAULT_STREAM_MAX_RETRIES)
            .min(MAX_STREAM_MAX_RETRIES)
    }

    /// Effective idle timeout for streaming responses.
    pub fn stream_idle_timeout(&self) -> Duration {
        self.stream_idle_timeout_ms
            .map(Duration::from_millis)
            .unwrap_or(Duration::from_millis(DEFAULT_STREAM_IDLE_TIMEOUT_MS))
    }

    /// Effective timeout for websocket connect attempts.
    pub fn websocket_connect_timeout(&self) -> Duration {
        self.websocket_connect_timeout_ms
            .map(Duration::from_millis)
            .unwrap_or(Duration::from_millis(DEFAULT_WEBSOCKET_CONNECT_TIMEOUT_MS))
    }

    pub fn create_openai_provider(base_url: Option<String>) -> ModelProviderInfo {
        ModelProviderInfo {
            base_url,
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            auth: None,
            aws: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: Some(
                [("version".to_string(), env!("CARGO_PKG_VERSION").to_string())]
                    .into_iter()
                    .collect(),
            ),
            env_http_headers: Some(
                [
                    (
                        "OpenAI-Organization".to_string(),
                        "OPENAI_ORGANIZATION".to_string(),
                    ),
                    ("OpenAI-Project".to_string(), "OPENAI_PROJECT".to_string()),
                ]
                .into_iter()
                .collect(),
            ),
            // Use global defaults for retry/timeout unless overridden in config.toml.
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            websocket_connect_timeout_ms: None,
            requires_provider_auth: true,
            supports_websockets: true,
        }
    }

    pub fn create_amazon_bedrock_provider(
        aws: Option<ModelProviderAwsAuthInfo>,
    ) -> ModelProviderInfo {
        ModelProviderInfo {
            base_url: Some(AMAZON_BEDROCK_DEFAULT_BASE_URL.into()),
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            auth: None,
            aws: Some(aws.unwrap_or(ModelProviderAwsAuthInfo {
                profile: None,
                region: None,
            })),
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            websocket_connect_timeout_ms: None,
            requires_provider_auth: false,
            supports_websockets: false,
        }
    }

    pub fn is_openai(&self) -> bool {
        matches!(self.wire_api, WireApi::Responses)
            && self.base_url.as_deref().is_none_or(|url| {
                url.starts_with("https://api.openai.com") || url.starts_with("https://chatgpt.com")
            })
    }

    pub fn is_amazon_bedrock(&self) -> bool {
        self.aws.is_some()
    }

    pub fn is_anthropic(&self) -> bool {
        self.wire_api == WireApi::Anthropic
    }

    pub fn supports_remote_compaction(&self) -> bool {
        self.is_openai()
            || self
                .base_url
                .as_deref()
                .is_some_and(|url| is_azure_responses_provider("", Some(url)))
    }
}

/// Built-in default provider list.
pub fn built_in_model_providers(
    openai_base_url: Option<String>,
) -> HashMap<String, ModelProviderInfo> {
    use ModelProviderInfo as P;
    let openai_provider = P::create_openai_provider(openai_base_url);
    let amazon_bedrock_provider = P::create_amazon_bedrock_provider(/*aws*/ None);

    [
        (OPENAI_PROVIDER_ID, openai_provider),
        (AMAZON_BEDROCK_PROVIDER_ID, amazon_bedrock_provider),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

/// Merge configured providers into the built-in provider catalog.
///
/// Configured providers extend the built-in set. Built-in providers are not
/// generally overridable, but the built-in Amazon Bedrock provider allows the
/// user to set `aws.profile` and `aws.region`.
pub fn merge_configured_model_providers(
    mut model_providers: HashMap<String, ModelProviderInfo>,
    configured_model_providers: HashMap<String, ModelProviderInfo>,
) -> Result<HashMap<String, ModelProviderInfo>, String> {
    for (key, mut provider) in configured_model_providers {
        if key == AMAZON_BEDROCK_PROVIDER_ID {
            let aws_override = provider.aws.take();
            if provider != ModelProviderInfo::default() {
                return Err(format!(
                    "model_providers.{AMAZON_BEDROCK_PROVIDER_ID} only supports changing \
`aws.profile` and `aws.region`; other non-default provider fields are not supported"
                ));
            }

            if let Some(aws_override) = aws_override
                && let Some(built_in_provider) = model_providers.get_mut(AMAZON_BEDROCK_PROVIDER_ID)
                && let Some(built_in_aws) = built_in_provider.aws.as_mut()
            {
                if let Some(profile) = aws_override.profile {
                    built_in_aws.profile = Some(profile);
                }
                if let Some(region) = aws_override.region {
                    built_in_aws.region = Some(region);
                }
            }
        } else {
            model_providers.entry(key).or_insert(provider);
        }
    }

    Ok(model_providers)
}

pub fn create_oss_provider_with_base_url(base_url: &str, wire_api: WireApi) -> ModelProviderInfo {
    ModelProviderInfo {
        base_url: Some(base_url.into()),
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        auth: None,
        aws: None,
        wire_api,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        websocket_connect_timeout_ms: None,
        requires_provider_auth: false,
        supports_websockets: false,
    }
}

/// Proxy provider domains that require thinking signature downgrade.
/// These proxies validate the Anthropic thinking block schema strictly
/// and reject blocks missing the `signature` field.
/// New domains can be added here without user configuration.
const THINKING_SIGNATURE_PROXY_DOMAINS: &[&str] = &["openrouter.ai", "api.deepseek.com"];

/// Check whether a provider's base URL belongs to a known proxy that requires
/// thinking blocks to carry a valid signature. When true, thinking blocks
/// without a signature should be downgraded to plain text blocks.
pub fn requires_thinking_signature_downgrade(base_url: &str) -> bool {
    let lower = base_url.to_ascii_lowercase();
    THINKING_SIGNATURE_PROXY_DOMAINS
        .iter()
        .any(|domain| lower.contains(*domain))
}

/// Providers whose API supports OpenAI Responses API "custom" tool type
/// with Lark grammar constraints. For these, apply_patch uses the
/// Freeform variant. All others get the Function variant.
const APPLY_PATCH_FREEFORM_DOMAINS: &[&str] = &["api.openai.com", "api.aiservice.ai.azure.com"];

impl ModelProviderInfo {
    /// Check whether this provider supports apply_patch as a Freeform
    /// (Lark grammar) tool.
    pub fn supports_apply_patch_freeform(&self) -> bool {
        // If base_url is not set, check if this is the default OpenAI provider.
        // Default OpenAI uses api.openai.com which supports Freeform tools.
        let base_url = self.base_url.as_deref().unwrap_or("");
        if base_url.is_empty() {
            return self.is_openai();
        }
        let lower = base_url.to_ascii_lowercase();
        APPLY_PATCH_FREEFORM_DOMAINS
            .iter()
            .any(|domain| lower.contains(*domain))
    }
}

#[cfg(test)]
#[path = "model_provider_info_tests.rs"]
mod tests;
