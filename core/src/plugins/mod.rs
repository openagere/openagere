use agere_config::types::McpServerConfig;

mod discoverable;
mod injection;
mod manager;
mod mentions;
mod render;
mod startup_sync;
#[cfg(test)]
pub(crate) mod test_support;

pub use agere_core_plugins::marketplace_upgrade::ConfiguredMarketplaceUpgradeError as PluginMarketplaceUpgradeError;
pub use agere_core_plugins::marketplace_upgrade::ConfiguredMarketplaceUpgradeOutcome as PluginMarketplaceUpgradeOutcome;
pub use agere_plugin::AppConnectorId;
pub use agere_plugin::EffectiveSkillRoots;
pub use agere_plugin::PluginCapabilitySummary;
pub use agere_plugin::PluginId;
pub use agere_plugin::PluginIdError;
pub use agere_plugin::PluginTelemetryMetadata;
pub use agere_plugin::validate_plugin_segment;

pub type LoadedPlugin = agere_plugin::LoadedPlugin<McpServerConfig>;
pub type PluginLoadOutcome = agere_plugin::PluginLoadOutcome<McpServerConfig>;

pub(crate) use discoverable::list_tool_suggest_discoverable_plugins;
pub(crate) use injection::build_plugin_injections;
pub use manager::ConfiguredMarketplace;
pub use manager::ConfiguredMarketplaceListOutcome;
pub use manager::ConfiguredMarketplacePlugin;
pub use manager::PluginDetail;
pub use manager::PluginDetailsUnavailableReason;
pub use manager::PluginInstallError;
pub use manager::PluginInstallOutcome;
pub use manager::PluginInstallRequest;
pub use manager::PluginReadOutcome;
pub use manager::PluginReadRequest;
pub use manager::PluginRemoteSyncError;
pub use manager::PluginUninstallError;
pub use manager::PluginsManager;
pub use manager::RemotePluginSyncResult;
pub(crate) use render::render_explicit_plugin_instructions;

pub(crate) use mentions::build_connector_slug_counts;
pub(crate) use mentions::build_skill_name_counts;
pub(crate) use mentions::collect_explicit_app_ids;
pub(crate) use mentions::collect_explicit_plugin_mentions;
pub(crate) use mentions::collect_tool_mentions_from_messages;
