use crate::config::external_agent_config::ExternalAgentConfigDetectOptions;
use crate::config::external_agent_config::ExternalAgentConfigMigrationItem as CoreMigrationItem;
use crate::config::external_agent_config::ExternalAgentConfigMigrationItemType as CoreMigrationItemType;
use crate::config::external_agent_config::ExternalAgentConfigService;
use crate::config::external_agent_config::NamedMigration as CoreNamedMigration;
use crate::config::external_agent_config::PendingPluginImport;
use crate::error_code::internal_error;
use agere_app_server_protocol::CommandMigration;
use agere_app_server_protocol::ExternalAgentConfigDetectParams;
use agere_app_server_protocol::ExternalAgentConfigDetectResponse;
use agere_app_server_protocol::ExternalAgentConfigImportParams;
use agere_app_server_protocol::ExternalAgentConfigMigrationItem;
use agere_app_server_protocol::ExternalAgentConfigMigrationItemType;
use agere_app_server_protocol::HookMigration;
use agere_app_server_protocol::JSONRPCErrorError;
use agere_app_server_protocol::McpServerMigration;
use agere_app_server_protocol::MigrationDetails;
use agere_app_server_protocol::PluginsMigration;
use agere_app_server_protocol::SubagentMigration;
use std::path::PathBuf;

#[derive(Clone)]
pub(crate) struct ExternalAgentConfigApi {
    #[allow(dead_code)]
    agere_home: PathBuf,
    migration_service: ExternalAgentConfigService,
}

impl ExternalAgentConfigApi {
    pub(crate) fn new(agere_home: PathBuf) -> Self {
        Self {
            migration_service: ExternalAgentConfigService::new(agere_home.clone()),
            agere_home,
        }
    }

    pub(crate) async fn detect(
        &self,
        params: ExternalAgentConfigDetectParams,
    ) -> Result<ExternalAgentConfigDetectResponse, JSONRPCErrorError> {
        let items = self
            .migration_service
            .detect(ExternalAgentConfigDetectOptions {
                include_home: params.include_home,
                cwds: params.cwds,
            })
            .await
            .map_err(|err| internal_error(err.to_string()))?;

        Ok(ExternalAgentConfigDetectResponse {
            items: items
                .into_iter()
                .map(|migration_item| ExternalAgentConfigMigrationItem {
                    item_type: match migration_item.item_type {
                        CoreMigrationItemType::Config => {
                            ExternalAgentConfigMigrationItemType::Config
                        }
                        CoreMigrationItemType::Skills => {
                            ExternalAgentConfigMigrationItemType::Skills
                        }
                        CoreMigrationItemType::AgentsMd => {
                            ExternalAgentConfigMigrationItemType::AgentsMd
                        }
                        CoreMigrationItemType::Plugins => {
                            ExternalAgentConfigMigrationItemType::Plugins
                        }
                        CoreMigrationItemType::McpServerConfig => {
                            ExternalAgentConfigMigrationItemType::McpServerConfig
                        }
                        CoreMigrationItemType::Subagents => {
                            ExternalAgentConfigMigrationItemType::Subagents
                        }
                        CoreMigrationItemType::Hooks => ExternalAgentConfigMigrationItemType::Hooks,
                        CoreMigrationItemType::Commands => {
                            ExternalAgentConfigMigrationItemType::Commands
                        }
                        CoreMigrationItemType::Sessions => {
                            ExternalAgentConfigMigrationItemType::Sessions
                        }
                    },
                    description: migration_item.description,
                    cwd: migration_item.cwd,
                    details: migration_item.details.map(|details| MigrationDetails {
                        plugins: details
                            .plugins
                            .into_iter()
                            .map(|plugin| PluginsMigration {
                                marketplace_name: plugin.marketplace_name,
                                plugin_names: plugin.plugin_names,
                            })
                            .collect(),
                        sessions: Vec::new(),
                        mcp_servers: details
                            .mcp_servers
                            .into_iter()
                            .map(|mcp_server| McpServerMigration {
                                name: mcp_server.name,
                            })
                            .collect(),
                        hooks: details
                            .hooks
                            .into_iter()
                            .map(|hook| HookMigration { name: hook.name })
                            .collect(),
                        subagents: details
                            .subagents
                            .into_iter()
                            .map(|subagent| SubagentMigration {
                                name: subagent.name,
                            })
                            .collect(),
                        commands: details
                            .commands
                            .into_iter()
                            .map(|command| CommandMigration { name: command.name })
                            .collect(),
                    }),
                })
                .collect(),
        })
    }

    pub(crate) async fn import(
        &self,
        params: ExternalAgentConfigImportParams,
    ) -> Result<Vec<PendingPluginImport>, JSONRPCErrorError> {
        self.migration_service
            .import(
                params
                    .migration_items
                    .into_iter()
                    .map(|migration_item| CoreMigrationItem {
                        item_type: match migration_item.item_type {
                            ExternalAgentConfigMigrationItemType::Config => {
                                CoreMigrationItemType::Config
                            }
                            ExternalAgentConfigMigrationItemType::Skills => {
                                CoreMigrationItemType::Skills
                            }
                            ExternalAgentConfigMigrationItemType::AgentsMd => {
                                CoreMigrationItemType::AgentsMd
                            }
                            ExternalAgentConfigMigrationItemType::Plugins => {
                                CoreMigrationItemType::Plugins
                            }
                            ExternalAgentConfigMigrationItemType::McpServerConfig => {
                                CoreMigrationItemType::McpServerConfig
                            }
                            ExternalAgentConfigMigrationItemType::Subagents => {
                                CoreMigrationItemType::Subagents
                            }
                            ExternalAgentConfigMigrationItemType::Hooks => {
                                CoreMigrationItemType::Hooks
                            }
                            ExternalAgentConfigMigrationItemType::Commands => {
                                CoreMigrationItemType::Commands
                            }
                            ExternalAgentConfigMigrationItemType::Sessions => {
                                CoreMigrationItemType::Sessions
                            }
                        },
                        description: migration_item.description,
                        cwd: migration_item.cwd,
                        details: migration_item.details.map(|details| {
                            crate::config::external_agent_config::MigrationDetails {
                                plugins: details
                                    .plugins
                                    .into_iter()
                                    .map(|plugin| {
                                        crate::config::external_agent_config::PluginsMigration {
                                            marketplace_name: plugin.marketplace_name,
                                            plugin_names: plugin.plugin_names,
                                        }
                                    })
                                    .collect(),
                                mcp_servers: details
                                    .mcp_servers
                                    .into_iter()
                                    .map(|mcp_server| CoreNamedMigration {
                                        name: mcp_server.name,
                                    })
                                    .collect(),
                                hooks: details
                                    .hooks
                                    .into_iter()
                                    .map(|hook| CoreNamedMigration { name: hook.name })
                                    .collect(),
                                subagents: details
                                    .subagents
                                    .into_iter()
                                    .map(|subagent| CoreNamedMigration {
                                        name: subagent.name,
                                    })
                                    .collect(),
                                commands: details
                                    .commands
                                    .into_iter()
                                    .map(|command| CoreNamedMigration { name: command.name })
                                    .collect(),
                            }
                        }),
                    })
                    .collect(),
            )
            .await
            .map_err(|err| internal_error(err.to_string()))
    }

    pub(crate) async fn complete_pending_plugin_import(
        &self,
        pending_plugin_import: PendingPluginImport,
    ) -> Result<(), JSONRPCErrorError> {
        self.migration_service
            .import_plugins(
                pending_plugin_import.cwd.as_deref(),
                Some(pending_plugin_import.details),
            )
            .await
            .map(|_| ())
            .map_err(|err| internal_error(err.to_string()))
    }
}
