use std::collections::HashSet;
use std::sync::Arc;

use crate::session::tests::make_session_and_context;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolRegistry;
use agere_protocol::dynamic_tools::DynamicToolSpec;
use agere_protocol::models::ResponseItem;
use agere_tools::JsonSchema;
use agere_tools::ResponsesApiNamespace;
use agere_tools::ResponsesApiNamespaceTool;
use agere_tools::ResponsesApiTool;
use agere_tools::ToolName;
use agere_tools::ToolSpec;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::ToolCall;
use super::ToolRouter;
use super::ToolRouterParams;

#[tokio::test]
#[expect(
    clippy::await_holding_invalid_type,
    reason = "test builds a router from session-owned MCP manager state"
)]
async fn parallel_support_does_not_match_namespaced_local_tool_names() -> anyhow::Result<()> {
    let (session, turn) = make_session_and_context().await;
    let mcp_tools = session
        .services
        .mcp_connection_manager
        .read()
        .await
        .list_all_tools()
        .await;
    let router = ToolRouter::from_config(
        &turn.tools_config,
        ToolRouterParams {
            deferred_mcp_tools: None,
            mcp_tools: Some(mcp_tools),
            unavailable_called_tools: Vec::new(),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: turn.dynamic_tools.as_slice(),
            loaded_search_tool_specs: Vec::new(),
        },
    );

    let parallel_tool_name = ["shell", "local_shell", "exec_command", "shell_command"]
        .into_iter()
        .find(|name| {
            router.tool_supports_parallel(&ToolCall {
                tool_name: ToolName::plain(*name),
                call_id: "call-parallel-tool".to_string(),
                payload: ToolPayload::Function {
                    arguments: "{}".to_string(),
                },
            })
        })
        .expect("test session should expose a parallel shell-like tool");

    assert!(!router.tool_supports_parallel(&ToolCall {
        tool_name: ToolName::namespaced("mcp__server__", parallel_tool_name),
        call_id: "call-namespaced-tool".to_string(),
        payload: ToolPayload::Function {
            arguments: "{}".to_string(),
        },
    }));

    Ok(())
}

#[tokio::test]
async fn build_tool_call_uses_namespace_for_registry_name() -> anyhow::Result<()> {
    let (session, _) = make_session_and_context().await;
    let session = Arc::new(session);
    let router = empty_router();
    let tool_name = "create_event".to_string();

    let call = router
        .build_tool_call(
            &session,
            ResponseItem::FunctionCall {
                id: None,
                name: tool_name.clone(),
                namespace: Some("mcp__agere_apps__calendar".to_string()),
                arguments: "{}".to_string(),
                call_id: "call-namespace".to_string(),
            },
        )
        .await?
        .expect("function_call should produce a tool call");

    assert_eq!(
        call.tool_name,
        ToolName::namespaced("mcp__agere_apps__calendar", tool_name)
    );
    assert_eq!(call.call_id, "call-namespace");
    match call.payload {
        ToolPayload::Function { arguments } => {
            assert_eq!(arguments, "{}");
        }
        other => panic!("expected function payload, got {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn build_tool_call_resolves_flattened_namespace_function_name() -> anyhow::Result<()> {
    let (session, _) = make_session_and_context().await;
    let session = Arc::new(session);
    let router = ToolRouter {
        registry: ToolRegistry::empty_for_test(),
        specs: Vec::new(),
        model_visible_specs: vec![ToolSpec::Namespace(ResponsesApiNamespace {
            name: "agere_app".to_string(),
            description: "Agere app tools".to_string(),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: "automation_list".to_string(),
                description: "List automations".to_string(),
                strict: false,
                defer_loading: None,
                parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
                output_schema: None,
            })],
        })],
        parallel_mcp_server_names: HashSet::new(),
    };

    let call = router
        .build_tool_call(
            &session,
            ResponseItem::FunctionCall {
                id: None,
                name: "agere_app_automation_list".to_string(),
                namespace: None,
                arguments: "{}".to_string(),
                call_id: "call-flattened".to_string(),
            },
        )
        .await?
        .expect("function_call should produce a tool call");

    assert_eq!(
        call.tool_name,
        ToolName::namespaced("agere_app", "automation_list")
    );
    assert_eq!(call.call_id, "call-flattened");
    match call.payload {
        ToolPayload::Function { arguments } => {
            assert_eq!(arguments, "{}");
        }
        other => panic!("expected function payload, got {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn build_tool_call_routes_flat_tool_search_function_call() -> anyhow::Result<()> {
    let (session, _) = make_session_and_context().await;
    let session = Arc::new(session);
    let router = ToolRouter {
        registry: ToolRegistry::empty_for_test(),
        specs: Vec::new(),
        model_visible_specs: vec![ToolSpec::ToolSearch {
            execution: "client".to_string(),
            description: "Search deferred tools".to_string(),
            parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
        }],
        parallel_mcp_server_names: HashSet::new(),
    };

    let call = router
        .build_tool_call(
            &session,
            ResponseItem::FunctionCall {
                id: None,
                name: "tool_search".to_string(),
                namespace: None,
                arguments: r#"{"query":"calendar","limit":8}"#.to_string(),
                call_id: "call-tool-search".to_string(),
            },
        )
        .await?
        .expect("tool_search function_call should produce a tool call");

    assert_eq!(call.tool_name, ToolName::plain("tool_search"));
    assert_eq!(call.call_id, "call-tool-search");
    match call.payload {
        ToolPayload::ToolSearch { arguments } => {
            assert_eq!(arguments.query, "calendar");
            assert_eq!(arguments.limit, Some(8));
        }
        other => panic!("expected tool_search payload, got {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn build_tool_call_treats_plain_tool_search_function_as_normal_tool() -> anyhow::Result<()> {
    let (session, _) = make_session_and_context().await;
    let session = Arc::new(session);
    let router = ToolRouter {
        registry: ToolRegistry::empty_for_test(),
        specs: Vec::new(),
        model_visible_specs: vec![ToolSpec::Function(ResponsesApiTool {
            name: "tool_search".to_string(),
            description: "Plain dynamic tool named tool_search".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
            output_schema: None,
        })],
        parallel_mcp_server_names: HashSet::new(),
    };

    let call = router
        .build_tool_call(
            &session,
            ResponseItem::FunctionCall {
                id: None,
                name: "tool_search".to_string(),
                namespace: None,
                arguments: r#"{"custom":true}"#.to_string(),
                call_id: "call-plain-tool-search".to_string(),
            },
        )
        .await?
        .expect("plain function_call should produce a tool call");

    assert_eq!(call.tool_name, ToolName::plain("tool_search"));
    assert_eq!(call.call_id, "call-plain-tool-search");
    match call.payload {
        ToolPayload::Function { arguments } => {
            assert_eq!(arguments, r#"{"custom":true}"#);
        }
        other => panic!("expected function payload, got {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn build_tool_call_disambiguates_plain_tool_search_from_builtin_search() -> anyhow::Result<()>
{
    let (session, _) = make_session_and_context().await;
    let session = Arc::new(session);
    let specs = vec![
        ToolSpec::Function(ResponsesApiTool {
            name: "tool_search".to_string(),
            description: "Plain dynamic tool named tool_search".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
            output_schema: None,
        }),
        ToolSpec::ToolSearch {
            execution: "client".to_string(),
            description: "Search deferred tools".to_string(),
            parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
        },
    ];
    let projection = agere_tools::FlatWireToolProjection::new(&specs);
    let plain_wire_name = projection.wire_name_for_function_tool(&ToolName::plain("tool_search"));
    let search_wire_name = projection.wire_name_for_tool_search();
    let router = ToolRouter {
        registry: ToolRegistry::empty_for_test(),
        specs: Vec::new(),
        model_visible_specs: specs,
        parallel_mcp_server_names: HashSet::new(),
    };

    let plain_call = router
        .build_tool_call(
            &session,
            ResponseItem::FunctionCall {
                id: None,
                name: plain_wire_name,
                namespace: None,
                arguments: r#"{"custom":true}"#.to_string(),
                call_id: "call-plain-tool-search".to_string(),
            },
        )
        .await?
        .expect("plain function_call should produce a tool call");
    let search_call = router
        .build_tool_call(
            &session,
            ResponseItem::FunctionCall {
                id: None,
                name: search_wire_name,
                namespace: None,
                arguments: r#"{"query":"calendar","limit":8}"#.to_string(),
                call_id: "call-builtin-tool-search".to_string(),
            },
        )
        .await?
        .expect("built-in tool_search function_call should produce a tool call");

    assert!(matches!(
        plain_call.payload,
        ToolPayload::Function { arguments } if arguments == r#"{"custom":true}"#
    ));
    assert!(matches!(
        search_call.payload,
        ToolPayload::ToolSearch { arguments }
            if arguments.query == "calendar" && arguments.limit == Some(8)
    ));

    Ok(())
}

#[tokio::test]
async fn loaded_search_namespace_tools_are_visible_and_route_from_flat_names() -> anyhow::Result<()>
{
    let (session, turn) = make_session_and_context().await;
    let session = Arc::new(session);
    let router = ToolRouter::from_config(
        &turn.tools_config,
        ToolRouterParams {
            deferred_mcp_tools: None,
            mcp_tools: None,
            unavailable_called_tools: Vec::new(),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: turn.dynamic_tools.as_slice(),
            loaded_search_tool_specs: vec![ToolSpec::Namespace(ResponsesApiNamespace {
                name: "agere_app".to_string(),
                description: "Agere app tools".to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: "automation_update".to_string(),
                    description: "Update automations".to_string(),
                    strict: false,
                    defer_loading: None,
                    parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
                    output_schema: None,
                })],
            })],
        },
    );

    assert_eq!(
        namespace_function_names(&router.model_visible_specs(), "agere_app"),
        vec!["automation_update".to_string()]
    );

    let call = router
        .build_tool_call(
            &session,
            ResponseItem::FunctionCall {
                id: None,
                name: "agere_app_automation_update".to_string(),
                namespace: None,
                arguments: "{}".to_string(),
                call_id: "call-loaded".to_string(),
            },
        )
        .await?
        .expect("loaded flat function_call should produce a tool call");

    assert_eq!(
        call.tool_name,
        ToolName::namespaced("agere_app", "automation_update")
    );

    Ok(())
}

#[tokio::test]
async fn normalize_history_item_restores_flat_tool_search_call() -> anyhow::Result<()> {
    let (_, turn) = make_session_and_context().await;
    let router = ToolRouter::from_config(
        &turn.tools_config,
        ToolRouterParams {
            deferred_mcp_tools: None,
            mcp_tools: None,
            unavailable_called_tools: Vec::new(),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: turn.dynamic_tools.as_slice(),
            loaded_search_tool_specs: vec![ToolSpec::ToolSearch {
                execution: "client".to_string(),
                description: "Search deferred tools".to_string(),
                parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
            }],
        },
    );

    let item = router.normalize_history_item(ResponseItem::FunctionCall {
        id: None,
        name: "tool_search".to_string(),
        namespace: None,
        arguments: r#"{"query":"calendar","limit":1}"#.to_string(),
        call_id: "search-1".to_string(),
    });

    assert_eq!(
        item,
        ResponseItem::ToolSearchCall {
            id: None,
            call_id: Some("search-1".to_string()),
            status: Some("completed".to_string()),
            execution: "client".to_string(),
            arguments: json!({
                "query": "calendar",
                "limit": 1,
            }),
        }
    );

    Ok(())
}

#[tokio::test]
async fn normalize_history_item_restores_flat_namespaced_function_call() -> anyhow::Result<()> {
    let (_, turn) = make_session_and_context().await;
    let router = ToolRouter::from_config(
        &turn.tools_config,
        ToolRouterParams {
            deferred_mcp_tools: None,
            mcp_tools: None,
            unavailable_called_tools: Vec::new(),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: turn.dynamic_tools.as_slice(),
            loaded_search_tool_specs: vec![ToolSpec::Namespace(ResponsesApiNamespace {
                name: "agere_app".to_string(),
                description: "Agere app tools".to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: "automation_update".to_string(),
                    description: "Update automations".to_string(),
                    strict: false,
                    defer_loading: None,
                    parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
                    output_schema: None,
                })],
            })],
        },
    );

    let item = router.normalize_history_item(ResponseItem::FunctionCall {
        id: None,
        name: "agere_app_automation_update".to_string(),
        namespace: None,
        arguments: "{}".to_string(),
        call_id: "call-loaded".to_string(),
    });

    assert_eq!(
        item,
        ResponseItem::FunctionCall {
            id: None,
            name: "automation_update".to_string(),
            namespace: Some("agere_app".to_string()),
            arguments: "{}".to_string(),
            call_id: "call-loaded".to_string(),
        }
    );

    Ok(())
}

#[tokio::test]
async fn mcp_parallel_support_uses_exact_payload_server() -> anyhow::Result<()> {
    let (_, turn) = make_session_and_context().await;
    let router = ToolRouter::from_config(
        &turn.tools_config,
        ToolRouterParams {
            deferred_mcp_tools: None,
            mcp_tools: None,
            unavailable_called_tools: Vec::new(),
            parallel_mcp_server_names: HashSet::from(["echo".to_string()]),
            discoverable_tools: None,
            dynamic_tools: turn.dynamic_tools.as_slice(),
            loaded_search_tool_specs: Vec::new(),
        },
    );

    let deferred_call = ToolCall {
        tool_name: ToolName::namespaced("mcp__echo__", "query_with_delay"),
        call_id: "call-deferred".to_string(),
        payload: ToolPayload::Mcp {
            server: "echo".to_string(),
            tool: "query_with_delay".to_string(),
            raw_arguments: "{}".to_string(),
        },
    };
    assert!(router.tool_supports_parallel(&deferred_call));

    let different_server_call = ToolCall {
        tool_name: ToolName::namespaced("mcp__hello_echo__", "query_with_delay"),
        call_id: "call-other-server".to_string(),
        payload: ToolPayload::Mcp {
            server: "hello_echo".to_string(),
            tool: "query_with_delay".to_string(),
            raw_arguments: "{}".to_string(),
        },
    };
    assert!(!router.tool_supports_parallel(&different_server_call));

    Ok(())
}

#[tokio::test]
async fn model_visible_specs_filter_deferred_dynamic_tools() -> anyhow::Result<()> {
    let (_, turn) = make_session_and_context().await;
    let hidden_tool = "hidden_dynamic_tool";
    let visible_tool = "visible_dynamic_tool";
    let dynamic_tools = vec![
        DynamicToolSpec {
            namespace: Some("agere_app".to_string()),
            name: hidden_tool.to_string(),
            description: "Hidden until discovered.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
            defer_loading: true,
        },
        DynamicToolSpec {
            namespace: Some("agere_app".to_string()),
            name: visible_tool.to_string(),
            description: "Visible immediately.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
            defer_loading: false,
        },
    ];

    let router = ToolRouter::from_config(
        &turn.tools_config,
        ToolRouterParams {
            deferred_mcp_tools: None,
            mcp_tools: None,
            unavailable_called_tools: Vec::new(),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: &dynamic_tools,
            loaded_search_tool_specs: Vec::new(),
        },
    );

    assert!(
        router
            .find_spec(&ToolName::namespaced("agere_app", hidden_tool))
            .is_some()
    );
    assert_eq!(
        namespace_function_names(&router.specs(), "agere_app"),
        vec![hidden_tool.to_string(), visible_tool.to_string()]
    );
    assert_eq!(
        namespace_function_names(&router.model_visible_specs(), "agere_app"),
        vec![visible_tool.to_string()]
    );

    Ok(())
}

fn namespace_function_names(specs: &[ToolSpec], namespace_name: &str) -> Vec<String> {
    specs
        .iter()
        .find_map(|spec| match spec {
            ToolSpec::Namespace(namespace) if namespace.name == namespace_name => Some(
                namespace
                    .tools
                    .iter()
                    .map(|tool| match tool {
                        ResponsesApiNamespaceTool::Function(tool) => tool.name.clone(),
                    })
                    .collect(),
            ),
            ToolSpec::Function(_)
            | ToolSpec::Freeform(_)
            | ToolSpec::ToolSearch { .. }
            | ToolSpec::LocalShell {}
            | ToolSpec::ImageGeneration { .. }
            | ToolSpec::WebSearch { .. }
            | ToolSpec::Namespace(_) => None,
        })
        .unwrap_or_default()
}

fn empty_router() -> ToolRouter {
    ToolRouter {
        registry: ToolRegistry::empty_for_test(),
        specs: Vec::new(),
        model_visible_specs: Vec::new(),
        parallel_mcp_server_names: HashSet::new(),
    }
}
