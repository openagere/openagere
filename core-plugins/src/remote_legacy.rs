use crate::remote::RemotePluginServiceConfig;
use agere_login::AgereAuth;
use agere_protocol::protocol::Product;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RemotePluginStatusSummary {
    pub name: String,
    #[serde(default = "default_remote_marketplace_name")]
    pub marketplace_name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct RemotePluginMutationResponse {
    pub id: String,
    pub enabled: bool,
}

fn default_remote_marketplace_name() -> String {
    "openagere-curated".to_string()
}

#[derive(Debug, thiserror::Error)]
pub enum RemotePluginMutationError {
    #[error("chatgpt authentication required for remote plugin mutation")]
    AuthRequired,

    #[error(
        "chatgpt authentication required for remote plugin mutation; api key auth is not supported"
    )]
    UnsupportedAuthMode,

    #[error("failed to read auth token for remote plugin mutation: {0}")]
    AuthToken(#[source] std::io::Error),

    #[error("invalid chatgpt base url for remote plugin mutation: {0}")]
    InvalidBaseUrl(#[source] url::ParseError),

    #[error("chatgpt base url cannot be used for plugin mutation")]
    InvalidBaseUrlPath,

    #[error("failed to send remote plugin mutation request to {url}: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("remote plugin mutation failed with status {status} from {url}: {body}")]
    UnexpectedStatus {
        url: String,
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("failed to parse remote plugin mutation response from {url}: {source}")]
    Decode {
        url: String,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "remote plugin mutation returned unexpected plugin id: expected `{expected}`, got `{actual}`"
    )]
    UnexpectedPluginId { expected: String, actual: String },

    #[error(
        "remote plugin mutation returned unexpected enabled state for `{plugin_id}`: expected {expected_enabled}, got {actual_enabled}"
    )]
    UnexpectedEnabledState {
        plugin_id: String,
        expected_enabled: bool,
        actual_enabled: bool,
    },

    #[error("remote plugin access is not supported in this build")]
    Unsupported(String),
}

#[derive(Debug, thiserror::Error)]
pub enum RemotePluginFetchError {
    #[error("chatgpt authentication required to sync remote plugins")]
    AuthRequired,

    #[error(
        "chatgpt authentication required to sync remote plugins; api key auth is not supported"
    )]
    UnsupportedAuthMode,

    #[error("failed to read auth token for remote plugin sync: {0}")]
    AuthToken(#[source] std::io::Error),

    #[error("failed to send remote plugin sync request to {url}: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("remote plugin sync request to {url} failed with status {status}: {body}")]
    UnexpectedStatus {
        url: String,
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("failed to parse remote plugin sync response from {url}: {source}")]
    Decode {
        url: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("remote plugin access is not supported in this build")]
    Unsupported(String),
}

pub async fn fetch_remote_plugin_status(
    _config: &RemotePluginServiceConfig,
    _auth: Option<&AgereAuth>,
) -> Result<Vec<RemotePluginStatusSummary>, RemotePluginFetchError> {
    Err(RemotePluginFetchError::Unsupported(
        "remote plugin status fetch is not supported in this build".into(),
    ))
}

pub async fn fetch_remote_featured_plugin_ids(
    _config: &RemotePluginServiceConfig,
    _auth: Option<&AgereAuth>,
    _product: Option<Product>,
) -> Result<Vec<String>, RemotePluginFetchError> {
    Err(RemotePluginFetchError::Unsupported(
        "remote featured plugin fetch is not supported in this build".into(),
    ))
}

pub async fn enable_remote_plugin(
    _config: &RemotePluginServiceConfig,
    _auth: Option<&AgereAuth>,
    _plugin_id: &str,
) -> Result<(), RemotePluginMutationError> {
    Err(RemotePluginMutationError::Unsupported(
        "remote plugin enable is not supported in this build".into(),
    ))
}

pub async fn uninstall_remote_plugin(
    _config: &RemotePluginServiceConfig,
    _auth: Option<&AgereAuth>,
    _plugin_id: &str,
) -> Result<(), RemotePluginMutationError> {
    Err(RemotePluginMutationError::Unsupported(
        "remote plugin uninstall is not supported in this build".into(),
    ))
}
