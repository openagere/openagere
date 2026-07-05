use crate::ToolDefinition;
use crate::translate::content::content_item_to_anthropic;
use crate::translate::content::parse_data_url;
use crate::translate::thinking::to_anthropic_output_config;
use crate::translate::thinking::to_anthropic_thinking;
use crate::translate::tools::to_anthropic_tools;
use crate::types::ImageSource;
use crate::types::Message;
use crate::types::MessageContent;
use crate::types::MessagesRequest;
use crate::types::SystemPrompt;
use crate::types::ToolResultBlock;
use crate::types::ToolResultContent;
use agere_protocol::models::ContentItem;
use agere_protocol::models::FunctionCallOutputContentItem;
use agere_protocol::models::FunctionCallOutputPayload;
use agere_protocol::models::ResponseInputItem;
use agere_protocol::models::ResponseItem;
use agere_protocol::openai_models::ReasoningEffort;
use std::collections::HashMap;
use tracing::debug;
use tracing::warn;

/// Build an Anthropic MessagesRequest from ResponseItems directly.
/// Handles FunctionCall → tool_use, FunctionCallOutput → tool_result,
/// and Reasoning → assistant text conversion.
pub(crate) fn build_anthropic_request(
    model: &str,
    system: &str,
    input_items: &[ResponseInputItem],
    tools: &[ToolDefinition],
    thinking: Option<ReasoningEffort>,
    max_tokens: u32,
    temperature: Option<f32>,
    top_p: Option<f32>,
    top_k: Option<u32>,
    tool_choice: Option<crate::types::ToolChoice>,
) -> MessagesRequest {
    MessagesRequest {
        model: model.to_string(),
        messages: convert_input_items(input_items),
        system: if system.is_empty() {
            None
        } else {
            Some(SystemPrompt::Text(system.to_string()))
        },
        max_tokens,
        temperature,
        top_p,
        top_k,
        stop_sequences: None,
        thinking: to_anthropic_thinking(thinking),
        output_config: to_anthropic_output_config(thinking),
        tools: to_anthropic_tools(tools),
        tool_choice,
        stream: true,
        metadata: None,
    }
}

/// Context for controlling how ResponseItems are converted to Anthropic messages.
/// New flags can be added here as needed without changing the function signature.
pub struct MessageBuildContext {
    /// When true (proxy providers like OpenRouter), thinking blocks without a
    /// signature are downgraded to plain text. When false (native Anthropic API),
    /// thinking blocks are emitted as-is.
    pub require_thinking_signature: bool,
}

impl MessageBuildContext {
    pub fn new() -> Self {
        Self {
            require_thinking_signature: false,
        }
    }

    pub fn with_require_thinking_signature(mut self, value: bool) -> Self {
        self.require_thinking_signature = value;
        self
    }
}

impl Default for MessageBuildContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Build Anthropic messages from ResponseItems directly, preserving tool_use/tool_result pairs.
/// Injects synthetic tool outputs for any orphaned calls to satisfy Anthropic's pairing requirement.
pub fn build_anthropic_messages_from_response_items(
    items: &[ResponseItem],
    ctx: &MessageBuildContext,
) -> Vec<Message> {
    let items = ensure_all_tool_outputs_present(items);
    let mut messages: Vec<Message> = Vec::new();

    for item in &items {
        match item {
            ResponseItem::Message { role, content, .. } => {
                let role = map_role(role);
                let blocks: Vec<MessageContent> =
                    content.iter().map(content_item_to_anthropic).collect();
                if blocks.is_empty() {
                    continue;
                }
                append_or_push(&mut messages, role, blocks);
            }
            ResponseItem::Reasoning {
                summary,
                content,
                encrypted_content,
                signature,
                ..
            } => {
                if let Some(data) = encrypted_content {
                    if signature.is_some() {
                        debug!(
                            "build_anthropic_messages: adding RedactedThinking block (data_len={}, sig={})",
                            data.len(),
                            signature
                                .as_deref()
                                .map(|s| &s[..s.len().min(20)])
                                .unwrap_or("none")
                        );
                        append_or_push(
                            &mut messages,
                            "assistant".into(),
                            vec![MessageContent::RedactedThinking {
                                data: data.clone(),
                                signature: signature.clone(),
                            }],
                        );
                    } else {
                        warn!(
                            "build_anthropic_messages: dropping RedactedThinking block — missing signature (data_len={})",
                            data.len()
                        );
                    }
                } else {
                    let text = extract_reasoning_text(summary, content);
                    if signature.is_some() {
                        // Signature present — emit as Thinking block.
                        debug!(
                            "build_anthropic_messages: adding Thinking block (text_len={}, has_sig={})",
                            text.len(),
                            signature.is_some()
                        );
                        append_or_push(
                            &mut messages,
                            "assistant".into(),
                            vec![MessageContent::Thinking {
                                thinking: text,
                                signature: signature.clone(),
                            }],
                        );
                    } else if !ctx.require_thinking_signature {
                        // No signature, but native Anthropic API doesn't require it.
                        append_or_push(
                            &mut messages,
                            "assistant".into(),
                            vec![MessageContent::Thinking {
                                thinking: text,
                                signature: None,
                            }],
                        );
                    } else if !text.is_empty() {
                        // No signature and proxy requires it — downgrade to Text.
                        warn!(
                            "build_anthropic_messages: reasoning without signature → Text (text_len={})",
                            text.len()
                        );
                        append_or_push(
                            &mut messages,
                            "assistant".into(),
                            vec![MessageContent::Text { text }],
                        );
                    }
                }
            }
            ResponseItem::FunctionCall {
                call_id,
                name,
                arguments,
                ..
            } => {
                let input: serde_json::Value =
                    serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
                append_or_push(
                    &mut messages,
                    "assistant".into(),
                    vec![MessageContent::ToolUse {
                        id: call_id.clone(),
                        name: name.clone(),
                        input,
                    }],
                );
            }
            ResponseItem::CustomToolCall {
                call_id,
                name,
                input,
                ..
            } => {
                let parsed: serde_json::Value =
                    serde_json::from_str(input).unwrap_or(serde_json::Value::Null);
                append_or_push(
                    &mut messages,
                    "assistant".into(),
                    vec![MessageContent::ToolUse {
                        id: call_id.clone(),
                        name: name.clone(),
                        input: parsed,
                    }],
                );
            }
            ResponseItem::FunctionCallOutput { call_id, output } => {
                append_or_push(
                    &mut messages,
                    "user".into(),
                    vec![MessageContent::ToolResult {
                        tool_use_id: call_id.clone(),
                        content: tool_result_content(output),
                        is_error: None,
                    }],
                );
            }
            ResponseItem::CustomToolCallOutput {
                call_id,
                name: _,
                output,
            } => {
                append_or_push(
                    &mut messages,
                    "user".into(),
                    vec![MessageContent::ToolResult {
                        tool_use_id: call_id.clone(),
                        content: tool_result_content(output),
                        is_error: None,
                    }],
                );
            }
            _ => {}
        }
    }

    messages
}

/// Ensure every tool call has a corresponding tool output immediately after it,
/// and remove orphan or duplicate outputs. This satisfies Anthropic's strict
/// requirement that each `tool_use` must be immediately followed by a
/// `tool_result` in the next message.
fn ensure_all_tool_outputs_present(items: &[ResponseItem]) -> Vec<ResponseItem> {
    let mut function_outputs_by_call_id: HashMap<&str, ResponseItem> = HashMap::new();
    let mut custom_outputs_by_call_id: HashMap<&str, ResponseItem> = HashMap::new();

    for item in items {
        match item {
            ResponseItem::FunctionCallOutput { call_id, .. } => {
                function_outputs_by_call_id
                    .entry(call_id.as_str())
                    .or_insert_with(|| item.clone());
            }
            ResponseItem::CustomToolCallOutput { call_id, .. } => {
                custom_outputs_by_call_id
                    .entry(call_id.as_str())
                    .or_insert_with(|| item.clone());
            }
            _ => {}
        }
    }

    let mut result = Vec::with_capacity(items.len());
    let mut index = 0;
    while index < items.len() {
        let item = &items[index];
        match item {
            // LocalShellCall items don't map to Anthropic tool_use blocks, and
            // their outputs are keyed by call_id; both are removed by only
            // materializing outputs for explicit FunctionCall items below.
            ResponseItem::LocalShellCall { .. } => {
                index += 1;
            }
            // Outputs are reinserted at the position of their matching call to
            // guarantee immediate pairing after tool_use.
            ResponseItem::FunctionCallOutput { .. } | ResponseItem::CustomToolCallOutput { .. } => {
                index += 1;
            }
            ResponseItem::FunctionCall { .. } | ResponseItem::CustomToolCall { .. } => {
                let mut pending_output_ids: Vec<(bool, String)> = Vec::new();
                while index < items.len() {
                    match &items[index] {
                        ResponseItem::FunctionCall { call_id, .. } => {
                            result.push(items[index].clone());
                            pending_output_ids.push((true, call_id.clone()));
                            index += 1;
                        }
                        ResponseItem::CustomToolCall { call_id, .. } => {
                            result.push(items[index].clone());
                            pending_output_ids.push((false, call_id.clone()));
                            index += 1;
                        }
                        _ => break,
                    }
                }
                for (is_function_call, call_id) in pending_output_ids {
                    let output = if is_function_call {
                        function_outputs_by_call_id
                            .remove(call_id.as_str())
                            .unwrap_or_else(|| ResponseItem::FunctionCallOutput {
                                call_id: call_id.clone(),
                                output: FunctionCallOutputPayload::from_text("aborted".to_string()),
                            })
                    } else {
                        custom_outputs_by_call_id
                            .remove(call_id.as_str())
                            .unwrap_or_else(|| ResponseItem::CustomToolCallOutput {
                                call_id: call_id.clone(),
                                name: None,
                                output: FunctionCallOutputPayload::from_text("aborted".to_string()),
                            })
                    };
                    result.push(output);
                }
            }
            _ => {
                result.push(item.clone());
                index += 1;
            }
        }
    }

    result
}

fn append_or_push(messages: &mut Vec<Message>, role: String, blocks: Vec<MessageContent>) {
    if let Some(last) = messages.last_mut()
        && last.role == role
    {
        last.content.extend(blocks);
    } else {
        messages.push(Message {
            role,
            content: blocks,
        });
    }
}

fn tool_result_content(output: &FunctionCallOutputPayload) -> ToolResultContent {
    match output.content_items() {
        Some([]) => ToolResultContent::Text(String::new()),
        Some(items) => ToolResultContent::Blocks(
            items
                .iter()
                .map(function_call_output_content_item_to_tool_result_block)
                .collect(),
        ),
        None => ToolResultContent::Text(output.text_content().unwrap_or("").to_string()),
    }
}

fn function_call_output_content_item_to_tool_result_block(
    item: &FunctionCallOutputContentItem,
) -> ToolResultBlock {
    match item {
        FunctionCallOutputContentItem::InputText { text } => {
            ToolResultBlock::Text { text: text.clone() }
        }
        FunctionCallOutputContentItem::InputImage { image_url, .. } => {
            let (media_type, data) = parse_data_url(image_url);
            ToolResultBlock::Image {
                source: ImageSource {
                    source_type: "base64".into(),
                    media_type: media_type.into(),
                    data: data.into(),
                },
            }
        }
    }
}

/// Convert a `ResponseItem` to a `ResponseInputItem` for simple cases.
/// Complex cases (FunctionCall, etc.) are handled directly by `build_anthropic_messages_from_response_items`.
pub fn response_item_to_input(item: &ResponseItem) -> Option<ResponseInputItem> {
    match item {
        ResponseItem::Message {
            role,
            content,
            phase,
            ..
        } => Some(ResponseInputItem::Message {
            role: role.clone(),
            content: content.to_vec(),
            phase: phase.clone(),
        }),
        ResponseItem::Reasoning {
            summary, content, ..
        } => {
            let text = extract_reasoning_text(summary, content);
            if !text.is_empty() {
                Some(ResponseInputItem::Message {
                    role: "assistant".into(),
                    content: vec![ContentItem::OutputText { text }],
                    phase: None,
                })
            } else {
                None
            }
        }
        ResponseItem::FunctionCall {
            call_id,
            name,
            arguments: _,
            ..
        } => {
            // Skip — handled by build_anthropic_messages_from_response_items
            let _ = (call_id, name);
            None
        }
        ResponseItem::FunctionCallOutput { call_id, output } => {
            Some(ResponseInputItem::FunctionCallOutput {
                call_id: call_id.clone(),
                output: output.clone(),
            })
        }
        ResponseItem::CustomToolCallOutput {
            call_id,
            name,
            output,
        } => Some(ResponseInputItem::CustomToolCallOutput {
            call_id: call_id.clone(),
            name: name.clone(),
            output: output.clone(),
        }),
        _ => None,
    }
}

fn extract_reasoning_text(
    summary: &[agere_protocol::models::ReasoningItemReasoningSummary],
    content: &Option<Vec<agere_protocol::models::ReasoningItemContent>>,
) -> String {
    let mut text = String::new();
    if let Some(content_items) = content {
        for ci in content_items {
            match ci {
                agere_protocol::models::ReasoningItemContent::ReasoningText { text: t }
                | agere_protocol::models::ReasoningItemContent::Text { text: t } => {
                    text.push_str(t);
                }
            }
        }
    }
    if text.is_empty() {
        for s in summary {
            match s {
                agere_protocol::models::ReasoningItemReasoningSummary::SummaryText { text: t } => {
                    text.push_str(t);
                }
            }
        }
    }
    text
}

fn convert_input_items(items: &[ResponseInputItem]) -> Vec<Message> {
    let mut messages: Vec<Message> = Vec::new();

    for item in items {
        match item {
            ResponseInputItem::Message { role, content, .. } => {
                let role = map_role(role);
                let blocks: Vec<MessageContent> =
                    content.iter().map(content_item_to_anthropic).collect();
                if blocks.is_empty() {
                    continue;
                }
                append_or_push(&mut messages, role, blocks);
            }
            ResponseInputItem::FunctionCallOutput { call_id, output } => {
                append_or_push(
                    &mut messages,
                    "user".into(),
                    vec![MessageContent::ToolResult {
                        tool_use_id: call_id.clone(),
                        content: tool_result_content(output),
                        is_error: None,
                    }],
                );
            }
            _ => {}
        }
    }

    messages
}

fn map_role(role: &str) -> String {
    match role {
        "assistant" => "assistant".into(),
        _ => "user".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ThinkingConfig;
    use agere_protocol::models::FunctionCallOutputContentItem;
    use agere_protocol::models::FunctionCallOutputPayload;

    fn user_text(text: &str) -> ResponseInputItem {
        ResponseInputItem::Message {
            role: "user".into(),
            content: vec![ContentItem::InputText { text: text.into() }],
            phase: None,
        }
    }

    fn assistant_text(text: &str) -> ResponseInputItem {
        ResponseInputItem::Message {
            role: "assistant".into(),
            content: vec![ContentItem::OutputText { text: text.into() }],
            phase: None,
        }
    }

    fn fn_call_output(call_id: &str, text: &str) -> ResponseInputItem {
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.into(),
            output: FunctionCallOutputPayload::from_text(text.into()),
        }
    }

    fn make_function_call(call_id: &str, name: &str, args: &str) -> ResponseItem {
        ResponseItem::FunctionCall {
            id: Some(call_id.into()),
            call_id: call_id.into(),
            name: name.into(),
            arguments: args.into(),
            namespace: None,
        }
    }

    fn make_function_call_output(call_id: &str, text: &str) -> ResponseItem {
        ResponseItem::FunctionCallOutput {
            call_id: call_id.into(),
            output: FunctionCallOutputPayload::from_text(text.into()),
        }
    }

    fn make_function_call_image_output(call_id: &str, image_url: &str) -> ResponseItem {
        ResponseItem::FunctionCallOutput {
            call_id: call_id.into(),
            output: FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::InputImage {
                    image_url: image_url.into(),
                    detail: None,
                },
            ]),
        }
    }

    fn make_function_call_empty_content_items_output(call_id: &str) -> ResponseItem {
        ResponseItem::FunctionCallOutput {
            call_id: call_id.into(),
            output: FunctionCallOutputPayload::from_content_items(Vec::new()),
        }
    }

    #[test]
    fn basic_user_message() {
        let items = vec![user_text("Hello")];
        let req = build_anthropic_request(
            "claude-sonnet-4-6",
            "Be helpful.",
            &items,
            &[],
            None,
            4096,
            None,
            None,
            None,
            None,
        );
        assert_eq!(req.model, "claude-sonnet-4-6");
        assert_eq!(req.system, Some(SystemPrompt::Text("Be helpful.".into())));
        assert!(req.stream);
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, "user");
        assert_eq!(
            req.messages[0].content[0],
            MessageContent::Text {
                text: "Hello".into()
            }
        );
    }

    #[test]
    fn consecutive_user_messages_merged() {
        let items = vec![user_text("Hello"), user_text("World")];
        let req = build_anthropic_request("m", "", &items, &[], None, 4096, None, None, None, None);
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, "user");
        assert_eq!(req.messages[0].content.len(), 2);
    }

    #[test]
    fn function_call_output_image_becomes_anthropic_tool_result_image_block() {
        let items = vec![
            make_function_call("call_1", "view_image", r#"{"path":"image.png"}"#),
            make_function_call_image_output("call_1", "data:image/png;base64,iVBORw0KGgo="),
        ];

        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, "user");
        let tool_result =
            serde_json::to_value(&messages[1].content[0]).expect("tool result serializes to JSON");
        assert_eq!(
            tool_result,
            serde_json::json!({
                "type": "tool_result",
                "tool_use_id": "call_1",
                "content": [{
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": "iVBORw0KGgo="
                    }
                }]
            })
        );
    }

    #[test]
    fn empty_content_item_tool_result_becomes_empty_text() {
        let items = vec![
            make_function_call("call_1", "empty_tool", "{}"),
            make_function_call_empty_content_items_output("call_1"),
        ];

        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, "user");
        match &messages[1].content[0] {
            MessageContent::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                assert_eq!(tool_use_id, "call_1");
                assert_eq!(content.as_str(), Some(""));
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }
    }

    #[test]
    fn tool_result_becomes_user_role() {
        let items = vec![
            assistant_text("Let me check"),
            fn_call_output("call_1", "Sunny"),
        ];
        let req = build_anthropic_request("m", "", &items, &[], None, 4096, None, None, None, None);
        assert_eq!(req.messages.len(), 2);
        assert_eq!(req.messages[0].role, "assistant");
        assert_eq!(req.messages[1].role, "user");
        match &req.messages[1].content[0] {
            MessageContent::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                assert_eq!(tool_use_id, "call_1");
                assert_eq!(content.as_str(), Some("Sunny"));
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }
    }

    #[test]
    fn function_call_converts_to_tool_use() {
        let items = vec![
            make_function_call("call_1", "get_weather", r#"{"city":"NYC"}"#),
            make_function_call_output("call_1", "Sunny"),
        ];
        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );
        assert_eq!(messages.len(), 2);
        // assistant with tool_use
        assert_eq!(messages[0].role, "assistant");
        match &messages[0].content[0] {
            MessageContent::ToolUse { id, name, input } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "get_weather");
                assert_eq!(input, &serde_json::json!({"city":"NYC"}));
            }
            other => panic!("expected ToolUse, got {other:?}"),
        }
        // user with tool_result
        assert_eq!(messages[1].role, "user");
        match &messages[1].content[0] {
            MessageContent::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                assert_eq!(tool_use_id, "call_1");
                assert_eq!(content.as_str(), Some("Sunny"));
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }
    }

    #[test]
    fn empty_system_is_omitted() {
        let items = vec![user_text("Hi")];
        let req = build_anthropic_request("m", "", &items, &[], None, 4096, None, None, None, None);
        assert_eq!(req.system, None);
    }

    #[test]
    fn thinking_injected_when_effort_is_set() {
        use crate::types::OutputConfig;
        let items = vec![user_text("Think")];
        let req = build_anthropic_request(
            "claude-opus-4-7",
            "",
            &items,
            &[],
            Some(ReasoningEffort::High),
            4096,
            None,
            None,
            None,
            None,
        );
        assert_eq!(
            req.thinking,
            Some(ThinkingConfig {
                thinking_type: "adaptive".into(),
                budget_tokens: None,
                display: None,
            })
        );
        assert_eq!(
            req.output_config,
            Some(OutputConfig {
                effort: Some("high".into()),
                format: None,
            })
        );
    }

    #[test]
    fn orphaned_function_call_gets_synthetic_output() {
        // Simulates a tool call without a matching output (e.g., interrupted turn).
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "What's the weather?".into(),
                }],
                phase: None,
            },
            make_function_call("call_orphan", "get_weather", r#"{"city":"NYC"}"#),
            // No FunctionCallOutput — synthetic one should be injected.
        ];
        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );
        // user message, assistant tool_use, synthetic tool_result
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[2].role, "user");
        match &messages[2].content[0] {
            MessageContent::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                assert_eq!(tool_use_id, "call_orphan");
                assert_eq!(content.as_str(), Some("aborted"));
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }
    }

    #[test]
    fn orphaned_function_call_output_is_removed() {
        // FunctionCallOutput without a matching FunctionCall should be dropped.
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "Hello".into(),
                }],
                phase: None,
            },
            ResponseItem::FunctionCallOutput {
                call_id: "call_orphan_output".into(),
                output: FunctionCallOutputPayload::from_text("ghost result".into()),
            },
        ];
        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );
        // Only the user message should remain; orphan output is dropped.
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
    }

    #[test]
    fn multi_turn_with_reasoning_and_tool_use() {
        // Simulates a multi-turn conversation with reasoning (thinking) blocks.
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "What's the weather?".into(),
                }],
                phase: None,
            },
            ResponseItem::Reasoning {
                id: "rs_1".into(),
                summary: vec![],
                content: Some(vec![
                    agere_protocol::models::ReasoningItemContent::ReasoningText {
                        text: "I should check the weather.".into(),
                    },
                ]),
                encrypted_content: None,
                signature: Some("sig_abc".into()),
            },
            make_function_call("call_1", "get_weather", r#"{"city":"NYC"}"#),
            make_function_call_output("call_1", "Sunny, 72°F"),
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "What about tomorrow?".into(),
                }],
                phase: None,
            },
            ResponseItem::Reasoning {
                id: "rs_2".into(),
                summary: vec![],
                content: Some(vec![
                    agere_protocol::models::ReasoningItemContent::ReasoningText {
                        text: "Let me check tomorrow's forecast.".into(),
                    },
                ]),
                encrypted_content: None,
                signature: Some("sig_def".into()),
            },
            make_function_call(
                "call_2",
                "get_weather",
                r#"{"city":"NYC","day":"tomorrow"}"#,
            ),
            make_function_call_output("call_2", "Cloudy, 65°F"),
        ];
        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );

        // Verify message count and roles
        // user, assistant(thinking+tool_use), user(tool_result+text), assistant(thinking+tool_use), user(tool_result)
        assert_eq!(messages.len(), 5);

        // Message 0: user with initial question
        assert_eq!(messages[0].role, "user");

        // Message 1: assistant with thinking + tool_use (merged)
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content.len(), 2);
        assert!(
            matches!(&messages[1].content[0], MessageContent::Thinking { thinking, signature } if thinking.contains("check the weather") && signature.as_ref().is_some())
        );
        assert!(
            matches!(&messages[1].content[1], MessageContent::ToolUse { id, name, .. } if id == "call_1" && name == "get_weather")
        );

        // Message 2: user with tool_result + follow-up question (merged)
        assert_eq!(messages[2].role, "user");
        assert_eq!(messages[2].content.len(), 2);
        assert!(
            matches!(&messages[2].content[0], MessageContent::ToolResult { content, .. } if content.as_str() == Some("Sunny, 72°F"))
        );
        assert!(
            matches!(&messages[2].content[1], MessageContent::Text { text } if text == "What about tomorrow?")
        );

        // Message 3: assistant with thinking + tool_use (merged)
        assert_eq!(messages[3].role, "assistant");
        assert_eq!(messages[3].content.len(), 2);
        assert!(
            matches!(&messages[3].content[0], MessageContent::Thinking { thinking, .. } if thinking.contains("tomorrow"))
        );
        assert!(
            matches!(&messages[3].content[1], MessageContent::ToolUse { id, .. } if id == "call_2")
        );

        // Message 4: user with tool_result
        assert_eq!(messages[4].role, "user");
        assert!(
            matches!(&messages[4].content[0], MessageContent::ToolResult { content, .. } if content.as_str() == Some("Cloudy, 65°F"))
        );
    }

    #[test]
    fn parallel_tool_calls_with_partial_outputs() {
        // Two function calls, only one has output.
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "Compare NYC and LA weather".into(),
                }],
                phase: None,
            },
            make_function_call("call_nyc", "get_weather", r#"{"city":"NYC"}"#),
            make_function_call("call_la", "get_weather", r#"{"city":"LA"}"#),
            // Only call_nyc has an output; call_la should get synthetic "aborted"
            make_function_call_output("call_nyc", "Sunny, 72°F"),
        ];
        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );

        // user, assistant(tool_use+tool_use), user(tool_result+tool_result)
        assert_eq!(messages.len(), 3);

        // Assistant message has both tool_uses
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content.len(), 2);

        // User message has both tool_results (real + synthetic)
        assert_eq!(messages[2].role, "user");
        assert_eq!(messages[2].content.len(), 2);
        let mut results: Vec<&str> = messages[2]
            .content
            .iter()
            .filter_map(|c| match c {
                MessageContent::ToolResult { content, .. } => content.as_str(),
                _ => None,
            })
            .collect();
        results.sort();
        assert_eq!(results, vec!["Sunny, 72°F", "aborted"]);
    }

    #[test]
    fn delayed_function_call_output_is_repositioned_after_call() {
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "Run shell command".into(),
                }],
                phase: None,
            },
            make_function_call("call_1", "shell_command", r#"{"command":"echo hi"}"#),
            ResponseItem::Message {
                id: None,
                role: "assistant".into(),
                content: vec![ContentItem::OutputText {
                    text: "Working on it...".into(),
                }],
                phase: None,
            },
            make_function_call_output("call_1", "hi"),
        ];

        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert!(
            matches!(&messages[1].content[0], MessageContent::ToolUse { id, .. } if id == "call_1")
        );
        assert_eq!(messages[2].role, "user");
        assert!(
            matches!(&messages[2].content[0], MessageContent::ToolResult { tool_use_id, content, .. } if tool_use_id == "call_1" && content.as_str() == Some("hi"))
        );
        assert_eq!(messages[3].role, "assistant");
        assert!(
            matches!(&messages[3].content[0], MessageContent::Text { text } if text == "Working on it...")
        );
    }

    #[test]
    fn orphaned_custom_tool_call_gets_synthetic_output() {
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "Run custom tool".into(),
                }],
                phase: None,
            },
            ResponseItem::CustomToolCall {
                id: None,
                status: None,
                call_id: "custom_call_1".into(),
                name: "custom_tool".into(),
                input: "{}".into(),
            },
        ];
        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[2].role, "user");
        match &messages[2].content[0] {
            MessageContent::ToolResult {
                tool_use_id,
                content,
                ..
            } => {
                assert_eq!(tool_use_id, "custom_call_1");
                assert_eq!(content.as_str(), Some("aborted"));
            }
            other => panic!("expected ToolResult, got {other:?}"),
        }
    }

    #[test]
    fn message_role_alternation_is_maintained() {
        // Complex scenario: verify that after all merging, roles strictly alternate.
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText { text: "A".into() }],
                phase: None,
            },
            ResponseItem::Reasoning {
                id: "r1".into(),
                summary: vec![],
                content: Some(vec![
                    agere_protocol::models::ReasoningItemContent::ReasoningText {
                        text: "think".into(),
                    },
                ]),
                encrypted_content: None,
                signature: None,
            },
            make_function_call("c1", "t1", "{}"),
            make_function_call_output("c1", "out1"),
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText { text: "B".into() }],
                phase: None,
            },
            ResponseItem::Reasoning {
                id: "r2".into(),
                summary: vec![],
                content: Some(vec![
                    agere_protocol::models::ReasoningItemContent::ReasoningText {
                        text: "think2".into(),
                    },
                ]),
                encrypted_content: None,
                signature: None,
            },
            make_function_call("c2", "t2", "{}"),
            // c2 orphaned
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText { text: "C".into() }],
                phase: None,
            },
        ];
        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );

        // Verify role alternation
        for i in 1..messages.len() {
            assert_ne!(
                messages[i].role,
                messages[i - 1].role,
                "Role alternation broken at index {}: both are '{}'",
                i,
                messages[i].role
            );
        }

        // Verify every tool_use has a following tool_result
        for (i, msg) in messages.iter().enumerate() {
            if msg
                .content
                .iter()
                .any(|c| matches!(c, MessageContent::ToolUse { .. }))
            {
                assert!(
                    i + 1 < messages.len(),
                    "tool_use at end with no tool_result"
                );
                assert_eq!(messages[i + 1].role, "user");
                assert!(
                    messages[i + 1]
                        .content
                        .iter()
                        .any(|c| matches!(c, MessageContent::ToolResult { .. }))
                );
            }
        }
    }

    #[test]
    fn reasoning_without_signature_emits_text() {
        // Without a signature, reasoning must be downgraded to a Text block.
        // Thinking blocks without signatures are invalid for multi-turn
        // round-trips — OpenRouter rejects them.
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "Hello".into(),
                }],
                phase: None,
            },
            ResponseItem::Reasoning {
                id: "rs_1".into(),
                summary: vec![],
                content: Some(vec![
                    agere_protocol::models::ReasoningItemContent::ReasoningText {
                        text: "Let me think...".into(),
                    },
                ]),
                encrypted_content: None,
                signature: None,
            },
        ];
        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content.len(), 1);
        assert!(
            matches!(&messages[1].content[0], MessageContent::Text { text } if text == "Let me think..."),
            "expected Text block (no signature), got {:?}",
            &messages[1].content[0]
        );
    }

    #[test]
    fn reasoning_without_signature_keeps_thinking_when_native() {
        // When require_thinking_signature=false (native Anthropic API),
        // thinking blocks without signature are emitted as-is.
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "Hello".into(),
                }],
                phase: None,
            },
            ResponseItem::Reasoning {
                id: "rs_1".into(),
                summary: vec![],
                content: Some(vec![
                    agere_protocol::models::ReasoningItemContent::ReasoningText {
                        text: "Let me think...".into(),
                    },
                ]),
                encrypted_content: None,
                signature: None,
            },
        ];
        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(false),
        );

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content.len(), 1);
        assert!(
            matches!(&messages[1].content[0], MessageContent::Thinking { thinking, signature } if thinking == "Let me think..." && signature.is_none()),
            "expected Thinking block without signature, got {:?}",
            &messages[1].content[0]
        );
    }

    #[test]
    fn reasoning_with_signature_keeps_thinking_block() {
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "Hello".into(),
                }],
                phase: None,
            },
            ResponseItem::Reasoning {
                id: "rs_1".into(),
                summary: vec![],
                content: Some(vec![
                    agere_protocol::models::ReasoningItemContent::ReasoningText {
                        text: "Let me think...".into(),
                    },
                ]),
                encrypted_content: None,
                signature: Some("sig_valid".into()),
            },
        ];
        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );

        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content.len(), 1);
        // With a valid signature, the reasoning must be emitted as a Thinking block.
        assert!(
            matches!(&messages[1].content[0], MessageContent::Thinking { thinking, signature } if thinking == "Let me think..." && *signature == Some("sig_valid".into())),
            "expected Thinking block with signature, got {:?}",
            &messages[1].content[0]
        );
    }

    #[test]
    fn redacted_reasoning_produces_redacted_thinking_block() {
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "Hello".into(),
                }],
                phase: None,
            },
            ResponseItem::Reasoning {
                id: "rs_redacted".into(),
                summary: vec![],
                content: None,
                encrypted_content: Some("ZW5jcnlwdGVk".into()),
                signature: Some("sig_xyz".into()),
            },
        ];
        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );

        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content.len(), 1);
        assert!(
            matches!(&messages[1].content[0], MessageContent::RedactedThinking { data, signature } if data == "ZW5jcnlwdGVk" && *signature == Some("sig_xyz".into())),
            "expected RedactedThinking block, got {:?}",
            &messages[1].content[0]
        );
    }

    #[test]
    fn redacted_reasoning_without_signature_is_skipped() {
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "Hello".into(),
                }],
                phase: None,
            },
            ResponseItem::Reasoning {
                id: "rs_bad".into(),
                summary: vec![],
                content: None,
                encrypted_content: Some("ZW5jcnlwdGVk".into()),
                signature: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "Follow-up".into(),
                }],
                phase: None,
            },
        ];
        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );

        // The bad redacted reasoning should be dropped, leaving 2 user messages merged.
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content.len(), 2);
    }

    #[test]
    fn empty_thinking_with_signature_is_emitted() {
        // DeepSeek returns thinking blocks with empty text and a UUID signature.
        // These must NOT be dropped — the signature is needed on follow-up requests.
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "Hello".into(),
                }],
                phase: None,
            },
            ResponseItem::Reasoning {
                id: "rs_ds".into(),
                summary: vec![],
                content: None,
                encrypted_content: None,
                signature: Some("a4c3cac5-e983-4bc7-9729-71ad4d7b0a8a".into()),
            },
        ];
        let messages = build_anthropic_messages_from_response_items(
            &items,
            &MessageBuildContext::new().with_require_thinking_signature(true),
        );

        // Empty thinking with signature must be preserved.
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content.len(), 1);
        assert!(
            matches!(&messages[1].content[0], MessageContent::Thinking { thinking, signature } if thinking.is_empty() && *signature == Some("a4c3cac5-e983-4bc7-9729-71ad4d7b0a8a".into())),
            "expected empty Thinking block with signature, got {:?}",
            &messages[1].content[0]
        );
    }
}
