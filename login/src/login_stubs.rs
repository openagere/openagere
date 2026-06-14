// Stub types/function for login server related code removed during
// auth simplification. These are needed for API compatibility.

use crate::auth::RefreshTokenError;
use std::io;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub open_browser: bool,
    pub issuer: String,
    pub scopes: Vec<String>,
    pub client_id: String,
    pub base_url: String,
    pub listen: SocketAddr,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            open_browser: false,
            issuer: String::new(),
            scopes: Vec::new(),
            client_id: String::new(),
            base_url: String::new(),
            listen: "127.0.0.1:0".parse().unwrap(),
        }
    }
}

impl ServerOptions {
    pub fn new(
        _agere_home: PathBuf,
        client_id: String,
        _forced_chatgpt_workspace_id: Option<String>,
        _auth_credentials_store_mode: agere_config::types::AuthCredentialsStoreMode,
    ) -> Self {
        Self {
            client_id,
            ..Default::default()
        }
    }
}

/// Stub struct returned by run_login_server, mimicking the original login server.
#[derive(Debug, Clone)]
pub struct LoginServer {
    pub auth_url: String,
    pub actual_port: u16,
}

impl LoginServer {
    pub fn cancel_handle(&self) -> ShutdownHandle {
        ShutdownHandle
    }
    pub async fn block_until_done(&self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "login server is not supported in this build",
        ))
    }
}

#[derive(Debug, Clone)]
pub struct ShutdownHandle;

impl Default for ShutdownHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl ShutdownHandle {
    pub fn new() -> Self {
        Self
    }
    pub fn shutdown(&self) {}
}

pub async fn request_device_code(_options: &ServerOptions) -> io::Result<DeviceCode> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "device code login is not supported in this build",
    ))
}

#[derive(Debug)]
pub struct DeviceCode {
    pub device_code: String,
    pub verification_url: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
}

pub async fn complete_device_code_login(
    _options: ServerOptions,
    _device_code: DeviceCode,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "device code login is not supported in this build",
    ))
}

pub fn run_login_server(_options: ServerOptions) -> io::Result<LoginServer> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "login server is not supported in this build",
    ))
}

pub fn login_with_chatgpt_auth_tokens(
    _agere_home: &Path,
    _access_token: &String,
    _chatgpt_account_id: &String,
    _chatgpt_plan_type: Option<&str>,
) -> Result<(), RefreshTokenError> {
    Err(RefreshTokenError::Transient(io::Error::new(
        io::ErrorKind::Unsupported,
        "chatgpt login is not supported in this build",
    )))
}

/// Stub — agent identity login is not supported.
pub async fn login_with_agent_identity(
    _agere_home: &Path,
    _agent_token: &str,
    _auth_credentials_store_mode: agere_config::types::AuthCredentialsStoreMode,
    _chatgpt_base_url: Option<&String>,
) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "agent identity login is not supported in this build",
    ))
}

/// Stub — same as logout since we don't have revoke support.
pub async fn logout_with_revoke(
    agere_home: &Path,
    _auth_credentials_store_mode: agere_config::types::AuthCredentialsStoreMode,
) -> std::io::Result<bool> {
    crate::auth::logout(agere_home)
}

/// Stub — device code login is not supported.
pub async fn run_device_code_login(_opts: ServerOptions) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "device code login is not supported in this build",
    ))
}
