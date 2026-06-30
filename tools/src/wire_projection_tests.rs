use super::FlatWireToolProjection;
use super::project_function_tools_for_flat_wire_api;
use super::resolve_flattened_tool_name;
use crate::JsonSchema;
use crate::ResponsesApiNamespace;
use crate::ResponsesApiNamespaceTool;
use crate::ResponsesApiTool;
use crate::ToolName;
use crate::ToolSpec;
use pretty_assertions::assert_eq;

fn is_provider_valid_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

#[test]
fn projects_namespace_tools_as_flat_function_tools() {
    let spec = ToolSpec::Namespace(ResponsesApiNamespace {
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
    });

    let projected = project_function_tools_for_flat_wire_api(&spec);

    assert_eq!(
        projected
            .iter()
            .map(|tool| tool.wire_name.as_str())
            .collect::<Vec<_>>(),
        vec!["agere_app_automation_list"]
    );
    assert_eq!(
        projected[0].canonical_name,
        ToolName::namespaced("agere_app", "automation_list")
    );
    assert_eq!(projected[0].description, "List automations");
}

#[test]
fn namespace_ending_with_separator_does_not_add_extra_separator() {
    let spec = ToolSpec::Namespace(ResponsesApiNamespace {
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
    });

    let projected = project_function_tools_for_flat_wire_api(&spec);

    assert_eq!(projected[0].wire_name, "mcp__calendar__create_event");
    assert_eq!(
        projected[0].canonical_name,
        ToolName::namespaced("mcp__calendar__", "create_event")
    );
}

#[test]
fn projects_plain_function_tool_without_renaming() {
    let spec = ToolSpec::Function(ResponsesApiTool {
        name: "lookup".to_string(),
        description: "Lookup a value".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
        output_schema: None,
    });

    let projected = project_function_tools_for_flat_wire_api(&spec);

    assert_eq!(projected[0].wire_name, "lookup");
    assert_eq!(projected[0].canonical_name, ToolName::plain("lookup"));
}

#[test]
fn resolves_flattened_name_from_visible_specs() {
    let specs = vec![ToolSpec::Namespace(ResponsesApiNamespace {
        name: "agere_app".to_string(),
        description: "Agere app tools".to_string(),
        tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
            name: "automation_update".to_string(),
            description: "Update automation".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
            output_schema: None,
        })],
    })];

    assert_eq!(
        resolve_flattened_tool_name(&specs, "agere_app_automation_update"),
        ToolName::namespaced("agere_app", "automation_update")
    );
    assert_eq!(
        resolve_flattened_tool_name(&specs, "missing_tool"),
        ToolName::plain("missing_tool")
    );
}

#[test]
fn grouped_projection_sanitizes_invalid_provider_name_characters() {
    let specs = vec![ToolSpec::Namespace(ResponsesApiNamespace {
        name: "test_server/with spaces".to_string(),
        description: "Test server tools".to_string(),
        tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
            name: "search.now".to_string(),
            description: "Search now".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
            output_schema: None,
        })],
    })];

    let projection = FlatWireToolProjection::new(&specs);
    let tool = &projection.function_tools()[0];

    assert_eq!(tool.wire_name, "test_server_with_spaces_search_now");
    assert!(is_provider_valid_tool_name(&tool.wire_name));
    assert_eq!(
        projection.canonical_name_for_wire_name(&tool.wire_name),
        ToolName::namespaced("test_server/with spaces", "search.now")
    );
}

#[test]
fn grouped_projection_bounds_long_wire_names() {
    let namespace = format!("{}{}", "very_long_namespace_".repeat(4), "tail");
    let tool_name = format!("{}{}", "very_long_tool_".repeat(4), "tail");
    let specs = vec![ToolSpec::Namespace(ResponsesApiNamespace {
        name: namespace.clone(),
        description: "Long tools".to_string(),
        tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
            name: tool_name.clone(),
            description: "Long tool".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
            output_schema: None,
        })],
    })];

    let projection = FlatWireToolProjection::new(&specs);
    let tool = &projection.function_tools()[0];

    assert!(is_provider_valid_tool_name(&tool.wire_name));
    assert!(
        tool.wire_name
            .starts_with("very_long_namespace_very_long_namespace_")
    );
    assert_eq!(
        projection.canonical_name_for_wire_name(&tool.wire_name),
        ToolName::namespaced(namespace, tool_name)
    );
}

#[test]
fn grouped_projection_disambiguates_duplicate_wire_names() {
    let specs = vec![
        ToolSpec::Namespace(ResponsesApiNamespace {
            name: "foo".to_string(),
            description: "Foo tools".to_string(),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: "bar_baz".to_string(),
                description: "First namespaced tool".to_string(),
                strict: false,
                defer_loading: None,
                parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
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
                parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
                output_schema: None,
            })],
        }),
        ToolSpec::Function(ResponsesApiTool {
            name: "foo_bar_baz".to_string(),
            description: "Plain tool".to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
            output_schema: None,
        }),
    ];

    let projection = FlatWireToolProjection::new(&specs);
    let tools = projection.function_tools();
    let wire_names = tools
        .iter()
        .map(|tool| tool.wire_name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(wire_names.len(), 3);
    assert_eq!(
        wire_names
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
    assert!(
        wire_names
            .iter()
            .filter(|wire_name| wire_name.starts_with("foo_bar_baz__"))
            .count()
            == 3
    );
    for tool in tools {
        assert_eq!(
            projection.canonical_name_for_wire_name(&tool.wire_name),
            tool.canonical_name
        );
    }
}

#[test]
fn grouped_projection_disambiguates_against_existing_hash_suffixed_name() {
    let colliding_specs = vec![
        ToolSpec::Namespace(ResponsesApiNamespace {
            name: "foo".to_string(),
            description: "Foo tools".to_string(),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: "bar_baz".to_string(),
                description: "First namespaced tool".to_string(),
                strict: false,
                defer_loading: None,
                parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
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
                parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
                output_schema: None,
            })],
        }),
    ];
    let existing_hash_suffixed_name = FlatWireToolProjection::new(&colliding_specs)
        .function_tools()[0]
        .wire_name
        .clone();
    let mut specs = colliding_specs;
    specs.push(ToolSpec::Function(ResponsesApiTool {
        name: existing_hash_suffixed_name,
        description: "Plain tool".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(Default::default(), None, Some(false.into())),
        output_schema: None,
    }));

    let projection = FlatWireToolProjection::new(&specs);
    let wire_names = projection
        .function_tools()
        .iter()
        .map(|tool| tool.wire_name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(wire_names.len(), 3);
    assert_eq!(
        wire_names
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        3
    );
    assert!(
        wire_names
            .iter()
            .all(|name| is_provider_valid_tool_name(name))
    );
}

#[test]
fn grouped_projection_returns_wire_name_for_canonical_tool() {
    let specs = vec![ToolSpec::Namespace(ResponsesApiNamespace {
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
    })];

    let projection = FlatWireToolProjection::new(&specs);

    assert_eq!(
        projection
            .wire_name_for_canonical_name(&ToolName::namespaced("mcp__calendar__", "create_event")),
        "mcp__calendar__create_event"
    );
    assert_eq!(
        projection.wire_name_for_canonical_name(&ToolName::plain("missing")),
        "missing"
    );
}

#[test]
fn grouped_projection_fallback_wire_name_is_provider_valid() {
    let projection = FlatWireToolProjection::new(&[]);
    let canonical_name = ToolName::namespaced(
        "test_server/with spaces/and/a/very/very/very/very/very/long/path",
        "search.now.with.a.very.very.very.very.very.long.name",
    );

    let wire_name = projection.wire_name_for_canonical_name(&canonical_name);

    assert!(is_provider_valid_tool_name(&wire_name));
    assert!(wire_name.starts_with("test_server_with_spaces_and_a_very_very_very_very"));
}
