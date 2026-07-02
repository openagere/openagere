use crate::tools::loaded_search_tools::LoadedSearchToolSource;
use crate::tools::loaded_search_tools::collect_loaded_search_tool_specs;
use agere_protocol::dynamic_tools::DynamicToolSpec;
use agere_protocol::models::ResponseItem;
use agere_tools::ConfiguredToolSpec;
use agere_tools::ResponsesApiNamespaceTool;
use agere_tools::ToolName;
use agere_tools::ToolSpec;
use agere_tools::ToolsConfig;
use std::collections::HashSet;

pub(crate) fn build_model_visible_specs(
    config: &ToolsConfig,
    specs: &[ConfiguredToolSpec],
    dynamic_tools: &[DynamicToolSpec],
    history_input: &[ResponseItem],
) -> Vec<ToolSpec> {
    let deferred_dynamic_tools = dynamic_tools
        .iter()
        .filter(|tool| tool.defer_loading)
        .map(|tool| ToolName::new(tool.namespace.clone(), tool.name.clone()))
        .collect::<HashSet<_>>();
    let mut model_visible_specs = specs
        .iter()
        .filter_map(|configured_tool| {
            filter_model_visible_tool_spec(
                config,
                configured_tool.spec.clone(),
                &deferred_dynamic_tools,
            )
        })
        .collect::<Vec<_>>();
    let loaded_search_tool_specs = collect_loaded_search_tool_specs(
        history_input,
        &[LoadedSearchToolSource::ToolSearchOutputs],
        &[],
    );
    extend_with_loaded_search_tool_specs(
        config,
        &mut model_visible_specs,
        loaded_search_tool_specs,
    );

    loop {
        let previous_tool_names = model_visible_tool_names(&model_visible_specs);
        let loaded_search_tool_specs = collect_loaded_search_tool_specs(
            history_input,
            &[LoadedSearchToolSource::FlatToolSearchFunctionOutputs],
            &model_visible_specs,
        );
        extend_with_loaded_search_tool_specs(
            config,
            &mut model_visible_specs,
            loaded_search_tool_specs,
        );
        if model_visible_tool_names(&model_visible_specs) == previous_tool_names {
            break;
        }
    }

    model_visible_specs
}

fn filter_model_visible_tool_spec(
    config: &ToolsConfig,
    spec: ToolSpec,
    deferred_dynamic_tools: &HashSet<ToolName>,
) -> Option<ToolSpec> {
    if config.code_mode_only_enabled
        && !matches!(
            &spec,
            ToolSpec::Freeform(tool) if tool.name == agere_code_mode::PUBLIC_TOOL_NAME
        )
        && !matches!(
            &spec,
            ToolSpec::Function(tool) if tool.name == agere_code_mode::WAIT_TOOL_NAME
        )
    {
        return None;
    }

    filter_deferred_dynamic_tool_spec(spec, deferred_dynamic_tools)
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

fn extend_with_loaded_search_tool_specs(
    config: &ToolsConfig,
    model_visible_specs: &mut Vec<ToolSpec>,
    loaded_search_tool_specs: Vec<ToolSpec>,
) {
    let mut visible_tool_names = model_visible_tool_names(model_visible_specs);
    for spec in loaded_search_tool_specs {
        let Some(spec) = filter_model_visible_tool_spec(config, spec, &HashSet::<ToolName>::new())
        else {
            continue;
        };
        match spec {
            ToolSpec::Function(tool) => {
                if visible_tool_names.insert(ToolName::plain(tool.name.clone())) {
                    model_visible_specs.push(ToolSpec::Function(tool));
                }
            }
            ToolSpec::Namespace(mut namespace) => {
                let namespace_name = namespace.name.clone();
                namespace.tools.retain(|tool| match tool {
                    ResponsesApiNamespaceTool::Function(tool) => visible_tool_names.insert(
                        ToolName::namespaced(namespace_name.clone(), tool.name.clone()),
                    ),
                });
                if namespace.tools.is_empty() {
                    continue;
                }

                if let Some(existing_namespace) =
                    model_visible_specs.iter_mut().find_map(|spec| match spec {
                        ToolSpec::Namespace(existing_namespace)
                            if existing_namespace.name == namespace.name =>
                        {
                            Some(existing_namespace)
                        }
                        ToolSpec::Function(_)
                        | ToolSpec::Freeform(_)
                        | ToolSpec::ToolSearch { .. }
                        | ToolSpec::LocalShell {}
                        | ToolSpec::ImageGeneration { .. }
                        | ToolSpec::WebSearch { .. }
                        | ToolSpec::Namespace(_) => None,
                    })
                {
                    existing_namespace.tools.append(&mut namespace.tools);
                } else {
                    model_visible_specs.push(ToolSpec::Namespace(namespace));
                }
            }
            ToolSpec::ToolSearch { .. }
            | ToolSpec::LocalShell {}
            | ToolSpec::ImageGeneration { .. }
            | ToolSpec::WebSearch { .. }
            | ToolSpec::Freeform(_) => {
                model_visible_specs.push(spec);
            }
        }
    }
}

fn model_visible_tool_names(specs: &[ToolSpec]) -> HashSet<ToolName> {
    let mut tool_names = HashSet::new();
    for spec in specs {
        match spec {
            ToolSpec::Function(tool) => {
                tool_names.insert(ToolName::plain(tool.name.clone()));
            }
            ToolSpec::Namespace(namespace) => {
                let namespace_name = namespace.name.clone();
                for tool in &namespace.tools {
                    match tool {
                        ResponsesApiNamespaceTool::Function(tool) => {
                            tool_names.insert(ToolName::namespaced(
                                namespace_name.clone(),
                                tool.name.clone(),
                            ));
                        }
                    }
                }
            }
            ToolSpec::Freeform(_)
            | ToolSpec::ToolSearch { .. }
            | ToolSpec::LocalShell {}
            | ToolSpec::ImageGeneration { .. }
            | ToolSpec::WebSearch { .. } => {}
        }
    }
    tool_names
}
