use super::AuthRequestTelemetryContext;
use super::ModelClient;
use super::ModelClientSession;
use super::PendingUnauthorizedRetry;
use super::UnauthorizedRecoveryExecution;
use super::X_AGERE_INSTALLATION_ID_HEADER;
use super::X_AGERE_PARENT_THREAD_ID_HEADER;
use super::X_AGERE_TURN_METADATA_HEADER;
use super::X_AGERE_WINDOW_ID_HEADER;
use super::X_OPENAI_SUBAGENT_HEADER;
use super::anthropic_tool_definitions_from_function_spec;
use super::chat_tool_definitions_from_function_spec;
use super::input_items_for_flat_wire_api;
use super::input_items_for_responses_provider;
use agere_api::ApiError;
use agere_api::ResponseEvent;
use agere_app_server_protocol::AuthMode;
use agere_model_provider::BearerAuthProvider;
use agere_model_provider_info::ModelProviderAwsAuthInfo;
use agere_model_provider_info::WireApi;
use agere_model_provider_info::create_oss_provider_with_base_url;
use agere_otel::SessionTelemetry;
use agere_protocol::ThreadId;
use agere_protocol::config_types::ReasoningSummary;
use agere_protocol::config_types::ServiceTier;
use agere_protocol::config_types::WebSearchContextSize;
use agere_protocol::models::BaseInstructions;
use agere_protocol::models::ContentItem;
use agere_protocol::models::FunctionCallOutputPayload;
use agere_protocol::models::ReasoningItemReasoningSummary;
use agere_protocol::models::ResponseItem;
use agere_protocol::openai_models::ModelInfo;
use agere_protocol::protocol::InternalSessionSource;
use agere_protocol::protocol::SessionSource;
use agere_protocol::protocol::SubAgentSource;
use agere_rollout_trace::ExecutionStatus;
use agere_rollout_trace::InferenceTraceAttempt;
use agere_rollout_trace::InferenceTraceContext;
use agere_rollout_trace::RawTraceEventPayload;
use agere_rollout_trace::RolloutTrace;
use agere_rollout_trace::TraceWriter;
use agere_rollout_trace::replay_bundle;
use agere_tools::JsonSchema;
use agere_tools::ResponsesApiNamespace;
use agere_tools::ResponsesApiNamespaceTool;
use agere_tools::ResponsesApiTool;
use agere_tools::ResponsesApiWebSearchFilters;
use agere_tools::ToolSpec;
use agere_tools::create_apply_patch_freeform_tool;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::Notify;

fn test_model_client(session_source: SessionSource) -> ModelClient {
    let provider = create_oss_provider_with_base_url("https://example.com/v1", WireApi::Responses);
    ModelClient::new(
        /*auth_manager*/ None,
        ThreadId::new(),
        /*installation_id*/ "11111111-1111-4111-8111-111111111111".to_string(),
        provider,
        session_source,
        /*model_verbosity*/ None,
        /*enable_request_compression*/ false,
        /*include_timing_metrics*/ false,
        /*beta_features_header*/ None,
    )
}

fn test_model_info() -> ModelInfo {
    serde_json::from_value(json!({
        "slug": "gpt-test",
        "display_name": "gpt-test",
        "description": "desc",
        "default_reasoning_level": "medium",
        "supported_reasoning_levels": [
            {"effort": "medium", "description": "medium"}
        ],
        "shell_type": "shell_command",
        "visibility": "list",
        "supported_in_api": true,
        "priority": 1,
        "upgrade": null,
        "base_instructions": "base instructions",
        "model_messages": null,
        "supports_reasoning_summaries": false,
        "support_verbosity": false,
        "default_verbosity": null,
        "apply_patch_tool_type": null,
        "truncation_policy": {"mode": "bytes", "limit": 10000},
        "supports_parallel_tool_calls": false,
        "supports_image_detail_original": false,
        "context_window": 272000,
        "auto_compact_token_limit": null,
        "experimental_supported_tools": []
    }))
    .expect("deserialize test model info")
}

fn test_session_telemetry() -> SessionTelemetry {
    SessionTelemetry::new(
        ThreadId::new(),
        "gpt-test",
        "gpt-test",
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "test-originator".to_string(),
        /*log_user_prompts*/ false,
        "test-terminal".to_string(),
        SessionSource::Cli,
    )
}

fn reasoning_with_provider_fields() -> ResponseItem {
    ResponseItem::Reasoning {
        id: "reasoning-1".to_string(),
        summary: vec![ReasoningItemReasoningSummary::SummaryText {
            text: "thinking".to_string(),
        }],
        content: None,
        encrypted_content: Some("encrypted".to_string()),
        signature: Some("anthropic-signature".to_string()),
    }
}

#[test]
fn compact_input_sanitization_drops_provider_specific_reasoning_for_responses() {
    let client = test_model_client(SessionSource::Cli);
    let input = vec![
        ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "hello".to_string(),
            }],
            phase: None,
        },
        reasoning_with_provider_fields(),
    ];

    let sanitized = client.sanitize_prompt_input_for_current_provider(&input);

    assert_eq!(sanitized.len(), input.len());
    assert_eq!(sanitized[0], input[0]);
    let ResponseItem::Reasoning {
        encrypted_content,
        signature,
        summary,
        ..
    } = &sanitized[1]
    else {
        panic!("expected reasoning item");
    };
    assert!(encrypted_content.is_none());
    assert!(signature.is_none());
    assert_eq!(
        summary,
        &vec![ReasoningItemReasoningSummary::SummaryText {
            text: "thinking".to_string(),
        }]
    );
}

#[test]
fn stream_sanitization_uses_turn_provider_switch_signal() {
    let input = vec![ResponseItem::Reasoning {
        id: "reasoning-1".to_string(),
        summary: vec![ReasoningItemReasoningSummary::SummaryText {
            text: "thinking".to_string(),
        }],
        content: None,
        encrypted_content: None,
        signature: None,
    }];

    let sanitized = ModelClientSession::sanitize_prompt_input_for_stream(
        WireApi::Anthropic,
        &input,
        /*provider_changed*/ false,
        /*provider_changed_for_turn*/ true,
    );

    assert_eq!(
        sanitized,
        vec![ResponseItem::Message {
            id: None,
            role: "assistant".to_string(),
            content: vec![ContentItem::InputText {
                text: "thinking".to_string(),
            }],
            phase: None,
        }]
    );
}

#[test]
fn stream_sanitization_preserves_anthropic_without_provider_switch_signal() {
    let input = vec![ResponseItem::Reasoning {
        id: "reasoning-1".to_string(),
        summary: vec![ReasoningItemReasoningSummary::SummaryText {
            text: "thinking".to_string(),
        }],
        content: None,
        encrypted_content: None,
        signature: None,
    }];

    let sanitized = ModelClientSession::sanitize_prompt_input_for_stream(
        WireApi::Anthropic,
        &input,
        /*provider_changed*/ false,
        /*provider_changed_for_turn*/ false,
    );

    assert_eq!(sanitized, input);
}

#[test]
fn non_responses_tool_translation_skips_freeform_apply_patch() {
    let apply_patch = create_apply_patch_freeform_tool();

    assert!(chat_tool_definitions_from_function_spec(&apply_patch).is_empty());
    assert!(anthropic_tool_definitions_from_function_spec(&apply_patch).is_empty());
}

#[test]
fn non_responses_tool_translation_keeps_function_tools() {
    let mut properties = BTreeMap::new();
    properties.insert("query".to_string(), JsonSchema::string(None));
    let tool = ToolSpec::Function(ResponsesApiTool {
        name: "lookup".to_string(),
        description: "Lookup a value".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            properties,
            Some(vec!["query".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    });

    let chat_tools = chat_tool_definitions_from_function_spec(&tool);
    let anthropic_tools = anthropic_tool_definitions_from_function_spec(&tool);

    assert_eq!(chat_tools.len(), 1);
    assert_eq!(anthropic_tools.len(), 1);
    assert_eq!(chat_tools[0].parameters["type"], "object");
    assert_eq!(anthropic_tools[0].parameters["type"], "object");
}

#[test]
fn non_responses_tool_translation_flattens_namespace_tools() {
    let namespace = ToolSpec::Namespace(ResponsesApiNamespace {
        name: "mcp__calendar__".to_string(),
        description: "Calendar tools".to_string(),
        tools: vec![
            ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: "create_event".to_string(),
                description: "Create an event".to_string(),
                strict: false,
                defer_loading: None,
                parameters: JsonSchema::object(
                    BTreeMap::new(),
                    /*required*/ None,
                    Some(false.into()),
                ),
                output_schema: None,
            }),
            ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: "_list_events".to_string(),
                description: "List events".to_string(),
                strict: false,
                defer_loading: None,
                parameters: JsonSchema::object(
                    BTreeMap::new(),
                    /*required*/ None,
                    Some(false.into()),
                ),
                output_schema: None,
            }),
        ],
    });

    let chat_tools = chat_tool_definitions_from_function_spec(&namespace);
    let anthropic_tools = anthropic_tool_definitions_from_function_spec(&namespace);

    assert_eq!(
        chat_tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        vec!["mcp__calendar__create_event", "mcp__calendar___list_events"]
    );
    assert_eq!(
        anthropic_tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>(),
        vec!["mcp__calendar__create_event", "mcp__calendar___list_events"]
    );
    assert_eq!(chat_tools[0].description, "Create an event");
    assert_eq!(anthropic_tools[1].description, "List events");
}

#[test]
fn flat_wire_api_input_rewrites_namespaced_function_call_history() {
    let tools = vec![ToolSpec::Namespace(ResponsesApiNamespace {
        name: "mcp__calendar__".to_string(),
        description: "Calendar tools".to_string(),
        tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
            name: "create_event".to_string(),
            description: "Create an event".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
            output_schema: None,
        })],
    })];
    let input = vec![
        ResponseItem::FunctionCall {
            id: None,
            name: "create_event".to_string(),
            namespace: Some("mcp__calendar__".to_string()),
            arguments: "{}".to_string(),
            call_id: "call-1".to_string(),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "call-1".to_string(),
            output: FunctionCallOutputPayload::from_text("ok".to_string()),
        },
    ];

    let projection = agere_tools::FlatWireToolProjection::new(&tools);
    let rewritten = input_items_for_flat_wire_api(&input, &projection);

    assert_eq!(
        rewritten[0],
        ResponseItem::FunctionCall {
            id: None,
            name: "mcp__calendar__create_event".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: "call-1".to_string(),
        }
    );
    assert_eq!(rewritten[1], input[1]);
}

#[test]
fn flat_wire_api_input_rewrites_tool_search_history_as_function_history() {
    let tools = vec![ToolSpec::ToolSearch {
        execution: "client".to_string(),
        description: "Search deferred tools".to_string(),
        parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
    }];
    let input = vec![
        ResponseItem::ToolSearchCall {
            id: None,
            call_id: Some("search-1".to_string()),
            status: Some("completed".to_string()),
            execution: "client".to_string(),
            arguments: json!({"query": "calendar"}),
        },
        ResponseItem::ToolSearchOutput {
            call_id: Some("search-1".to_string()),
            status: "completed".to_string(),
            execution: "client".to_string(),
            tools: vec![json!({
                "type": "namespace",
                "name": "mcp__calendar__",
                "description": "Calendar tools",
                "tools": []
            })],
        },
    ];

    let projection = agere_tools::FlatWireToolProjection::new(&tools);
    let rewritten = input_items_for_flat_wire_api(&input, &projection);

    assert_eq!(
        rewritten[0],
        ResponseItem::FunctionCall {
            id: None,
            name: "tool_search".to_string(),
            namespace: None,
            arguments: r#"{"query":"calendar"}"#.to_string(),
            call_id: "search-1".to_string(),
        }
    );
    match &rewritten[1] {
        ResponseItem::FunctionCallOutput { call_id, output } => {
            assert_eq!(call_id, "search-1");
            assert_eq!(
                output.text_content(),
                Some(
                    r#"[{"description":"Calendar tools","name":"mcp__calendar__","tools":[],"type":"namespace"}]"#
                )
            );
        }
        other => panic!("expected function output, got {other:?}"),
    }
}

#[test]
fn flat_wire_api_input_does_not_rewrite_server_tool_search_output() {
    let tools = vec![ToolSpec::ToolSearch {
        execution: "client".to_string(),
        description: "Search deferred tools".to_string(),
        parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
    }];
    let input = vec![ResponseItem::ToolSearchOutput {
        call_id: Some("search-1".to_string()),
        status: "completed".to_string(),
        execution: "server".to_string(),
        tools: vec![json!({
            "type": "namespace",
            "name": "mcp__calendar__",
            "description": "Calendar tools",
            "tools": []
        })],
    }];

    let projection = agere_tools::FlatWireToolProjection::new(&tools);
    let rewritten = input_items_for_flat_wire_api(&input, &projection);

    assert_eq!(rewritten, input);
}

#[test]
fn responses_tools_flatten_namespace_specs_when_provider_disables_namespace_tools() {
    let namespace = ToolSpec::Namespace(ResponsesApiNamespace {
        name: "mcp__calendar__".to_string(),
        description: "Calendar tools".to_string(),
        tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
            name: "create_event".to_string(),
            description: "Create an event".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
            output_schema: None,
        })],
    });

    let tools = super::responses_tools_json_for_provider(&[namespace], false)
        .expect("responses tools should serialize");

    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["type"], "function");
    assert_eq!(tools[0]["name"], "mcp__calendar__create_event");
}

#[test]
fn responses_tools_preserve_native_specs_when_provider_disables_namespace_tools() {
    let tools = super::responses_tools_json_for_provider(
        &[
            ToolSpec::Namespace(ResponsesApiNamespace {
                name: "mcp__calendar__".to_string(),
                description: "Calendar tools".to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: "create_event".to_string(),
                    description: "Create an event".to_string(),
                    strict: false,
                    defer_loading: None,
                    parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
                    output_schema: None,
                })],
            }),
            ToolSpec::WebSearch {
                external_web_access: Some(true),
                filters: Some(ResponsesApiWebSearchFilters {
                    allowed_domains: Some(vec!["example.com".to_string()]),
                }),
                user_location: None,
                search_context_size: Some(WebSearchContextSize::High),
                search_content_types: None,
            },
        ],
        false,
    )
    .expect("responses tools should serialize");

    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["type"], "function");
    assert_eq!(tools[0]["name"], "mcp__calendar__create_event");
    assert_eq!(tools[1]["type"], "web_search");
    assert_eq!(tools[1]["external_web_access"], true);
}

#[test]
fn responses_request_flattens_history_when_provider_disables_namespace_tools() {
    let mut provider =
        create_oss_provider_with_base_url("https://bedrock.example.com/v1", WireApi::Responses);
    provider.aws = Some(ModelProviderAwsAuthInfo {
        profile: None,
        region: Some("us-east-1".to_string()),
    });
    let client = ModelClient::new(
        /*auth_manager*/ None,
        ThreadId::new(),
        /*installation_id*/ "11111111-1111-4111-8111-111111111111".to_string(),
        provider.clone(),
        SessionSource::Cli,
        /*model_verbosity*/ None,
        /*enable_request_compression*/ false,
        /*include_timing_metrics*/ false,
        /*beta_features_header*/ None,
    );
    let session = client.new_session();
    let prompt = super::Prompt {
        input: vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "create_event".to_string(),
                namespace: Some("mcp__calendar__".to_string()),
                arguments: "{}".to_string(),
                call_id: "call-event".to_string(),
            },
            ResponseItem::ToolSearchCall {
                id: None,
                call_id: Some("search-1".to_string()),
                status: Some("completed".to_string()),
                execution: "client".to_string(),
                arguments: json!({"query": "calendar"}),
            },
            ResponseItem::ToolSearchOutput {
                call_id: Some("search-1".to_string()),
                status: "completed".to_string(),
                execution: "client".to_string(),
                tools: vec![json!({
                    "type": "namespace",
                    "name": "mcp__calendar__",
                    "description": "Calendar tools",
                    "tools": []
                })],
            },
        ],
        tools: vec![
            ToolSpec::Namespace(ResponsesApiNamespace {
                name: "mcp__calendar__".to_string(),
                description: "Calendar tools".to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: "create_event".to_string(),
                    description: "Create an event".to_string(),
                    strict: false,
                    defer_loading: None,
                    parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
                    output_schema: None,
                })],
            }),
            ToolSpec::ToolSearch {
                execution: "client".to_string(),
                description: "Search deferred tools".to_string(),
                parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
            },
        ],
        base_instructions: BaseInstructions {
            text: "base".to_string(),
        },
        ..Default::default()
    };
    let api_provider = provider
        .to_api_provider(/*auth_mode*/ None)
        .expect("provider should convert");

    let request = session
        .build_responses_request(
            &api_provider,
            &prompt,
            &test_model_info(),
            /*effort*/ None,
            ReasoningSummary::None,
            Option::<ServiceTier>::None,
        )
        .expect("request should build");

    assert_eq!(request.tools[0]["type"], "function");
    assert_eq!(request.tools[0]["name"], "mcp__calendar__create_event");
    assert_eq!(request.tools[1]["type"], "function");
    assert_eq!(request.tools[1]["name"], "tool_search");
    assert_eq!(
        request.input[0],
        ResponseItem::FunctionCall {
            id: None,
            name: "mcp__calendar__create_event".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: "call-event".to_string(),
        }
    );
    assert_eq!(
        request.input[1],
        ResponseItem::FunctionCall {
            id: None,
            name: "tool_search".to_string(),
            namespace: None,
            arguments: r#"{"query":"calendar"}"#.to_string(),
            call_id: "search-1".to_string(),
        }
    );
    assert!(matches!(
        request.input[2],
        ResponseItem::FunctionCallOutput { .. }
    ));
}

#[test]
fn responses_provider_input_restores_native_history_when_namespace_tools_are_supported() {
    let tools = vec![
        ToolSpec::Namespace(ResponsesApiNamespace {
            name: "mcp__calendar__".to_string(),
            description: "Calendar tools".to_string(),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: "create_event".to_string(),
                description: "Create an event".to_string(),
                strict: false,
                defer_loading: None,
                parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
                output_schema: None,
            })],
        }),
        ToolSpec::ToolSearch {
            execution: "client".to_string(),
            description: "Search deferred tools".to_string(),
            parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
        },
    ];
    let input = vec![
        ResponseItem::FunctionCall {
            id: None,
            name: "mcp__calendar__create_event".to_string(),
            namespace: None,
            arguments: "{}".to_string(),
            call_id: "call-event".to_string(),
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "tool_search".to_string(),
            namespace: None,
            arguments: r#"{"query":"calendar"}"#.to_string(),
            call_id: "search-1".to_string(),
        },
        ResponseItem::FunctionCallOutput {
            call_id: "search-1".to_string(),
            output: FunctionCallOutputPayload::from_text(
                r#"[{"description":"Calendar tools","name":"mcp__calendar__","tools":[],"type":"namespace"}]"#
                    .to_string(),
            ),
        },
    ];

    let rewritten = input_items_for_responses_provider(&input, &tools, true);

    assert_eq!(
        rewritten[0],
        ResponseItem::FunctionCall {
            id: None,
            name: "create_event".to_string(),
            namespace: Some("mcp__calendar__".to_string()),
            arguments: "{}".to_string(),
            call_id: "call-event".to_string(),
        }
    );
    assert_eq!(
        rewritten[1],
        ResponseItem::ToolSearchCall {
            id: None,
            call_id: Some("search-1".to_string()),
            status: Some("completed".to_string()),
            execution: "client".to_string(),
            arguments: json!({"query":"calendar"}),
        }
    );
    assert_eq!(
        rewritten[2],
        ResponseItem::ToolSearchOutput {
            call_id: Some("search-1".to_string()),
            status: "completed".to_string(),
            execution: "client".to_string(),
            tools: vec![json!({
                "description": "Calendar tools",
                "name": "mcp__calendar__",
                "tools": [],
                "type": "namespace"
            })],
        }
    );
}

#[test]
fn responses_provider_restores_flat_tool_search_history_and_advertises_loaded_tools() {
    let mut provider =
        create_oss_provider_with_base_url("https://api.example.com/v1", WireApi::Responses);
    provider.wire_api = WireApi::Responses;
    let client = ModelClient::new(
        /*auth_manager*/ None,
        ThreadId::new(),
        /*installation_id*/ "11111111-1111-4111-8111-111111111111".to_string(),
        provider.clone(),
        SessionSource::Cli,
        /*model_verbosity*/ None,
        /*enable_request_compression*/ false,
        /*include_timing_metrics*/ false,
        /*beta_features_header*/ None,
    );
    let session = client.new_session();
    let loaded_tool = ToolSpec::Namespace(ResponsesApiNamespace {
        name: "mcp__calendar__".to_string(),
        description: "Calendar tools".to_string(),
        tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
            name: "create_event".to_string(),
            description: "Create an event".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
            output_schema: None,
        })],
    });
    let prompt = super::Prompt {
        input: vec![
            ResponseItem::FunctionCall {
                id: None,
                name: "tool_search".to_string(),
                namespace: None,
                arguments: r#"{"query":"calendar"}"#.to_string(),
                call_id: "search-1".to_string(),
            },
            ResponseItem::FunctionCallOutput {
                call_id: "search-1".to_string(),
                output: FunctionCallOutputPayload::from_text(
                    r#"[{"description":"Calendar tools","name":"mcp__calendar__","tools":[{"description":"Create an event","name":"create_event","parameters":{"additionalProperties":false,"properties":{},"type":"object"},"strict":false,"type":"function"}],"type":"namespace"}]"#
                        .to_string(),
                ),
            },
        ],
        tools: vec![
            ToolSpec::ToolSearch {
                execution: "client".to_string(),
                description: "Search deferred tools".to_string(),
                parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
            },
            loaded_tool,
        ],
        base_instructions: BaseInstructions {
            text: "base".to_string(),
        },
        ..Default::default()
    };
    let api_provider = provider
        .to_api_provider(/*auth_mode*/ None)
        .expect("provider should convert");

    let request = session
        .build_responses_request(
            &api_provider,
            &prompt,
            &test_model_info(),
            /*effort*/ None,
            ReasoningSummary::None,
            Option::<ServiceTier>::None,
        )
        .expect("request should build");

    assert_eq!(request.tools.len(), 2);
    assert_eq!(request.tools[0]["type"], "tool_search");
    assert_eq!(request.tools[1]["type"], "namespace");
    assert_eq!(request.tools[1]["name"], "mcp__calendar__");
    assert_eq!(
        request.input[0],
        ResponseItem::ToolSearchCall {
            id: None,
            call_id: Some("search-1".to_string()),
            status: Some("completed".to_string()),
            execution: "client".to_string(),
            arguments: json!({"query":"calendar"}),
        }
    );
    assert!(matches!(
        request.input[1],
        ResponseItem::ToolSearchOutput { .. }
    ));
}

#[test]
fn flat_wire_api_input_uses_disambiguated_history_names() {
    let tools = vec![
        ToolSpec::Namespace(ResponsesApiNamespace {
            name: "foo".to_string(),
            description: "Foo tools".to_string(),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: "bar_baz".to_string(),
                description: "First namespaced tool".to_string(),
                strict: false,
                defer_loading: None,
                parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
                output_schema: None,
            })],
        }),
        ToolSpec::Namespace(ResponsesApiNamespace {
            name: "foo_bar".to_string(),
            description: "Foo bar tools".to_string(),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: "baz".to_string(),
                description: "Second namespaced tool".to_string(),
                strict: false,
                defer_loading: None,
                parameters: JsonSchema::object(BTreeMap::new(), None, Some(false.into())),
                output_schema: None,
            })],
        }),
    ];
    let projection = agere_tools::FlatWireToolProjection::new(&tools);
    let input = vec![
        ResponseItem::FunctionCall {
            id: None,
            name: "bar_baz".to_string(),
            namespace: Some("foo".to_string()),
            arguments: "{}".to_string(),
            call_id: "call-1".to_string(),
        },
        ResponseItem::FunctionCall {
            id: None,
            name: "baz".to_string(),
            namespace: Some("foo_bar".to_string()),
            arguments: "{}".to_string(),
            call_id: "call-2".to_string(),
        },
    ];

    let rewritten = input_items_for_flat_wire_api(&input, &projection);
    let names = rewritten
        .iter()
        .map(|item| match item {
            ResponseItem::FunctionCall {
                name, namespace, ..
            } => {
                assert!(namespace.is_none());
                name.as_str()
            }
            other => panic!("expected function call, got {other:?}"),
        })
        .collect::<Vec<_>>();

    assert_eq!(names.len(), 2);
    assert_ne!(names[0], names[1]);
    assert!(names.iter().all(|name| name.starts_with("foo_bar_baz__")));
}

fn started_inference_attempt(temp: &TempDir) -> anyhow::Result<InferenceTraceAttempt> {
    let writer = Arc::new(TraceWriter::create(
        temp.path(),
        "trace-1".to_string(),
        "rollout-1".to_string(),
        "thread-root".to_string(),
    )?);
    writer.append(RawTraceEventPayload::ThreadStarted {
        thread_id: "thread-root".to_string(),
        agent_path: "/root".to_string(),
        metadata_payload: None,
    })?;
    writer.append(RawTraceEventPayload::AgereTurnStarted {
        agere_turn_id: "turn-1".to_string(),
        thread_id: "thread-root".to_string(),
    })?;

    let inference_trace = InferenceTraceContext::enabled(
        writer,
        "thread-root".to_string(),
        "turn-1".to_string(),
        "gpt-test".to_string(),
        "test-provider".to_string(),
    );
    let attempt = inference_trace.start_attempt();
    attempt.record_started(&json!({
        "model": "gpt-test",
        "input": [{
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": "hello"}]
        }],
    }));
    Ok(attempt)
}

fn output_message(id: &str, text: &str) -> ResponseItem {
    ResponseItem::Message {
        id: Some(id.to_string()),
        role: "assistant".to_string(),
        content: vec![ContentItem::OutputText {
            text: text.to_string(),
        }],
        phase: None,
    }
}

async fn replay_until_cancelled(temp: &TempDir) -> anyhow::Result<RolloutTrace> {
    let mut rollout = replay_bundle(temp.path())?;
    for _ in 0..50 {
        let inference = rollout
            .inference_calls
            .values()
            .next()
            .expect("inference should be reduced");
        if inference.execution.status == ExecutionStatus::Cancelled {
            return Ok(rollout);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
        rollout = replay_bundle(temp.path())?;
    }
    Ok(rollout)
}

struct NotifyAfterEventStream {
    events: VecDeque<ResponseEvent>,
    yielded: usize,
    notify_after: usize,
    notify: Arc<Notify>,
}

impl futures::Stream for NotifyAfterEventStream {
    type Item = std::result::Result<ResponseEvent, ApiError>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Some(event) = self.events.pop_front() else {
            return Poll::Pending;
        };
        self.yielded += 1;
        if self.yielded == self.notify_after {
            self.notify.notify_one();
        }
        Poll::Ready(Some(Ok(event)))
    }
}

#[test]
fn build_subagent_headers_sets_other_subagent_label() {
    let client = test_model_client(SessionSource::SubAgent(SubAgentSource::Other(
        "memory_consolidation".to_string(),
    )));
    let headers = client.build_subagent_headers();
    let value = headers
        .get(X_OPENAI_SUBAGENT_HEADER)
        .and_then(|value| value.to_str().ok());
    assert_eq!(value, Some("memory_consolidation"));
}

#[test]
fn build_subagent_headers_sets_internal_memory_consolidation_label() {
    let client = test_model_client(SessionSource::Internal(
        InternalSessionSource::MemoryConsolidation,
    ));
    let headers = client.build_subagent_headers();
    let value = headers
        .get(X_OPENAI_SUBAGENT_HEADER)
        .and_then(|value| value.to_str().ok());
    assert_eq!(value, Some("memory_consolidation"));
}

#[test]
fn build_ws_client_metadata_includes_window_lineage_and_turn_metadata() {
    let parent_thread_id = ThreadId::new();
    let client = test_model_client(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
        parent_thread_id,
        depth: 2,
        agent_path: None,
        agent_nickname: None,
        agent_role: None,
    }));

    client.advance_window_generation();

    let client_metadata = client.build_ws_client_metadata(Some(r#"{"turn_id":"turn-123"}"#));
    let conversation_id = client.state.conversation_id;
    assert_eq!(
        client_metadata,
        std::collections::HashMap::from([
            (
                X_AGERE_INSTALLATION_ID_HEADER.to_string(),
                "11111111-1111-4111-8111-111111111111".to_string(),
            ),
            (
                X_AGERE_WINDOW_ID_HEADER.to_string(),
                format!("{conversation_id}:1"),
            ),
            (
                X_OPENAI_SUBAGENT_HEADER.to_string(),
                "collab_spawn".to_string(),
            ),
            (
                X_AGERE_PARENT_THREAD_ID_HEADER.to_string(),
                parent_thread_id.to_string(),
            ),
            (
                X_AGERE_TURN_METADATA_HEADER.to_string(),
                r#"{"turn_id":"turn-123"}"#.to_string(),
            ),
        ])
    );
}

#[test]
fn provider_switch_invalidates_existing_turn_websocket_session() {
    let client = test_model_client(SessionSource::Cli);
    let mut session = client.new_session();
    session.mark_websocket_session_used_for_tests();

    let switched_provider =
        create_oss_provider_with_base_url("https://other.example.com/v1", WireApi::Responses);
    client.set_provider(switched_provider);
    session.reset_websocket_session_if_provider_changed();

    assert_eq!(session.provider_generation_for_tests(), 1);
    assert!(!session.websocket_session_has_cached_request_for_tests());
    assert!(!session.websocket_connection_reused_for_tests());
}

#[test]
fn provider_switch_resets_websocket_fallback_latch() {
    let mut provider =
        create_oss_provider_with_base_url("https://example.com/v1", WireApi::Responses);
    provider.supports_websockets = true;
    let client = ModelClient::new(
        /*auth_manager*/ None,
        ThreadId::new(),
        /*installation_id*/ "11111111-1111-4111-8111-111111111111".to_string(),
        provider,
        SessionSource::Cli,
        /*model_verbosity*/ None,
        /*enable_request_compression*/ false,
        /*include_timing_metrics*/ false,
        /*beta_features_header*/ None,
    );
    let model_info = test_model_info();
    let session_telemetry = test_session_telemetry();

    assert!(client.responses_websocket_enabled());
    assert!(client.force_http_fallback(&session_telemetry, &model_info));
    assert!(!client.responses_websocket_enabled());

    let mut switched_provider =
        create_oss_provider_with_base_url("https://other.example.com/v1", WireApi::Responses);
    switched_provider.supports_websockets = true;
    client.set_provider(switched_provider);

    assert!(client.responses_websocket_enabled());
}

#[tokio::test]
async fn summarize_memories_returns_empty_for_empty_input() {
    let client = test_model_client(SessionSource::Cli);
    let model_info = test_model_info();
    let session_telemetry = test_session_telemetry();

    let output = client
        .summarize_memories(
            Vec::new(),
            &model_info,
            /*effort*/ None,
            &session_telemetry,
        )
        .await
        .expect("empty summarize request should succeed");
    assert_eq!(output.len(), 0);
}

#[tokio::test]
async fn dropped_response_stream_traces_cancelled_partial_output() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let attempt = started_inference_attempt(&temp)?;

    // The provider has produced one complete output item, but no terminal
    // response.completed event. The harness has enough information to keep this
    // item in history, so the trace should preserve it when the stream is
    // abandoned.
    let item = output_message("msg-1", "partial answer");
    let api_stream = futures::stream::iter([Ok(ResponseEvent::output_item_done(item))])
        .chain(futures::stream::pending());
    let (mut stream, _) = super::map_response_events(
        /*upstream_request_id*/ None,
        api_stream,
        test_session_telemetry(),
        attempt,
    );

    let observed = stream
        .next()
        .await
        .expect("mapped stream should yield output item")?;
    assert!(matches!(observed, ResponseEvent::OutputItemDone { .. }));

    // Dropping the consumer is how turn interruption/preemption stops polling
    // the provider stream. The mapper task observes that drop asynchronously
    // and records cancellation using the output items it has already seen.
    drop(stream);

    // Cancellation is recorded by the mapper task after Drop wakes it, so the
    // replay may need a short wait before the terminal event appears on disk.
    let rollout = replay_until_cancelled(&temp).await?;
    let inference = rollout
        .inference_calls
        .values()
        .next()
        .expect("inference should be reduced");

    assert_eq!(inference.execution.status, ExecutionStatus::Cancelled);
    assert_eq!(inference.response_item_ids.len(), 1);
    assert_eq!(rollout.raw_payloads.len(), 2);

    Ok(())
}

#[tokio::test]
async fn dropped_backpressured_response_stream_traces_cancelled_partial_output()
-> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let attempt = started_inference_attempt(&temp)?;
    let backpressured_item_yielded = Arc::new(Notify::new());
    let mut events = VecDeque::new();
    for _ in 0..super::RESPONSE_STREAM_CHANNEL_CAPACITY {
        events.push_back(ResponseEvent::Created);
    }
    events.push_back(ResponseEvent::output_item_done(output_message(
        "msg-1",
        "partial answer",
    )));
    let api_stream = NotifyAfterEventStream {
        events,
        yielded: 0,
        notify_after: super::RESPONSE_STREAM_CHANNEL_CAPACITY + 1,
        notify: Arc::clone(&backpressured_item_yielded),
    };

    let (stream, _) = super::map_response_events(
        /*upstream_request_id*/ None,
        api_stream,
        test_session_telemetry(),
        attempt,
    );

    // Fill the mapper channel with non-terminal events, then yield one output
    // item. The mapper has observed that item and is blocked trying to send it
    // downstream, so dropping the consumer covers the send-failure path rather
    // than the `consumer_dropped` select branch.
    backpressured_item_yielded.notified().await;
    drop(stream);

    let rollout = replay_until_cancelled(&temp).await?;
    let inference = rollout
        .inference_calls
        .values()
        .next()
        .expect("inference should be reduced");

    assert_eq!(inference.execution.status, ExecutionStatus::Cancelled);
    assert_eq!(inference.response_item_ids.len(), 1);
    assert_eq!(rollout.raw_payloads.len(), 2);

    Ok(())
}

#[test]
fn auth_request_telemetry_context_tracks_attached_auth_and_retry_phase() {
    let auth_context = AuthRequestTelemetryContext::new(
        Some(AuthMode::Chatgpt),
        &BearerAuthProvider::for_test(Some("access-token"), Some("workspace-123")),
        PendingUnauthorizedRetry::from_recovery(UnauthorizedRecoveryExecution {
            mode: "managed",
            phase: "refresh_token",
        }),
    );

    assert_eq!(auth_context.auth_mode, Some("Chatgpt"));
    assert!(auth_context.auth_header_attached);
    assert_eq!(auth_context.auth_header_name, Some("authorization"));
    assert!(auth_context.retry_after_unauthorized);
    assert_eq!(auth_context.recovery_mode, Some("managed"));
    assert_eq!(auth_context.recovery_phase, Some("refresh_token"));
}
