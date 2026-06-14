//! Runtime support for Model Context Protocol (MCP) servers.
//!
//! This module contains data that describes the runtime environment in which MCP
//! servers execute, plus the execution state payload sent to capable servers and a
//! tiny shared metrics helper. Transport startup and orchestration live in
//! [`crate::rmcp_client`] and [`crate::connection_manager`].

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use agere_exec_server::Environment;
use agere_protocol::models::PermissionProfile;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_profile: Option<PermissionProfile>,
    pub access_policy: String,
    pub agere_linux_exe: Option<PathBuf>,
    pub access_cwd: PathBuf,
    #[serde(default)]
    pub use_legacy_landlock: bool,
}

/// Runtime placement information used when starting MCP server transports.
///
/// `McpConfig` describes what servers exist. This value describes where those
/// servers should run for the current caller. Keep it explicit at manager
/// construction time so status/snapshot paths and real sessions make the same
/// local-vs-remote decision. `fallback_cwd` is not a per-server override; it is
/// used when a stdio server omits `cwd` and the launcher needs a concrete
/// process working directory.
#[derive(Clone)]
pub struct McpRuntimeEnvironment {
    environment: Arc<Environment>,
    fallback_cwd: PathBuf,
}

impl McpRuntimeEnvironment {
    pub fn new(environment: Arc<Environment>, fallback_cwd: PathBuf) -> Self {
        Self {
            environment,
            fallback_cwd,
        }
    }

    pub(crate) fn environment(&self) -> Arc<Environment> {
        Arc::clone(&self.environment)
    }

    pub(crate) fn fallback_cwd(&self) -> PathBuf {
        self.fallback_cwd.clone()
    }
}

pub(crate) fn emit_duration(metric: &str, duration: Duration, tags: &[(&str, &str)]) {
    if let Some(metrics) = agere_otel::global() {
        let _ = metrics.record_duration(metric, duration, tags);
    }
}
