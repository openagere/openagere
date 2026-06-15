//! Agere Apps support for the built-in apps MCP server.
//!
//! This module owns the pieces that are unique to ChatGPT-hosted app
//! connectors: cache scoping by authenticated user, disk cache reads/writes,
//! connector allow-list filtering, and the normalization that turns app
//! connector/tool metadata into model-visible MCP callable names.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::mcp::AGERE_APPS_MCP_SERVER_NAME;
use crate::runtime::emit_duration;
use crate::tools::MCP_TOOLS_CACHE_WRITE_DURATION_METRIC;
use crate::tools::ToolInfo;
use agere_login::AgereAuth;
use agere_utils_plugins_placeholder::mcp_connector::is_connector_id_allowed;
use agere_utils_plugins_placeholder::mcp_connector::sanitize_name;
use serde::Deserialize;
use serde::Serialize;
use sha1::Digest;
use sha1::Sha1;

pub(crate) const AGERE_APPS_TOOLS_CACHE_SCHEMA_VERSION: u8 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgereAppsToolsCacheKey {
    pub(crate) account_id: Option<String>,
    pub(crate) chatgpt_user_id: Option<String>,
    pub(crate) is_workspace_account: bool,
}

pub fn agere_apps_tools_cache_key(auth: Option<&AgereAuth>) -> AgereAppsToolsCacheKey {
    AgereAppsToolsCacheKey {
        account_id: auth.and_then(AgereAuth::get_account_id),
        chatgpt_user_id: auth.and_then(AgereAuth::get_chatgpt_user_id),
        is_workspace_account: auth.is_some_and(AgereAuth::is_workspace_account),
    }
}

pub fn filter_non_agere_apps_mcp_tools_only(
    mcp_tools: &HashMap<String, ToolInfo>,
) -> HashMap<String, ToolInfo> {
    mcp_tools
        .iter()
        .filter(|(_, tool)| tool.server_name != AGERE_APPS_MCP_SERVER_NAME)
        .map(|(name, tool)| (name.clone(), tool.clone()))
        .collect()
}

#[derive(Clone)]
pub(crate) struct AgereAppsToolsCacheContext {
    pub(crate) agere_home: PathBuf,
    pub(crate) user_key: AgereAppsToolsCacheKey,
}

impl AgereAppsToolsCacheContext {
    pub(crate) fn cache_path(&self) -> PathBuf {
        let user_key_json = serde_json::to_string(&self.user_key).unwrap_or_default();
        let user_key_hash = sha1_hex(&user_key_json);
        self.agere_home
            .join(AGERE_APPS_TOOLS_CACHE_DIR)
            .join(format!("{user_key_hash}.json"))
    }
}

pub(crate) enum CachedAgereAppsToolsLoad {
    Hit(Vec<ToolInfo>),
    Missing,
    Invalid,
}

pub(crate) fn normalize_agere_apps_tool_title(
    server_name: &str,
    connector_name: Option<&str>,
    value: &str,
) -> String {
    if server_name != AGERE_APPS_MCP_SERVER_NAME {
        return value.to_string();
    }

    let Some(connector_name) = connector_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
    else {
        return value.to_string();
    };

    let prefix = format!("{connector_name}_");
    if let Some(stripped) = value.strip_prefix(&prefix)
        && !stripped.is_empty()
    {
        return stripped.to_string();
    }

    value.to_string()
}

pub(crate) fn normalize_agere_apps_callable_name(
    server_name: &str,
    tool_name: &str,
    connector_id: Option<&str>,
    connector_name: Option<&str>,
) -> String {
    if server_name != AGERE_APPS_MCP_SERVER_NAME {
        return tool_name.to_string();
    }

    let tool_name = sanitize_name(tool_name);

    if let Some(connector_name) = connector_name
        .map(str::trim)
        .map(sanitize_name)
        .filter(|name| !name.is_empty())
        && let Some(stripped) = tool_name.strip_prefix(&connector_name)
        && !stripped.is_empty()
    {
        return stripped.to_string();
    }

    if let Some(connector_id) = connector_id
        .map(str::trim)
        .map(sanitize_name)
        .filter(|name| !name.is_empty())
        && let Some(stripped) = tool_name.strip_prefix(&connector_id)
        && !stripped.is_empty()
    {
        return stripped.to_string();
    }

    tool_name
}

pub(crate) fn normalize_agere_apps_callable_namespace(
    server_name: &str,
    connector_name: Option<&str>,
) -> String {
    if server_name == AGERE_APPS_MCP_SERVER_NAME
        && let Some(connector_name) = connector_name
    {
        format!("mcp__{}__{}", server_name, sanitize_name(connector_name))
    } else {
        format!("mcp__{server_name}__")
    }
}

pub(crate) fn write_cached_agere_apps_tools_if_needed(
    server_name: &str,
    cache_context: Option<&AgereAppsToolsCacheContext>,
    tools: &[ToolInfo],
) {
    if server_name != AGERE_APPS_MCP_SERVER_NAME {
        return;
    }

    if let Some(cache_context) = cache_context {
        let cache_write_start = Instant::now();
        write_cached_agere_apps_tools(cache_context, tools);
        emit_duration(
            MCP_TOOLS_CACHE_WRITE_DURATION_METRIC,
            cache_write_start.elapsed(),
            &[],
        );
    }
}

pub(crate) fn load_startup_cached_agere_apps_tools_snapshot(
    server_name: &str,
    cache_context: Option<&AgereAppsToolsCacheContext>,
) -> Option<Vec<ToolInfo>> {
    if server_name != AGERE_APPS_MCP_SERVER_NAME {
        return None;
    }

    let cache_context = cache_context?;

    match load_cached_agere_apps_tools(cache_context) {
        CachedAgereAppsToolsLoad::Hit(tools) => Some(tools),
        CachedAgereAppsToolsLoad::Missing | CachedAgereAppsToolsLoad::Invalid => None,
    }
}

#[cfg(test)]
pub(crate) fn read_cached_agere_apps_tools(
    cache_context: &AgereAppsToolsCacheContext,
) -> Option<Vec<ToolInfo>> {
    match load_cached_agere_apps_tools(cache_context) {
        CachedAgereAppsToolsLoad::Hit(tools) => Some(tools),
        CachedAgereAppsToolsLoad::Missing | CachedAgereAppsToolsLoad::Invalid => None,
    }
}

pub(crate) fn load_cached_agere_apps_tools(
    cache_context: &AgereAppsToolsCacheContext,
) -> CachedAgereAppsToolsLoad {
    let cache_path = cache_context.cache_path();
    let bytes = match std::fs::read(cache_path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return CachedAgereAppsToolsLoad::Missing;
        }
        Err(_) => return CachedAgereAppsToolsLoad::Invalid,
    };
    let cache: AgereAppsToolsDiskCache = match serde_json::from_slice(&bytes) {
        Ok(cache) => cache,
        Err(_) => return CachedAgereAppsToolsLoad::Invalid,
    };
    if cache.schema_version != AGERE_APPS_TOOLS_CACHE_SCHEMA_VERSION {
        return CachedAgereAppsToolsLoad::Invalid;
    }
    CachedAgereAppsToolsLoad::Hit(filter_disallowed_agere_apps_tools(cache.tools))
}

pub(crate) fn write_cached_agere_apps_tools(
    cache_context: &AgereAppsToolsCacheContext,
    tools: &[ToolInfo],
) {
    let cache_path = cache_context.cache_path();
    if let Some(parent) = cache_path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        return;
    }
    let tools = filter_disallowed_agere_apps_tools(tools.to_vec());
    let Ok(bytes) = serde_json::to_vec_pretty(&AgereAppsToolsDiskCache {
        schema_version: AGERE_APPS_TOOLS_CACHE_SCHEMA_VERSION,
        tools,
    }) else {
        return;
    };
    let _ = std::fs::write(cache_path, bytes);
}

pub(crate) fn filter_disallowed_agere_apps_tools(tools: Vec<ToolInfo>) -> Vec<ToolInfo> {
    tools
        .into_iter()
        .filter(|tool| {
            tool.connector_id
                .as_deref()
                .is_none_or(is_connector_id_allowed)
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgereAppsToolsDiskCache {
    schema_version: u8,
    tools: Vec<ToolInfo>,
}

const AGERE_APPS_TOOLS_CACHE_DIR: &str = "cache/agere_apps_tools";

fn sha1_hex(s: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(s.as_bytes());
    let sha1 = hasher.finalize();
    format!("{sha1:x}")
}
