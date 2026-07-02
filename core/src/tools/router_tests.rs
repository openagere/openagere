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
            history_input: &[],
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
    let router = test_router(vec![ToolSpec::Namespace(ResponsesApiNamespace {
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
    })]);

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
    let router = test_router(vec![ToolSpec::ToolSearch {
        execution: "client".to_string(),
        description: "Search deferred tools".to_string(),
        parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
    }]);

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
    let router = test_router(vec![ToolSpec::Function(ResponsesApiTool {
        name: "tool_search".to_string(),
        description: "Plain dynamic tool named tool_search".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
        output_schema: None,
    })]);

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
    let router = test_router(specs);

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
    let history_input = vec![loaded_namespace_tool_search_output(
        "agere_app",
        "automation_update",
        "Update automations",
    )];
    let router = ToolRouter::from_config(
        &turn.tools_config,
        ToolRouterParams {
            deferred_mcp_tools: None,
            mcp_tools: None,
            unavailable_called_tools: Vec::new(),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: turn.dynamic_tools.as_slice(),
            history_input: &history_input,
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
async fn code_mode_only_hides_loaded_search_tools() -> anyhow::Result<()> {
    let (_, turn) = make_session_and_context().await;
    let mut tools_config = turn.tools_config;
    tools_config.code_mode_enabled = true;
    tools_config.code_mode_only_enabled = true;
    let history_input = vec![loaded_namespace_tool_search_output(
        "agere_app",
        "automation_update",
        "Update automations",
    )];
    let router = ToolRouter::from_config(
        &tools_config,
        ToolRouterParams {
            deferred_mcp_tools: None,
            mcp_tools: None,
            unavailable_called_tools: Vec::new(),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: turn.dynamic_tools.as_slice(),
            history_input: &history_input,
        },
    );

    assert_eq!(
        all_namespace_function_names(&router.model_visible_specs(), "agere_app"),
        Vec::<String>::new()
    );

    Ok(())
}

#[tokio::test]
async fn code_mode_only_hides_loaded_plain_exec_tool() -> anyhow::Result<()> {
    let (_, turn) = make_session_and_context().await;
    let mut tools_config = turn.tools_config;
    tools_config.code_mode_enabled = true;
    tools_config.code_mode_only_enabled = true;
    let history_input = vec![ResponseItem::ToolSearchOutput {
        call_id: Some("search-1".to_string()),
        status: "completed".to_string(),
        execution: "client".to_string(),
        tools: vec![serde_json::json!({
            "type": "function",
            "name": agere_code_mode::PUBLIC_TOOL_NAME,
            "description": "Loaded plain tool that must not replace code-mode exec.",
            "strict": false,
            "parameters": {
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            },
        })],
    }];
    let router = ToolRouter::from_config(
        &tools_config,
        ToolRouterParams {
            deferred_mcp_tools: None,
            mcp_tools: None,
            unavailable_called_tools: Vec::new(),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: turn.dynamic_tools.as_slice(),
            history_input: &history_input,
        },
    );

    let model_visible_specs = router.model_visible_specs();
    let exec_specs = model_visible_specs
        .iter()
        .filter(|spec| spec.name() == agere_code_mode::PUBLIC_TOOL_NAME)
        .cloned()
        .collect::<Vec<_>>();

    assert_eq!(exec_specs.len(), 1);
    assert!(
        matches!(exec_specs.as_slice(), [ToolSpec::Freeform(tool)] if tool.name == agere_code_mode::PUBLIC_TOOL_NAME)
    );

    Ok(())
}

#[tokio::test]
async fn model_visible_specs_deduplicate_loaded_tools_against_regular_tools() -> anyhow::Result<()>
{
    let (_, turn) = make_session_and_context().await;
    let duplicate_tool = "duplicate_dynamic_tool";
    let loaded_only_tool = "loaded_only_dynamic_tool";
    let dynamic_tools = vec![DynamicToolSpec {
        namespace: Some("agere_app".to_string()),
        name: duplicate_tool.to_string(),
        description: "Already visible through regular registry.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false,
        }),
        defer_loading: false,
    }];
    let history_input = vec![ResponseItem::ToolSearchOutput {
        call_id: Some("search-1".to_string()),
        status: "completed".to_string(),
        execution: "client".to_string(),
        tools: vec![serde_json::json!({
            "type": "namespace",
            "name": "agere_app",
            "description": "Agere app tools",
            "tools": [
                {
                    "type": "function",
                    "name": duplicate_tool,
                    "description": "Loaded duplicate.",
                    "strict": false,
                    "parameters": {
                        "type": "object",
                        "properties": {},
                        "additionalProperties": false,
                    },
                },
                {
                    "type": "function",
                    "name": loaded_only_tool,
                    "description": "Loaded only.",
                    "strict": false,
                    "parameters": {
                        "type": "object",
                        "properties": {},
                        "additionalProperties": false,
                    },
                },
            ],
        })],
    }];

    let router = ToolRouter::from_config(
        &turn.tools_config,
        ToolRouterParams {
            deferred_mcp_tools: None,
            mcp_tools: None,
            unavailable_called_tools: Vec::new(),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: &dynamic_tools,
            history_input: &history_input,
        },
    );

    assert_eq!(
        all_namespace_function_names(&router.model_visible_specs(), "agere_app"),
        vec![duplicate_tool.to_string(), loaded_only_tool.to_string()]
    );

    Ok(())
}

#[tokio::test]
async fn model_visible_specs_keep_loaded_plain_tool_search_function() -> anyhow::Result<()> {
    let (_, turn) = make_session_and_context().await;
    let mut tools_config = turn.tools_config;
    tools_config.search_tool = true;
    let dynamic_tools = vec![
        DynamicToolSpec {
            namespace: Some("agere_app".to_string()),
            name: "automation_update".to_string(),
            description: "Create or update automations.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
            defer_loading: true,
        },
        DynamicToolSpec {
            namespace: None,
            name: agere_tools::TOOL_SEARCH_TOOL_NAME.to_string(),
            description: "Plain dynamic tool named tool_search.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
            defer_loading: true,
        },
    ];
    let history_input = vec![ResponseItem::ToolSearchOutput {
        call_id: Some("search-1".to_string()),
        status: "completed".to_string(),
        execution: "client".to_string(),
        tools: vec![serde_json::json!({
            "type": "function",
            "name": "tool_search",
            "description": "Plain dynamic tool named tool_search.",
            "strict": false,
            "parameters": {
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            },
        })],
    }];

    let router = ToolRouter::from_config(
        &tools_config,
        ToolRouterParams {
            deferred_mcp_tools: None,
            mcp_tools: None,
            unavailable_called_tools: Vec::new(),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: &dynamic_tools,
            history_input: &history_input,
        },
    );

    let specs = router.model_visible_specs();
    assert!(
        specs
            .iter()
            .any(|spec| matches!(spec, ToolSpec::ToolSearch { .. }))
    );
    let plain_function_count = specs
        .iter()
        .filter(|spec| match spec {
            ToolSpec::Function(tool) => tool.name == agere_tools::TOOL_SEARCH_TOOL_NAME,
            ToolSpec::Freeform(_)
            | ToolSpec::ToolSearch { .. }
            | ToolSpec::LocalShell {}
            | ToolSpec::ImageGeneration { .. }
            | ToolSpec::WebSearch { .. }
            | ToolSpec::Namespace(_) => false,
        })
        .count();

    assert_eq!(plain_function_count, 1);

    Ok(())
}

#[tokio::test]
async fn model_visible_specs_restore_disambiguated_flat_search_after_loaded_plain_tool_search()
-> anyhow::Result<()> {
    let (_, turn) = make_session_and_context().await;
    let mut tools_config = turn.tools_config;
    tools_config.search_tool = true;
    let dynamic_tools = vec![
        DynamicToolSpec {
            namespace: None,
            name: agere_tools::TOOL_SEARCH_TOOL_NAME.to_string(),
            description: "Plain dynamic tool named tool_search.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
            defer_loading: true,
        },
        DynamicToolSpec {
            namespace: Some("agere_app".to_string()),
            name: "automation_update".to_string(),
            description: "Create or update automations.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            }),
            defer_loading: true,
        },
    ];
    let plain_tool_search_output = ResponseItem::ToolSearchOutput {
        call_id: Some("search-1".to_string()),
        status: "completed".to_string(),
        execution: "client".to_string(),
        tools: vec![serde_json::json!({
            "type": "function",
            "name": "tool_search",
            "description": "Plain dynamic tool named tool_search.",
            "strict": false,
            "parameters": {
                "type": "object",
                "properties": {},
                "additionalProperties": false,
            },
        })],
    };
    let previously_loaded_router = ToolRouter::from_config(
        &tools_config,
        ToolRouterParams {
            deferred_mcp_tools: None,
            mcp_tools: None,
            unavailable_called_tools: Vec::new(),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: &dynamic_tools,
            history_input: std::slice::from_ref(&plain_tool_search_output),
        },
    );
    let disambiguated_search_wire_name =
        agere_tools::FlatWireToolProjection::new(&previously_loaded_router.model_visible_specs())
            .wire_name_for_tool_search();
    let history_input = vec![
        plain_tool_search_output,
        ResponseItem::FunctionCall {
            id: None,
            name: disambiguated_search_wire_name,
            namespace: None,
            arguments: r#"{"query":"automation","limit":1}"#.to_string(),
            call_id: "search-2".to_string(),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "search-2".to_string(),
            output: agere_protocol::models::FunctionCallOutputPayload::from_text(
                serde_json::to_string(&vec![serde_json::json!({
                    "type": "namespace",
                    "name": "agere_app",
                    "description": "Agere app tools",
                    "tools": [{
                        "type": "function",
                        "name": "automation_update",
                        "description": "Create or update automations.",
                        "strict": false,
                        "parameters": {
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false,
                        },
                    }],
                })])?,
            ),
        },
    ];

    let router = ToolRouter::from_config(
        &tools_config,
        ToolRouterParams {
            deferred_mcp_tools: None,
            mcp_tools: None,
            unavailable_called_tools: Vec::new(),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: &dynamic_tools,
            history_input: &history_input,
        },
    );

    assert_eq!(
        all_namespace_function_names(&router.model_visible_specs(), "agere_app"),
        vec!["automation_update".to_string()]
    );

    Ok(())
}

#[tokio::test]
async fn normalize_history_item_restores_flat_tool_search_call() -> anyhow::Result<()> {
    let router = test_router(vec![ToolSpec::ToolSearch {
        execution: "client".to_string(),
        description: "Search deferred tools".to_string(),
        parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
    }]);

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
async fn normalize_history_item_restores_disambiguated_flat_tool_search_call() -> anyhow::Result<()>
{
    let specs = vec![
        ToolSpec::Function(ResponsesApiTool {
            name: agere_tools::TOOL_SEARCH_TOOL_NAME.to_string(),
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
    let search_wire_name =
        agere_tools::FlatWireToolProjection::new(&specs).wire_name_for_tool_search();
    let router = test_router(specs);

    let item = router.normalize_history_item(ResponseItem::FunctionCall {
        id: None,
        name: search_wire_name,
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
async fn normalize_history_item_restores_flat_tool_search_output() -> anyhow::Result<()> {
    let router = test_router(vec![ToolSpec::ToolSearch {
        execution: "client".to_string(),
        description: "Search deferred tools".to_string(),
        parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
    }]);
    let call_id = "search-1".to_string();
    let _call_item = router.normalize_history_item(ResponseItem::FunctionCall {
        id: None,
        name: "tool_search".to_string(),
        namespace: None,
        arguments: r#"{"query":"calendar","limit":1}"#.to_string(),
        call_id: call_id.clone(),
    });

    let item = router.normalize_history_item(ResponseItem::FunctionCallOutput {
        call_id: call_id.clone(),
        output: agere_protocol::models::FunctionCallOutputPayload::from_text(
            serde_json::to_string(&vec![serde_json::json!({
                "type": "namespace",
                "name": "mcp__calendar__",
                "description": "Calendar tools",
                "tools": [],
            })])?,
        ),
    });

    assert_eq!(
        item,
        ResponseItem::ToolSearchOutput {
            call_id: Some(call_id),
            status: "completed".to_string(),
            execution: "client".to_string(),
            tools: vec![serde_json::json!({
                "type": "namespace",
                "name": "mcp__calendar__",
                "description": "Calendar tools",
                "tools": [],
            })],
        }
    );

    Ok(())
}

#[tokio::test]
async fn normalize_history_item_restores_disambiguated_flat_tool_search_output()
-> anyhow::Result<()> {
    let specs = vec![
        ToolSpec::Function(ResponsesApiTool {
            name: agere_tools::TOOL_SEARCH_TOOL_NAME.to_string(),
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
    let search_wire_name =
        agere_tools::FlatWireToolProjection::new(&specs).wire_name_for_tool_search();
    let router = test_router(specs);
    let call_id = "search-1".to_string();
    let _call_item = router.normalize_history_item(ResponseItem::FunctionCall {
        id: None,
        name: search_wire_name,
        namespace: None,
        arguments: r#"{"query":"calendar","limit":1}"#.to_string(),
        call_id: call_id.clone(),
    });

    let item = router.normalize_history_item(ResponseItem::FunctionCallOutput {
        call_id: call_id.clone(),
        output: agere_protocol::models::FunctionCallOutputPayload::from_text(
            serde_json::to_string(&vec![serde_json::json!({
                "type": "namespace",
                "name": "mcp__calendar__",
                "description": "Calendar tools",
                "tools": [],
            })])?,
        ),
    });

    assert_eq!(
        item,
        ResponseItem::ToolSearchOutput {
            call_id: Some(call_id),
            status: "completed".to_string(),
            execution: "client".to_string(),
            tools: vec![serde_json::json!({
                "type": "namespace",
                "name": "mcp__calendar__",
                "description": "Calendar tools",
                "tools": [],
            })],
        }
    );

    Ok(())
}

#[tokio::test]
async fn normalize_history_item_restores_flat_namespaced_function_call() -> anyhow::Result<()> {
    let router = test_router(vec![ToolSpec::Namespace(ResponsesApiNamespace {
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
    })]);

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
            history_input: &[],
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
            history_input: &[],
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

fn all_namespace_function_names(specs: &[ToolSpec], namespace_name: &str) -> Vec<String> {
    specs
        .iter()
        .flat_map(|spec| match spec {
            ToolSpec::Namespace(namespace) if namespace.name == namespace_name => namespace
                .tools
                .iter()
                .map(|tool| match tool {
                    ResponsesApiNamespaceTool::Function(tool) => tool.name.clone(),
                })
                .collect(),
            ToolSpec::Function(_)
            | ToolSpec::Freeform(_)
            | ToolSpec::ToolSearch { .. }
            | ToolSpec::LocalShell {}
            | ToolSpec::ImageGeneration { .. }
            | ToolSpec::WebSearch { .. }
            | ToolSpec::Namespace(_) => Vec::new(),
        })
        .collect()
}

fn loaded_namespace_tool_search_output(
    namespace: &str,
    tool_name: &str,
    description: &str,
) -> ResponseItem {
    ResponseItem::ToolSearchOutput {
        call_id: Some("search-1".to_string()),
        status: "completed".to_string(),
        execution: "client".to_string(),
        tools: vec![serde_json::json!({
            "type": "namespace",
            "name": namespace,
            "description": format!("Tools in the {namespace} namespace."),
            "tools": [{
                "type": "function",
                "name": tool_name,
                "description": description,
                "strict": false,
                "parameters": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                },
            }],
        })],
    }
}

fn empty_router() -> ToolRouter {
    test_router(Vec::new())
}

fn test_router(model_visible_specs: Vec<ToolSpec>) -> ToolRouter {
    ToolRouter {
        registry: ToolRegistry::empty_for_test(),
        specs: Vec::new(),
        model_visible_specs,
        parallel_mcp_server_names: HashSet::new(),
        normalized_flat_tool_search_call_ids: std::sync::Mutex::new(HashSet::new()),
    }
}
