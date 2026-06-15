use std::collections::HashSet;

use agere_app_server_protocol::AppInfo;
use agere_app_server_protocol::AppSummary;
use agere_core::config::Config;
use agere_core::plugins::AppConnectorId;
use agere_exec_server::EnvironmentManager;
use tracing::warn;

/// Stub: ChatGPT connectors removed. Returns empty plugin app summaries
/// since the connector metadata source is no longer available.
#[allow(unused_variables)]
pub(super) async fn load_plugin_app_summaries(
    config: &Config,
    plugin_apps: &[AppConnectorId],
    _environment_manager: &EnvironmentManager,
) -> Vec<AppSummary> {
    if plugin_apps.is_empty() {
        return Vec::new();
    }
    warn!(
        "load_plugin_app_summaries: ChatGPT connectors removed; returning empty summaries for {} plugin apps",
        plugin_apps.len()
    );
    plugin_apps
        .iter()
        .map(|id| AppSummary {
            id: id.0.clone(),
            name: id.0.clone(),
            description: None,
            install_url: None,
            needs_auth: true,
        })
        .collect()
}

#[allow(dead_code)]
pub(super) fn plugin_apps_needing_auth(
    all_connectors: &[AppInfo],
    accessible_connectors: &[AppInfo],
    plugin_apps: &[AppConnectorId],
    agere_apps_ready: bool,
) -> Vec<AppSummary> {
    if !agere_apps_ready {
        return Vec::new();
    }

    let accessible_ids = accessible_connectors
        .iter()
        .map(|connector| connector.id.as_str())
        .collect::<HashSet<_>>();
    let plugin_app_ids = plugin_apps
        .iter()
        .map(|connector_id| connector_id.0.as_str())
        .collect::<HashSet<_>>();

    all_connectors
        .iter()
        .filter(|connector| {
            plugin_app_ids.contains(connector.id.as_str())
                && !accessible_ids.contains(connector.id.as_str())
        })
        .cloned()
        .map(|connector| AppSummary {
            id: connector.id,
            name: connector.name,
            description: connector.description,
            install_url: connector.install_url,
            needs_auth: true,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use agere_app_server_protocol::AppInfo;
    use agere_core::plugins::AppConnectorId;
    use pretty_assertions::assert_eq;

    use super::plugin_apps_needing_auth;

    #[test]
    fn plugin_apps_needing_auth_returns_empty_when_agere_apps_is_not_ready() {
        let all_connectors = vec![AppInfo {
            id: "alpha".to_string(),
            name: "Alpha".to_string(),
            description: Some("Alpha connector".to_string()),
            logo_url: None,
            logo_url_dark: None,
            distribution_channel: None,
            branding: None,
            app_metadata: None,
            labels: None,
            install_url: Some("https://chatgpt.com/apps/alpha/alpha".to_string()),
            is_accessible: false,
            is_enabled: true,
            plugin_display_names: Vec::new(),
        }];

        assert_eq!(
            plugin_apps_needing_auth(
                &all_connectors,
                &[],
                &[AppConnectorId("alpha".to_string())],
                /*agere_apps_ready*/ false,
            ),
            Vec::new()
        );
    }
}
