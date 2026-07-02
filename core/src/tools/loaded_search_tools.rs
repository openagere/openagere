use std::collections::HashSet;

use agere_protocol::models::FunctionCallOutputBody;
use agere_protocol::models::ResponseItem;
use agere_protocol::models::SearchToolCallParams;
use agere_tools::FlatWireFunctionToolKind;
use agere_tools::FlatWireToolProjection;
use agere_tools::LoadableToolSpec;
use agere_tools::ResponsesApiNamespaceTool;
use agere_tools::ToolName;
use agere_tools::ToolSpec;
use agere_tools::coalesce_loadable_tool_specs;

#[derive(Clone, Copy)]
pub(crate) enum LoadedSearchToolSource {
    ToolSearchOutputs,
    FlatToolSearchFunctionOutputs,
}

pub(crate) fn collect_loaded_search_tool_specs(
    input: &[ResponseItem],
    sources: &[LoadedSearchToolSource],
    flat_wire_tool_specs: &[ToolSpec],
) -> Vec<ToolSpec> {
    let include_tool_search_outputs = sources
        .iter()
        .any(|source| matches!(source, LoadedSearchToolSource::ToolSearchOutputs));
    let include_flat_tool_search_function_outputs = sources.iter().any(|source| {
        matches!(
            source,
            LoadedSearchToolSource::FlatToolSearchFunctionOutputs
        )
    });
    let flat_wire_projection = include_flat_tool_search_function_outputs
        .then(|| FlatWireToolProjection::new(flat_wire_tool_specs));
    let flat_tool_search_call_ids = input
        .iter()
        .filter_map(|item| match item {
            ResponseItem::FunctionCall {
                name,
                namespace: None,
                arguments,
                call_id,
                ..
            } if is_flat_tool_search_function_name(name, flat_wire_projection.as_ref())
                && serde_json::from_str::<SearchToolCallParams>(arguments).is_ok() =>
            {
                Some(call_id.clone())
            }
            _ => None,
        })
        .collect::<HashSet<_>>();

    let loaded_specs = input
        .iter()
        .filter_map(|item| match item {
            ResponseItem::ToolSearchOutput {
                execution, tools, ..
            } if include_tool_search_outputs && execution == "client" => Some(tools.clone()),
            ResponseItem::FunctionCallOutput { call_id, output }
                if include_flat_tool_search_function_outputs
                    && flat_tool_search_call_ids.contains(call_id) =>
            {
                match &output.body {
                    FunctionCallOutputBody::Text(text) => {
                        match serde_json::from_str::<Vec<serde_json::Value>>(text) {
                            Ok(tools) => Some(tools),
                            Err(error) => {
                                tracing::error!(
                                    "Failed to parse flat tool_search output tools: {error}"
                                );
                                None
                            }
                        }
                    }
                    FunctionCallOutputBody::ContentItems(_) => None,
                }
            }
            _ => None,
        })
        .flat_map(std::iter::IntoIterator::into_iter)
        .filter_map(
            |tool| match serde_json::from_value::<LoadableToolSpec>(tool) {
                Ok(spec) => Some(spec),
                Err(error) => {
                    tracing::error!("Failed to parse tool_search output tool spec: {error}");
                    None
                }
            },
        )
        .collect::<Vec<_>>();

    let mut seen_tool_names = HashSet::new();
    coalesce_loadable_tool_specs(loaded_specs)
        .into_iter()
        .map(strip_defer_loading)
        .filter_map(|spec| match spec {
            LoadableToolSpec::Function(tool) => seen_tool_names
                .insert(ToolName::plain(tool.name.clone()))
                .then_some(LoadableToolSpec::Function(tool)),
            LoadableToolSpec::Namespace(mut namespace) => {
                let namespace_name = namespace.name.clone();
                namespace.tools.retain(|tool| match tool {
                    ResponsesApiNamespaceTool::Function(tool) => seen_tool_names.insert(
                        ToolName::namespaced(namespace_name.clone(), tool.name.clone()),
                    ),
                });
                (!namespace.tools.is_empty()).then_some(LoadableToolSpec::Namespace(namespace))
            }
        })
        .map(ToolSpec::from)
        .collect()
}

fn is_flat_tool_search_function_name(
    name: &str,
    projection: Option<&FlatWireToolProjection>,
) -> bool {
    if let Some(source_kind) =
        projection.and_then(|projection| projection.source_kind_for_wire_name(name))
    {
        return source_kind == FlatWireFunctionToolKind::ToolSearch;
    }

    name == agere_tools::TOOL_SEARCH_TOOL_NAME
}

fn strip_defer_loading(spec: LoadableToolSpec) -> LoadableToolSpec {
    match spec {
        LoadableToolSpec::Function(mut tool) => {
            tool.defer_loading = None;
            LoadableToolSpec::Function(tool)
        }
        LoadableToolSpec::Namespace(mut namespace) => {
            for tool in &mut namespace.tools {
                match tool {
                    agere_tools::ResponsesApiNamespaceTool::Function(tool) => {
                        tool.defer_loading = None;
                    }
                }
            }
            LoadableToolSpec::Namespace(namespace)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agere_tools::JsonSchema;
    use agere_tools::ResponsesApiNamespace;
    use agere_tools::ResponsesApiNamespaceTool;
    use agere_tools::ResponsesApiTool;
    use pretty_assertions::assert_eq;

    #[test]
    fn collects_loaded_namespace_tools_from_tool_search_outputs() {
        let tool = LoadableToolSpec::Namespace(ResponsesApiNamespace {
            name: "mcp__calendar__".to_string(),
            description: "Calendar tools".to_string(),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: "create_event".to_string(),
                description: "Create an event".to_string(),
                strict: false,
                defer_loading: Some(true),
                parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
                output_schema: None,
            })],
        });
        let input = vec![ResponseItem::ToolSearchOutput {
            call_id: Some("search-1".to_string()),
            status: "completed".to_string(),
            execution: "client".to_string(),
            tools: vec![serde_json::to_value(tool).expect("serialize loadable tool")],
        }];

        let specs = collect_loaded_search_tool_specs(
            &input,
            &[LoadedSearchToolSource::ToolSearchOutputs],
            &[],
        );

        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name(), "mcp__calendar__");
        assert_eq!(
            specs[0],
            ToolSpec::Namespace(ResponsesApiNamespace {
                name: "mcp__calendar__".to_string(),
                description: "Calendar tools".to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: "create_event".to_string(),
                    description: "Create an event".to_string(),
                    strict: false,
                    defer_loading: None,
                    parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
                    output_schema: None,
                })],
            })
        );
    }

    #[test]
    fn collects_loaded_tools_from_flat_tool_search_function_history() {
        let tools = serde_json::to_string(&vec![serde_json::json!({
            "type": "namespace",
            "name": "mcp__calendar__",
            "description": "Calendar tools",
            "tools": [{
                "type": "function",
                "name": "create_event",
                "description": "Create an event",
                "strict": false,
                "defer_loading": true,
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            }]
        })])
        .expect("serialize tool search output");
        let input = vec![
            ResponseItem::FunctionCall {
                id: None,
                name: agere_tools::TOOL_SEARCH_TOOL_NAME.to_string(),
                namespace: None,
                arguments: r#"{"query":"calendar"}"#.to_string(),
                call_id: "search-1".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "search-1".to_string(),
                output: agere_protocol::models::FunctionCallOutputPayload::from_text(tools),
            },
        ];
        let projection_specs = vec![tool_search_spec()];

        let specs = collect_loaded_search_tool_specs(
            &input,
            &[LoadedSearchToolSource::FlatToolSearchFunctionOutputs],
            &projection_specs,
        );

        assert_eq!(
            specs,
            vec![ToolSpec::Namespace(ResponsesApiNamespace {
                name: "mcp__calendar__".to_string(),
                description: "Calendar tools".to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: "create_event".to_string(),
                    description: "Create an event".to_string(),
                    strict: false,
                    defer_loading: None,
                    parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
                    output_schema: None,
                })],
            })]
        );
    }

    #[test]
    fn collects_loaded_tools_from_disambiguated_flat_tool_search_function_history() {
        let projection_specs = vec![
            ToolSpec::Function(ResponsesApiTool {
                name: agere_tools::TOOL_SEARCH_TOOL_NAME.to_string(),
                description: "Plain dynamic tool named tool_search".to_string(),
                strict: false,
                defer_loading: None,
                parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
                output_schema: None,
            }),
            tool_search_spec(),
        ];
        let search_wire_name =
            agere_tools::FlatWireToolProjection::new(&projection_specs).wire_name_for_tool_search();
        let tools = serde_json::to_string(&vec![serde_json::json!({
            "type": "namespace",
            "name": "mcp__calendar__",
            "description": "Calendar tools",
            "tools": [{
                "type": "function",
                "name": "create_event",
                "description": "Create an event",
                "strict": false,
                "defer_loading": true,
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            }]
        })])
        .expect("serialize tool search output");
        let input = vec![
            ResponseItem::FunctionCall {
                id: None,
                name: search_wire_name,
                namespace: None,
                arguments: r#"{"query":"calendar"}"#.to_string(),
                call_id: "search-1".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "search-1".to_string(),
                output: agere_protocol::models::FunctionCallOutputPayload::from_text(tools),
            },
        ];

        let specs = collect_loaded_search_tool_specs(
            &input,
            &[LoadedSearchToolSource::FlatToolSearchFunctionOutputs],
            &projection_specs,
        );

        assert_eq!(
            specs,
            vec![ToolSpec::Namespace(ResponsesApiNamespace {
                name: "mcp__calendar__".to_string(),
                description: "Calendar tools".to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: "create_event".to_string(),
                    description: "Create an event".to_string(),
                    strict: false,
                    defer_loading: None,
                    parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
                    output_schema: None,
                })],
            })]
        );
    }

    #[test]
    fn deduplicates_loaded_namespace_tools_from_repeated_search_outputs() {
        let repeated_tool = serde_json::json!({
            "type": "namespace",
            "name": "mcp__calendar__",
            "description": "Calendar tools",
            "tools": [{
                "type": "function",
                "name": "create_event",
                "description": "Create an event",
                "strict": false,
                "defer_loading": true,
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            }]
        });
        let input = vec![
            ResponseItem::ToolSearchOutput {
                call_id: Some("search-1".to_string()),
                status: "completed".to_string(),
                execution: "client".to_string(),
                tools: vec![repeated_tool.clone()],
            },
            ResponseItem::ToolSearchOutput {
                call_id: Some("search-2".to_string()),
                status: "completed".to_string(),
                execution: "client".to_string(),
                tools: vec![repeated_tool],
            },
        ];

        let specs = collect_loaded_search_tool_specs(
            &input,
            &[LoadedSearchToolSource::ToolSearchOutputs],
            &[],
        );

        assert_eq!(
            specs,
            vec![ToolSpec::Namespace(ResponsesApiNamespace {
                name: "mcp__calendar__".to_string(),
                description: "Calendar tools".to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: "create_event".to_string(),
                    description: "Create an event".to_string(),
                    strict: false,
                    defer_loading: None,
                    parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
                    output_schema: None,
                })],
            })]
        );
    }

    #[test]
    fn ignores_tool_search_outputs_when_only_flat_history_is_requested() {
        let input = vec![ResponseItem::ToolSearchOutput {
            call_id: Some("search-1".to_string()),
            status: "completed".to_string(),
            execution: "client".to_string(),
            tools: vec![serde_json::json!({
                "type": "namespace",
                "name": "mcp__calendar__",
                "description": "Calendar tools",
                "tools": []
            })],
        }];

        let specs = collect_loaded_search_tool_specs(
            &input,
            &[LoadedSearchToolSource::FlatToolSearchFunctionOutputs],
            &[],
        );

        assert_eq!(specs, Vec::new());
    }

    fn tool_search_spec() -> ToolSpec {
        ToolSpec::ToolSearch {
            execution: "client".to_string(),
            description: "Search deferred tools".to_string(),
            parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
        }
    }
}
