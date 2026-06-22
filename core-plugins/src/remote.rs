use agere_app_server_protocol::PluginAuthPolicy;
use agere_app_server_protocol::PluginInstallPolicy;
use agere_app_server_protocol::PluginInterface;
use agere_app_server_protocol::SkillInterface;
use std::path::PathBuf;

pub const REMOTE_GLOBAL_MARKETPLACE_NAME: &str = "chatgpt-global";
pub const REMOTE_WORKSPACE_MARKETPLACE_NAME: &str = "chatgpt-workspace";
pub const REMOTE_GLOBAL_MARKETPLACE_DISPLAY_NAME: &str = "ChatGPT Plugins";
pub const REMOTE_WORKSPACE_MARKETPLACE_DISPLAY_NAME: &str = "ChatGPT Workspace Plugins";

pub fn remote_plugin_backend_supported() -> bool {
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePluginServiceConfig {
    pub chatgpt_base_url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteMarketplace {
    pub name: String,
    pub display_name: String,
    pub plugins: Vec<RemotePluginSummary>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemotePluginSummary {
    pub id: String,
    pub name: String,
    pub installed: bool,
    pub enabled: bool,
    pub install_policy: PluginInstallPolicy,
    pub auth_policy: PluginAuthPolicy,
    pub interface: Option<PluginInterface>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemotePluginDetail {
    pub marketplace_name: String,
    pub marketplace_display_name: String,
    pub summary: RemotePluginSummary,
    pub description: Option<String>,
    pub release_version: Option<String>,
    pub bundle_download_url: Option<String>,
    pub skills: Vec<RemotePluginSkill>,
    pub app_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemotePluginSkill {
    pub name: String,
    pub description: String,
    pub short_description: Option<String>,
    pub interface: Option<SkillInterface>,
    pub enabled: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum RemotePluginCatalogError {
    #[error("chatgpt authentication required for remote plugin catalog")]
    AuthRequired,

    #[error(
        "chatgpt authentication required for remote plugin catalog; api key auth is not supported"
    )]
    UnsupportedAuthMode,

    #[error("failed to read auth token for remote plugin catalog: {0}")]
    AuthToken(#[source] std::io::Error),

    #[error("failed to send remote plugin catalog request to {url}: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("remote plugin catalog request to {url} failed with status {status}: {body}")]
    UnexpectedStatus {
        url: String,
        status: reqwest::StatusCode,
        body: String,
    },

    #[error("failed to parse remote plugin catalog response from {url}: {source}")]
    Decode {
        url: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("remote marketplace `{marketplace_name}` is not supported")]
    UnknownMarketplace { marketplace_name: String },

    #[error(
        "remote plugin `{plugin_id}` belongs to marketplace `{actual_marketplace_name}`, not `{expected_marketplace_name}`"
    )]
    MarketplaceMismatch {
        plugin_id: String,
        expected_marketplace_name: String,
        actual_marketplace_name: String,
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

    #[error("{0}")]
    CacheRemove(String),

    #[error("remote plugin access is not supported in this build")]
    Unsupported(String),
}

pub async fn fetch_remote_marketplaces(
    _config: &RemotePluginServiceConfig,
    _auth: Option<&agere_login::AgereAuth>,
) -> Result<Vec<RemoteMarketplace>, RemotePluginCatalogError> {
    Err(RemotePluginCatalogError::Unsupported(
        "remote marketplace access is not supported in this build".into(),
    ))
}

pub async fn fetch_remote_plugin_status(
    _config: &RemotePluginServiceConfig,
    _auth: Option<&agere_login::AgereAuth>,
) -> Result<
    Vec<super::remote_legacy::RemotePluginStatusSummary>,
    super::remote_legacy::RemotePluginFetchError,
> {
    Err(super::remote_legacy::RemotePluginFetchError::AuthRequired)
}

pub async fn fetch_remote_featured_plugin_ids(
    _config: &RemotePluginServiceConfig,
    _auth: Option<&agere_login::AgereAuth>,
    _product: Option<agere_protocol::protocol::Product>,
) -> Result<Vec<String>, super::remote_legacy::RemotePluginFetchError> {
    Err(super::remote_legacy::RemotePluginFetchError::AuthRequired)
}

pub async fn fetch_remote_plugin_detail(
    _config: &RemotePluginServiceConfig,
    _auth: Option<&agere_login::AgereAuth>,
    _marketplace_name: &str,
    _plugin_id: &str,
) -> Result<RemotePluginDetail, RemotePluginCatalogError> {
    Err(RemotePluginCatalogError::Unsupported(
        "remote plugin detail access is not supported in this build".into(),
    ))
}

pub async fn fetch_remote_plugin_detail_with_download_urls(
    _config: &RemotePluginServiceConfig,
    _auth: Option<&agere_login::AgereAuth>,
    _marketplace_name: &str,
    _plugin_id: &str,
) -> Result<RemotePluginDetail, RemotePluginCatalogError> {
    Err(RemotePluginCatalogError::Unsupported(
        "remote plugin detail access is not supported in this build".into(),
    ))
}

pub async fn install_remote_plugin(
    _config: &RemotePluginServiceConfig,
    _auth: Option<&agere_login::AgereAuth>,
    _marketplace_name: &str,
    _plugin_id: &str,
) -> Result<(), RemotePluginCatalogError> {
    Err(RemotePluginCatalogError::Unsupported(
        "remote plugin install is not supported in this build".into(),
    ))
}

pub async fn uninstall_remote_plugin(
    _config: &RemotePluginServiceConfig,
    _auth: Option<&agere_login::AgereAuth>,
    _agere_home: PathBuf,
    _plugin_id: &str,
) -> Result<(), RemotePluginCatalogError> {
    Err(RemotePluginCatalogError::Unsupported(
        "remote plugin uninstall is not supported in this build".into(),
    ))
}

pub async fn enable_remote_plugin(
    _config: &RemotePluginServiceConfig,
    _auth: Option<&agere_login::AgereAuth>,
    _plugin_id: &str,
) -> Result<(), RemotePluginCatalogError> {
    Err(RemotePluginCatalogError::Unsupported(
        "remote plugin enable is not supported in this build".into(),
    ))
}
