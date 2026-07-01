use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::registry::AnyToolResult;
use crate::tools::registry::ToolArgumentDiffConsumer;
use crate::tools::registry::ToolRegistry;
use crate::tools::spec::build_specs_with_discoverable_tools;
use agere_mcp::ToolInfo;
use agere_protocol::dynamic_tools::DynamicToolSpec;
use agere_protocol::models::LocalShellAction;
use agere_protocol::models::ResponseItem;
use agere_protocol::models::SearchToolCallParams;
use agere_protocol::models::ShellToolCallParams;
use agere_tools::ConfiguredToolSpec;
use agere_tools::DiscoverableTool;
use agere_tools::FlatWireFunctionToolKind;
use agere_tools::ResponsesApiNamespaceTool;
use agere_tools::ToolName;
use agere_tools::ToolSpec;
use agere_tools::ToolsConfig;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::instrument;

pub use crate::tools::context::ToolCallSource;

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub tool_name: ToolName,
    pub call_id: String,
    pub payload: ToolPayload,
}

pub struct ToolRouter {
    registry: ToolRegistry,
    specs: Vec<ConfiguredToolSpec>,
    model_visible_specs: Vec<ToolSpec>,
    parallel_mcp_server_names: HashSet<String>,
}

pub(crate) struct ToolRouterParams<'a> {
    pub(crate) mcp_tools: Option<HashMap<String, ToolInfo>>,
    pub(crate) deferred_mcp_tools: Option<HashMap<String, ToolInfo>>,
    pub(crate) unavailable_called_tools: Vec<ToolName>,
    pub(crate) parallel_mcp_server_names: HashSet<String>,
    pub(crate) discoverable_tools: Option<Vec<DiscoverableTool>>,
    pub(crate) dynamic_tools: &'a [DynamicToolSpec],
    pub(crate) loaded_search_tool_specs: Vec<ToolSpec>,
}

impl ToolRouter {
    pub fn from_config(config: &ToolsConfig, params: ToolRouterParams<'_>) -> Self {
        let ToolRouterParams {
            mcp_tools,
            deferred_mcp_tools,
            unavailable_called_tools,
            parallel_mcp_server_names,
            discoverable_tools,
            dynamic_tools,
            loaded_search_tool_specs,
        } = params;
        let builder = build_specs_with_discoverable_tools(
            config,
            mcp_tools,
            deferred_mcp_tools,
            unavailable_called_tools,
            discoverable_tools,
            dynamic_tools,
        );
        let (specs, registry) = builder.build();
        let deferred_dynamic_tools = dynamic_tools
            .iter()
            .filter(|tool| tool.defer_loading)
            .map(|tool| ToolName::new(tool.namespace.clone(), tool.name.clone()))
            .collect::<HashSet<_>>();
        let mut model_visible_specs: Vec<ToolSpec> = specs
            .iter()
            .filter_map(|configured_tool| {
                if config.code_mode_only_enabled
                    && agere_code_mode::is_code_mode_nested_tool(configured_tool.name())
                {
                    return None;
                }

                filter_deferred_dynamic_tool_spec(
                    configured_tool.spec.clone(),
                    &deferred_dynamic_tools,
                )
            })
            .collect();
        model_visible_specs.extend(loaded_search_tool_specs);

        Self {
            registry,
            specs,
            model_visible_specs,
            parallel_mcp_server_names,
        }
    }

    pub fn specs(&self) -> Vec<ToolSpec> {
        self.specs
            .iter()
            .map(|config| config.spec.clone())
            .collect()
    }

    pub fn model_visible_specs(&self) -> Vec<ToolSpec> {
        self.model_visible_specs.clone()
    }

    pub fn find_spec(&self, tool_name: &ToolName) -> Option<ToolSpec> {
        self.specs.iter().find_map(|config| match &config.spec {
            ToolSpec::Function(tool)
                if tool_name.namespace.is_none() && tool.name == tool_name.name =>
            {
                Some(config.spec.clone())
            }
            ToolSpec::Freeform(tool)
                if tool_name.namespace.is_none() && tool.name == tool_name.name =>
            {
                Some(config.spec.clone())
            }
            ToolSpec::Namespace(namespace) => namespace.tools.iter().find_map(|tool| match tool {
                ResponsesApiNamespaceTool::Function(tool)
                    if tool_name.namespace.as_deref() == Some(namespace.name.as_str())
                        && tool.name == tool_name.name =>
                {
                    Some(ToolSpec::Function(tool.clone()))
                }
                _ => None,
            }),
            _ => None,
        })
    }

    pub(crate) fn normalize_history_item(&self, item: ResponseItem) -> ResponseItem {
        match item {
            ResponseItem::FunctionCall {
                id,
                name,
                namespace: None,
                arguments,
                call_id,
            } => {
                let projection =
                    agere_tools::FlatWireToolProjection::new(&self.model_visible_specs);
                match projection.source_kind_for_wire_name(&name) {
                    Some(FlatWireFunctionToolKind::NamespaceFunction) => {
                        let canonical_name = projection.canonical_name_for_wire_name(&name);
                        if let Some(namespace) = canonical_name.namespace {
                            ResponseItem::FunctionCall {
                                id,
                                name: canonical_name.name,
                                namespace: Some(namespace),
                                arguments,
                                call_id,
                            }
                        } else {
                            ResponseItem::FunctionCall {
                                id,
                                name,
                                namespace: None,
                                arguments,
                                call_id,
                            }
                        }
                    }
                    Some(FlatWireFunctionToolKind::ToolSearch) => {
                        match serde_json::from_str::<SearchToolCallParams>(&arguments) {
                            Ok(arguments) => ResponseItem::ToolSearchCall {
                                id,
                                call_id: Some(call_id),
                                status: Some("completed".to_string()),
                                execution: "client".to_string(),
                                arguments: serde_json::to_value(arguments).unwrap_or_default(),
                            },
                            Err(_) => ResponseItem::FunctionCall {
                                id,
                                name,
                                namespace: None,
                                arguments,
                                call_id,
                            },
                        }
                    }
                    Some(FlatWireFunctionToolKind::Function) | None => ResponseItem::FunctionCall {
                        id,
                        name,
                        namespace: None,
                        arguments,
                        call_id,
                    },
                }
            }
            item => item,
        }
    }

    pub(crate) fn create_diff_consumer(
        &self,
        tool_name: &ToolName,
    ) -> Option<Box<dyn ToolArgumentDiffConsumer>> {
        self.registry.create_diff_consumer(tool_name)
    }

    fn configured_tool_supports_parallel(&self, tool_name: &ToolName) -> bool {
        if tool_name.namespace.is_some() {
            return false;
        }

        self.specs
            .iter()
            .filter(|config| config.supports_parallel_tool_calls)
            .any(|config| match &config.spec {
                ToolSpec::Function(tool) => tool.name == tool_name.name.as_str(),
                ToolSpec::Freeform(tool) => tool.name == tool_name.name.as_str(),
                ToolSpec::Namespace(_)
                | ToolSpec::ToolSearch { .. }
                | ToolSpec::LocalShell {}
                | ToolSpec::ImageGeneration { .. }
                | ToolSpec::WebSearch { .. } => false,
            })
    }

    pub fn tool_supports_parallel(&self, call: &ToolCall) -> bool {
        match &call.payload {
            // MCP parallel support is configured per server, including for deferred
            // tools that may not have a matching spec entry. Use the parsed payload
            // server so similarly named servers/tools cannot collide.
            ToolPayload::Mcp { server, .. } => self.parallel_mcp_server_names.contains(server),
            _ => self.configured_tool_supports_parallel(&call.tool_name),
        }
    }

    #[instrument(level = "trace", skip_all, err)]
    pub async fn build_tool_call(
        &self,
        session: &Session,
        item: ResponseItem,
    ) -> Result<Option<ToolCall>, FunctionCallError> {
        match item {
            ResponseItem::FunctionCall {
                name,
                namespace,
                arguments,
                call_id,
                ..
            } => {
                let (tool_name, is_flat_tool_search) = if let Some(namespace) = namespace {
                    (ToolName::namespaced(namespace, name), false)
                } else {
                    let projection =
                        agere_tools::FlatWireToolProjection::new(&self.model_visible_specs);
                    let source_kind = projection.source_kind_for_wire_name(&name);
                    (
                        projection.canonical_name_for_wire_name(&name),
                        source_kind == Some(FlatWireFunctionToolKind::ToolSearch),
                    )
                };
                if is_flat_tool_search {
                    let arguments: SearchToolCallParams = serde_json::from_str(&arguments)
                        .map_err(|err| {
                            FunctionCallError::RespondToModel(format!(
                                "failed to parse tool_search arguments: {err}"
                            ))
                        })?;
                    return Ok(Some(ToolCall {
                        tool_name,
                        call_id,
                        payload: ToolPayload::ToolSearch { arguments },
                    }));
                }
                if let Some(tool_info) = session.resolve_mcp_tool_info(&tool_name).await {
                    Ok(Some(ToolCall {
                        tool_name: tool_info.canonical_tool_name(),
                        call_id,
                        payload: ToolPayload::Mcp {
                            server: tool_info.server_name,
                            tool: tool_info.tool.name.to_string(),
                            raw_arguments: arguments,
                        },
                    }))
                } else {
                    Ok(Some(ToolCall {
                        tool_name,
                        call_id,
                        payload: ToolPayload::Function { arguments },
                    }))
                }
            }
            ResponseItem::ToolSearchCall {
                call_id: Some(call_id),
                execution,
                arguments,
                ..
            } if execution == "client" => {
                let arguments: SearchToolCallParams =
                    serde_json::from_value(arguments).map_err(|err| {
                        FunctionCallError::RespondToModel(format!(
                            "failed to parse tool_search arguments: {err}"
                        ))
                    })?;
                Ok(Some(ToolCall {
                    tool_name: ToolName::plain("tool_search"),
                    call_id,
                    payload: ToolPayload::ToolSearch { arguments },
                }))
            }
            ResponseItem::ToolSearchCall { .. } => Ok(None),
            ResponseItem::CustomToolCall {
                name,
                input,
                call_id,
                ..
            } => Ok(Some(ToolCall {
                tool_name: ToolName::plain(name),
                call_id,
                payload: ToolPayload::Custom { input },
            })),
            ResponseItem::LocalShellCall {
                id,
                call_id,
                action,
                ..
            } => {
                let call_id = call_id
                    .or(id)
                    .ok_or(FunctionCallError::MissingLocalShellCallId)?;

                match action {
                    LocalShellAction::Exec(exec) => {
                        let params = ShellToolCallParams {
                            command: exec.command,
                            workdir: exec.working_directory,
                            timeout_ms: exec.timeout_ms,
                            additional_permissions: None,
                            prefix_rule: None,
                            justification: None,
                        };
                        Ok(Some(ToolCall {
                            tool_name: ToolName::plain("local_shell"),
                            call_id,
                            payload: ToolPayload::LocalShell { params },
                        }))
                    }
                }
            }
            _ => Ok(None),
        }
    }

    #[instrument(level = "trace", skip_all, err)]
    pub async fn dispatch_tool_call_with_code_mode_result(
        &self,
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        cancellation_token: CancellationToken,
        tracker: SharedTurnDiffTracker,
        call: ToolCall,
        source: ToolCallSource,
    ) -> Result<AnyToolResult, FunctionCallError> {
        let ToolCall {
            tool_name,
            call_id,
            payload,
        } = call;

        let invocation = ToolInvocation {
            session,
            turn,
            cancellation_token,
            tracker,
            call_id,
            tool_name,
            source,
            payload,
        };

        self.registry.dispatch_any(invocation).await
    }
}

fn filter_deferred_dynamic_tool_spec(
    spec: ToolSpec,
    deferred_dynamic_tools: &HashSet<ToolName>,
) -> Option<ToolSpec> {
    if deferred_dynamic_tools.is_empty() {
        return Some(spec);
    }

    match spec {
        ToolSpec::Function(tool) => {
            if deferred_dynamic_tools.contains(&ToolName::plain(tool.name.as_str())) {
                None
            } else {
                Some(ToolSpec::Function(tool))
            }
        }
        ToolSpec::Namespace(mut namespace) => {
            let namespace_name = namespace.name.clone();
            namespace.tools.retain(|tool| match tool {
                ResponsesApiNamespaceTool::Function(tool) => !deferred_dynamic_tools.contains(
                    &ToolName::namespaced(namespace_name.as_str(), tool.name.as_str()),
                ),
            });
            if namespace.tools.is_empty() {
                None
            } else {
                Some(ToolSpec::Namespace(namespace))
            }
        }
        spec => Some(spec),
    }
}
#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
