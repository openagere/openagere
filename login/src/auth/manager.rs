use agere_config;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::sync::RwLock;

pub use crate::auth::storage::AuthDotJson;
use crate::auth::storage::create_auth_storage;

/// Authentication mechanism: API key only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyAuth {
    pub api_key: String,
}

/// Simplified: only supports API Key authentication.
#[derive(Debug, Clone)]
pub enum AgereAuth {
    ApiKey(ApiKeyAuth),
    /// Stub — kept for API compatibility, never constructed.
    AgentIdentity(AgentIdentityAuth),
}

/// Stub type for AgentIdentity auth, kept for API compatibility.
#[derive(Debug, Clone)]
pub struct AgentIdentityAuth;

impl PartialEq for AgereAuth {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::ApiKey(a), Self::ApiKey(b)) => a.api_key == b.api_key,
            (Self::AgentIdentity(_), Self::AgentIdentity(_)) => true,
            _ => false,
        }
    }
}

pub const OPENAI_API_KEY_ENV_VAR: &str = "OPENAI_API_KEY";
pub const AGERE_API_KEY_ENV_VAR: &str = "AGERE_API_KEY";

pub fn read_openai_api_key_from_env() -> Option<String> {
    env::var(OPENAI_API_KEY_ENV_VAR)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn read_agere_api_key_from_env() -> Option<String> {
    env::var(AGERE_API_KEY_ENV_VAR)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

impl AgereAuth {
    pub fn from_api_key(api_key: &str) -> Self {
        Self::ApiKey(ApiKeyAuth {
            api_key: api_key.to_owned(),
        })
    }

    /// Stable test double used across workspace integration tests.
    pub fn create_dummy_chatgpt_auth_for_testing() -> Self {
        Self::from_api_key("dummy-chatgpt-auth-for-testing")
    }

    /// Load auth from storage (simplified — only checks API key).
    pub async fn from_auth_storage(
        agere_home: &Path,
        _auth_credentials_store_mode: agere_config::types::AuthCredentialsStoreMode,
        _chatgpt_base_url: Option<&String>,
    ) -> std::io::Result<Option<Self>> {
        load_auth(agere_home, true).await
    }

    /// Returns the API key.
    pub fn api_key(&self) -> Option<&str> {
        match self {
            Self::ApiKey(auth) => Some(auth.api_key.as_str()),
            Self::AgentIdentity(_) => None,
        }
    }

    /// Returns the token string used for bearer authentication (the API key).
    pub fn get_token(&self) -> Result<String, std::io::Error> {
        match self {
            Self::ApiKey(auth) => Ok(auth.api_key.clone()),
            Self::AgentIdentity(_) => Err(std::io::Error::other("AgentIdentity is not supported")),
        }
    }

    /// Returns `None` — API key auth does not expose account info.
    pub fn get_account_id(&self) -> Option<String> {
        None
    }

    /// Returns `None` — API key auth does not expose account email.
    pub fn get_account_email(&self) -> Option<String> {
        None
    }

    /// Returns `None` — API key auth does not expose ChatGPT user id.
    pub fn get_chatgpt_user_id(&self) -> Option<String> {
        None
    }

    /// Returns `None` — API key auth does not expose plan type.
    pub fn account_plan_type(&self) -> Option<agere_protocol::account::PlanType> {
        None
    }

    pub fn is_workspace_account(&self) -> bool {
        false
    }

    /// Returns false — API key auth does not use the Agere backend.
    pub fn uses_agere_backend(&self) -> bool {
        false
    }

    /// Returns true — API key auth is the only auth mode.
    pub fn is_api_key_auth(&self) -> bool {
        true
    }

    /// Returns false — no ChatGPT auth in simplified auth.
    pub fn is_chatgpt_auth(&self) -> bool {
        false
    }

    /// Returns false — API key auth is not FedRAMP.
    pub fn is_fedramp_account(&self) -> bool {
        false
    }

    /// Returns `AuthMode::ApiKey` — the only auth mode.
    pub fn auth_mode(&self) -> AuthMode {
        AuthMode::ApiKey
    }

    /// Returns `ApiAuthMode::ApiKey` — the only auth mode.
    pub fn api_auth_mode(&self) -> ApiAuthMode {
        ApiAuthMode::ApiKey
    }
}

/// Writes an `auth.json` that contains only the API key.
pub fn login_with_api_key(
    agere_home: &Path,
    api_key: &str,
    _store_mode: agere_config::types::AuthCredentialsStoreMode,
) -> std::io::Result<()> {
    let auth_dot_json = AuthDotJson {
        openai_api_key: Some(api_key.to_string()),
    };
    let storage = create_auth_storage(agere_home.to_path_buf());
    storage.save(&auth_dot_json)
}

/// Delete the auth.json file inside `agere_home` if it exists.
pub fn logout(agere_home: &Path) -> std::io::Result<bool> {
    let storage = create_auth_storage(agere_home.to_path_buf());
    storage.delete()
}

/// Persist the provided auth payload.
pub fn save_auth(agere_home: &Path, auth: &AuthDotJson) -> std::io::Result<()> {
    let storage = create_auth_storage(agere_home.to_path_buf());
    storage.save(auth)
}

/// Load CLI auth data. Returns `None` when no credentials are stored.
pub fn load_auth_dot_json(agere_home: &Path) -> std::io::Result<Option<AuthDotJson>> {
    let storage = create_auth_storage(agere_home.to_path_buf());
    storage.load()
}

/// Load auth from storage and env vars.
async fn load_auth(
    agere_home: &Path,
    enable_agere_api_key_env: bool,
) -> std::io::Result<Option<AgereAuth>> {
    // API key via env var takes precedence.
    if enable_agere_api_key_env && let Some(api_key) = read_agere_api_key_from_env() {
        return Ok(Some(AgereAuth::from_api_key(api_key.as_str())));
    }

    // Fall back to file storage.
    let storage = create_auth_storage(agere_home.to_path_buf());
    let auth_dot_json = match storage.load()? {
        Some(auth) => auth,
        None => return Ok(None),
    };

    let Some(api_key) = auth_dot_json.openai_api_key.as_deref() else {
        return Ok(None);
    };

    Ok(Some(AgereAuth::from_api_key(api_key)))
}

/// Central manager providing auth loading and caching.
pub struct AuthManager {
    agere_home: PathBuf,
    inner: RwLock<Option<AgereAuth>>,
    enable_agere_api_key_env: bool,
}

/// Configuration view required to construct a shared [`AuthManager`].
pub trait AuthManagerConfig {
    fn agere_home(&self) -> PathBuf;
    fn cli_auth_credentials_store_mode(&self) -> agere_config::types::AuthCredentialsStoreMode;
    fn forced_chatgpt_workspace_id(&self) -> Option<String>;
    fn chatgpt_base_url(&self) -> String;
}

impl std::fmt::Debug for AuthManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthManager")
            .field("agere_home", &self.agere_home)
            .field("inner", &self.inner)
            .field("enable_agere_api_key_env", &self.enable_agere_api_key_env)
            .finish_non_exhaustive()
    }
}

impl AuthManager {
    pub async fn new(
        agere_home: PathBuf,
        enable_agere_api_key_env: bool,
        _auth_credentials_store_mode: agere_config::types::AuthCredentialsStoreMode,
        _chatgpt_base_url: Option<String>,
    ) -> Self {
        let managed_auth = load_auth(&agere_home, enable_agere_api_key_env)
            .await
            .ok()
            .flatten();
        Self {
            agere_home,
            inner: RwLock::new(managed_auth),
            enable_agere_api_key_env,
        }
    }

    pub fn from_auth_for_testing(auth: AgereAuth) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            agere_home: PathBuf::from("non-existent"),
            inner: RwLock::new(Some(auth)),
            enable_agere_api_key_env: false,
        })
    }

    pub fn from_auth_for_testing_with_home(
        auth: AgereAuth,
        agere_home: PathBuf,
    ) -> std::sync::Arc<Self> {
        std::sync::Arc::new(Self {
            agere_home,
            inner: RwLock::new(Some(auth)),
            enable_agere_api_key_env: false,
        })
    }

    /// Current cached auth (clone) without attempting a refresh.
    pub fn auth_cached(&self) -> Option<AgereAuth> {
        self.inner.read().ok().and_then(|c| c.clone())
    }

    /// Current cached auth (clone).
    pub async fn auth(&self) -> Option<AgereAuth> {
        self.auth_cached()
    }

    /// Force a reload of the auth information from auth.json.
    pub async fn reload(&self) -> bool {
        let new_auth = self.load_auth_from_storage().await;
        self.set_cached_auth(new_auth)
    }

    async fn load_auth_from_storage(&self) -> Option<AgereAuth> {
        load_auth(&self.agere_home, self.enable_agere_api_key_env)
            .await
            .ok()
            .flatten()
    }

    fn set_cached_auth(&self, new_auth: Option<AgereAuth>) -> bool {
        if let Ok(mut guard) = self.inner.write() {
            let previous = guard.as_ref();
            let changed = previous != new_auth.as_ref();
            *guard = new_auth;
            changed
        } else {
            false
        }
    }

    /// Convenience constructor returning an `Arc` wrapper.
    pub async fn shared(
        agere_home: PathBuf,
        enable_agere_api_key_env: bool,
        auth_credentials_store_mode: agere_config::types::AuthCredentialsStoreMode,
        chatgpt_base_url: Option<String>,
    ) -> std::sync::Arc<Self> {
        std::sync::Arc::new(
            Self::new(
                agere_home,
                enable_agere_api_key_env,
                auth_credentials_store_mode,
                chatgpt_base_url,
            )
            .await,
        )
    }

    /// Convenience constructor returning an `Arc` wrapper from resolved config.
    pub async fn shared_from_config(
        config: &impl AuthManagerConfig,
        enable_agere_api_key_env: bool,
    ) -> std::sync::Arc<Self> {
        Self::shared(
            config.agere_home(),
            enable_agere_api_key_env,
            config.cli_auth_credentials_store_mode(),
            Some(config.chatgpt_base_url()),
        )
        .await
    }

    /// Log out by deleting the on-disk auth.json.
    pub async fn logout(&self) -> std::io::Result<bool> {
        use crate::auth::storage::delete_file_if_exists;
        let removed = delete_file_if_exists(&self.agere_home)?;
        self.reload().await;
        Ok(removed)
    }

    /// Stub — same as logout since we don't have revoke support.
    pub async fn logout_with_revoke(&self) -> std::io::Result<bool> {
        self.logout().await
    }

    /// Stub — always fails since API keys don't refresh.
    pub async fn refresh_token(&self) -> Result<(), RefreshTokenError> {
        Err(RefreshTokenError::Transient(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "token refresh is not supported in simplified auth",
        )))
    }

    pub fn auth_mode(&self) -> Option<AuthMode> {
        self.auth_cached().map(|_| AuthMode::ApiKey)
    }

    pub fn get_api_auth_mode(&self) -> Option<ApiAuthMode> {
        self.auth_cached().map(|_| ApiAuthMode::ApiKey)
    }

    pub fn current_auth_uses_agere_backend(&self) -> bool {
        false
    }

    pub fn agere_api_key_env_enabled(&self) -> bool {
        self.enable_agere_api_key_env
    }

    pub fn forced_chatgpt_workspace_id(&self) -> Option<String> {
        None
    }

    pub fn set_forced_chatgpt_workspace_id(&self, _workspace_id: Option<String>) {}

    pub fn has_external_auth(&self) -> bool {
        false
    }

    pub fn is_external_chatgpt_auth_active(&self) -> bool {
        false
    }

    pub fn set_external_auth(&self, _external_auth: std::sync::Arc<dyn ExternalAuth>) {}

    pub fn clear_external_auth(&self) {}

    /// Stub — always returns None since we don't support refresh.
    pub fn refresh_failure_for_auth(&self, _auth: &AgereAuth) -> Option<String> {
        None
    }

    pub fn unauthorized_recovery(self: &std::sync::Arc<Self>) -> UnauthorizedRecovery {
        UnauthorizedRecovery::new(std::sync::Arc::clone(self))
    }
}

/// Simplified UnauthorizedRecovery — always indicates no recovery possible
/// (API keys don't expire and don't need refresh).
pub struct UnauthorizedRecovery {
    #[allow(dead_code)]
    manager: std::sync::Arc<AuthManager>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnauthorizedRecoveryStepResult {
    auth_state_changed: Option<bool>,
}

impl UnauthorizedRecoveryStepResult {
    pub fn auth_state_changed(&self) -> Option<bool> {
        self.auth_state_changed
    }
}

impl UnauthorizedRecovery {
    fn new(manager: std::sync::Arc<AuthManager>) -> Self {
        Self { manager }
    }

    pub fn has_next(&self) -> bool {
        false
    }

    pub fn unavailable_reason(&self) -> &'static str {
        "not_chatgpt_auth"
    }

    pub fn mode_name(&self) -> &'static str {
        "managed"
    }

    pub fn step_name(&self) -> &'static str {
        "done"
    }

    pub async fn next(&mut self) -> Result<UnauthorizedRecoveryStepResult, RefreshTokenError> {
        Err(RefreshTokenError::Permanent(
            agere_protocol::auth::RefreshTokenFailedError::new(
                agere_protocol::auth::RefreshTokenFailedReason::Other,
                "API key auth does not support token recovery.",
            ),
        ))
    }
}

/// Use agere_app_server_protocol::AuthMode directly.
pub use agere_app_server_protocol::AuthMode;
pub use agere_app_server_protocol::AuthMode as ApiAuthMode;

/// Simplified error — only permanent errors are possible for API key auth.
pub use agere_protocol::auth::RefreshTokenFailedError;
pub use agere_protocol::auth::RefreshTokenFailedReason;

#[derive(Debug)]
pub enum RefreshTokenError {
    Permanent(RefreshTokenFailedError),
    Transient(std::io::Error),
}

impl std::fmt::Display for RefreshTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Permanent(e) => write!(f, "{e}"),
            Self::Transient(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RefreshTokenError {}

impl RefreshTokenError {
    pub fn failed_reason(&self) -> Option<RefreshTokenFailedReason> {
        match self {
            Self::Permanent(error) => Some(error.reason),
            Self::Transient(_) => None,
        }
    }
}

impl From<RefreshTokenError> for std::io::Error {
    fn from(err: RefreshTokenError) -> Self {
        match err {
            RefreshTokenError::Permanent(failed) => std::io::Error::other(failed),
            RefreshTokenError::Transient(inner) => inner,
        }
    }
}

/// Shared constant (kept for compatibility).
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

/// ExternalAuth trait — stubbed for compatibility.
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalAuthTokens {
    pub access_token: String,
    pub chatgpt_metadata: Option<ExternalAuthChatgptMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalAuthChatgptMetadata {
    pub account_id: String,
    pub plan_type: Option<String>,
}

impl ExternalAuthTokens {
    pub fn access_token_only(access_token: impl Into<String>) -> Self {
        Self {
            access_token: access_token.into(),
            chatgpt_metadata: None,
        }
    }

    pub fn chatgpt(
        access_token: impl Into<String>,
        chatgpt_account_id: impl Into<String>,
        chatgpt_plan_type: Option<String>,
    ) -> Self {
        Self {
            access_token: access_token.into(),
            chatgpt_metadata: Some(ExternalAuthChatgptMetadata {
                account_id: chatgpt_account_id.into(),
                plan_type: chatgpt_plan_type,
            }),
        }
    }

    pub fn chatgpt_metadata(&self) -> Option<&ExternalAuthChatgptMetadata> {
        self.chatgpt_metadata.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalAuthRefreshReason {
    Unauthorized,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalAuthRefreshContext {
    pub reason: ExternalAuthRefreshReason,
    pub previous_account_id: Option<String>,
}

#[async_trait]
pub trait ExternalAuth: Send + Sync {
    fn auth_mode(&self) -> AuthMode;

    async fn resolve(&self) -> std::io::Result<Option<ExternalAuthTokens>> {
        Ok(None)
    }

    async fn refresh(
        &self,
        _context: ExternalAuthRefreshContext,
    ) -> std::io::Result<ExternalAuthTokens>;
}

/// Enforce login restrictions — simplified for API key only.
pub struct AuthConfig {
    pub agere_home: PathBuf,
    pub auth_credentials_store_mode: agere_config::types::AuthCredentialsStoreMode,
    pub forced_login_method: Option<agere_protocol::config_types::ForcedLoginMethod>,
    pub forced_chatgpt_workspace_id: Option<String>,
    pub chatgpt_base_url: Option<String>,
}

pub async fn enforce_login_restrictions(config: &AuthConfig) -> std::io::Result<()> {
    let Some(_auth) = load_auth(&config.agere_home, /*enable_agere_api_key_env*/ true).await?
    else {
        return Ok(());
    };

    // In API-key-only mode, forced ChatGPT login is always a mismatch.
    if let Some(agere_protocol::config_types::ForcedLoginMethod::Chatgpt) =
        config.forced_login_method
    {
        return Err(std::io::Error::other(
            "ChatGPT login is required, but an API key is currently being used. Logging out.",
        ));
    }

    // forced_chatgpt_workspace_id is irrelevant for API key auth
    Ok(())
}
