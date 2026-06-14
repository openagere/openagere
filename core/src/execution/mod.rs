/*
Module: execution

Core-owned adapter types for exec/runtime plumbing. Managed OS isolation and
permission profiles live elsewhere; this module keeps the shared execution
request/error surface used by tool runtimes.
*/

use serde::Deserialize;
use serde::Serialize;

use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
pub(crate) use crate::exec::execute_exec_request;
use agere_network_proxy::NetworkProxy;
use agere_protocol::models::PermissionProfile;
use agere_utils_fs::AbsolutePathBuf;
use std::collections::HashMap;

/// Requested permission posture for tool executions (simplified).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ExecutionPermissionLevel {
    #[default]
    UseDefault,
    RequireEscalated,
    WithAdditionalPermissions,
}

impl ExecutionPermissionLevel {
    pub fn requires_escalated_permissions(&self) -> bool {
        matches!(self, Self::RequireEscalated)
    }

    pub fn uses_additional_permissions(&self) -> bool {
        matches!(self, Self::WithAdditionalPermissions)
    }

    pub fn requests_escalated_permissions(&self) -> bool {
        matches!(self, Self::WithAdditionalPermissions)
    }
}

/// Error type for managed execution adapter plumbing.
#[derive(Debug)]
pub enum ExecError {
    Transform(String),
    Io(std::io::Error),
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transform(msg) => write!(f, "managed execution transform error: {msg}"),
            Self::Io(err) => write!(f, "managed execution I/O error: {err}"),
        }
    }
}

impl std::error::Error for ExecError {}

impl From<std::io::Error> for ExecError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

#[derive(Debug)]
pub(crate) struct ExecOptions {
    pub(crate) expiration: ExecExpiration,
    pub(crate) capture_policy: ExecCapturePolicy,
}

#[derive(Clone, Debug)]
pub(crate) struct ExecServerEnvConfig {
    pub(crate) policy: agere_exec_server::ExecEnvPolicy,
    pub(crate) local_policy_env: HashMap<String, String>,
}

#[derive(Debug)]
pub struct ExecRequest {
    pub command: Vec<String>,
    pub cwd: AbsolutePathBuf,
    pub env: HashMap<String, String>,
    pub(crate) exec_server_env_config: Option<ExecServerEnvConfig>,
    pub network: Option<NetworkProxy>,
    pub expiration: ExecExpiration,
    pub capture_policy: ExecCapturePolicy,
    pub permission_profile: PermissionProfile,
    pub arg0: Option<String>,
}

impl ExecRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command: Vec<String>,
        cwd: AbsolutePathBuf,
        env: HashMap<String, String>,
        network: Option<NetworkProxy>,
        expiration: ExecExpiration,
        capture_policy: ExecCapturePolicy,
        permission_profile: PermissionProfile,
        arg0: Option<String>,
    ) -> Self {
        Self {
            command,
            cwd,
            env,
            exec_server_env_config: None,
            network,
            expiration,
            capture_policy,
            permission_profile,
            arg0,
        }
    }
}
