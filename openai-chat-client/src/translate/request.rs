use crate::translate::content::content_item_to_chat;
use crate::types::ChatContent;
use crate::types::ChatContentBlock;
use crate::types::ChatMessage;
use crate::types::ChatToolCall;
use agere_protocol::models::ResponseItem;
use tracing::debug;

/// Build Chat Completions messages from ResponseItem slice.
///
/// Following CLIProxyAPI's pattern: each ResponseItem produces exactly one ChatMessage.
/// No merging of same-role messages — the OpenAI Chat Completions API supports
/// consecutive messages of the same role.
pub(crate) fn build_chat_messages_from_response_items(items: &[ResponseItem]) -> Vec<ChatMessage> {
    let mut messages: Vec<ChatMessage> = Vec::new();

    for item in items {
        match item {
            ResponseItem::Message { role, content, .. } => {
                let role_str = map_role(role);

                // Skip messages with empty content — they produce invalid Chat Completions requests
                if content.is_empty() {
                    debug!(
                        "Skipping ResponseItem::Message role={} with empty content",
                        role_str
                    );
                    continue;
                }

                // Convert content items to ChatContent
                let mut all_blocks: Vec<ChatContentBlock> = Vec::new();
                for ci in content {
                    match content_item_to_chat(ci) {
                        ChatContent::Text(text) => {
                            all_blocks.push(ChatContentBlock {
                                block_type: "text".into(),
                                text: Some(text),
                                image_url: None,
                            });
                        }
                        ChatContent::Blocks(blocks) => {
                            all_blocks.extend(blocks);
                        }
                    }
                }

                let message_content = if all_blocks.is_empty() {
                    // All content items were empty — skip this message
                    debug!(
                        "Skipping ResponseItem::Message role={} after conversion (all content empty)",
                        role_str
                    );
                    continue;
                } else if all_blocks.len() == 1 {
                    let block = all_blocks.remove(0);
                    if block.block_type == "text" {
                        ChatContent::Text(block.text.unwrap_or_default())
                    } else {
                        ChatContent::Blocks(vec![block])
                    }
                } else {
                    ChatContent::Blocks(all_blocks)
                };

                messages.push(ChatMessage {
                    role: role_str,
                    content: Some(message_content),
                    tool_calls: None,
                    tool_call_id: None,
                    reasoning: None,
                });
            }
            ResponseItem::FunctionCall {
                call_id,
                name,
                arguments,
                ..
            } => {
                debug!(
                    "ResponseItem::FunctionCall call_id={} name={}",
                    call_id, name
                );
                // FunctionCall → assistant message with tool_calls
                messages.push(ChatMessage {
                    role: "assistant".into(),
                    content: None,
                    tool_calls: Some(vec![ChatToolCall {
                        id: call_id.clone(),
                        call_type: "function".into(),
                        function: crate::types::ChatFunctionCall {
                            name: name.clone(),
                            arguments: arguments.clone(),
                        },
                    }]),
                    tool_call_id: None,
                    reasoning: None,
                });
            }
            ResponseItem::CustomToolCall {
                call_id,
                name,
                input,
                ..
            } => {
                debug!(
                    "ResponseItem::CustomToolCall call_id={} name={}",
                    call_id, name
                );
                messages.push(ChatMessage {
                    role: "assistant".into(),
                    content: None,
                    tool_calls: Some(vec![ChatToolCall {
                        id: call_id.clone(),
                        call_type: "function".into(),
                        function: crate::types::ChatFunctionCall {
                            name: name.clone(),
                            arguments: input.clone(),
                        },
                    }]),
                    tool_call_id: None,
                    reasoning: None,
                });
            }
            ResponseItem::ToolSearchCall {
                call_id, arguments, ..
            } => {
                let call_id_str = call_id.as_deref().unwrap_or("unknown");
                debug!("ResponseItem::ToolSearchCall call_id={}", call_id_str);
                messages.push(ChatMessage {
                    role: "assistant".into(),
                    content: None,
                    tool_calls: Some(vec![ChatToolCall {
                        id: call_id_str.into(),
                        call_type: "function".into(),
                        function: crate::types::ChatFunctionCall {
                            name: "web_search".into(),
                            arguments: serde_json::to_string(arguments).unwrap_or_default(),
                        },
                    }]),
                    tool_call_id: None,
                    reasoning: None,
                });
            }
            ResponseItem::FunctionCallOutput { call_id, output } => {
                let text = output.text_content().unwrap_or("").to_string();
                messages.push(ChatMessage {
                    role: "tool".into(),
                    content: Some(ChatContent::Text(text)),
                    tool_calls: None,
                    tool_call_id: Some(call_id.clone()),
                    reasoning: None,
                });
            }
            ResponseItem::CustomToolCallOutput {
                call_id, output, ..
            } => {
                let text = output.text_content().unwrap_or("").to_string();
                messages.push(ChatMessage {
                    role: "tool".into(),
                    content: Some(ChatContent::Text(text)),
                    tool_calls: None,
                    tool_call_id: Some(call_id.clone()),
                    reasoning: None,
                });
            }
            ResponseItem::ToolSearchOutput { .. } => {
                // Skip — not mappable to Chat Completions message format
            }
            ResponseItem::Reasoning {
                summary, content, ..
            } => {
                // Convert reasoning to assistant text message
                let text = extract_reasoning_text(summary, content);
                if !text.is_empty() {
                    debug!("ResponseItem::Reasoning text_len={}", text.len());
                    messages.push(ChatMessage {
                        role: "assistant".into(),
                        content: Some(ChatContent::Text(text)),
                        tool_calls: None,
                        tool_call_id: None,
                        reasoning: None,
                    });
                }
            }
            // LocalShellCall, WebSearchCall, ImageGenerationCall, Compaction, Other
            // are not mappable to Chat Completions format
            _ => {}
        }
    }

    messages
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

/// Build Chat Completions messages with an optional system prompt.
///
/// System prompt is prepended as the first message. No merging is performed —
/// each item produces its own message, matching CLIProxyAPI's approach.
pub(crate) fn build_chat_messages_with_system(
    system: &str,
    items: &[ResponseItem],
) -> Vec<ChatMessage> {
    let mut messages = Vec::new();
    if !system.is_empty() {
        messages.push(ChatMessage {
            role: "system".into(),
            content: Some(ChatContent::Text(system.to_string())),
            tool_calls: None,
            tool_call_id: None,
            reasoning: None,
        });
    }
    messages.extend(build_chat_messages_from_response_items(items));

    // Defensive: ensure no non-system message reaches the API with null content.
    // Upstream providers (e.g. DeepSeek) reject `content: null` on user/assistant messages.
    for msg in &mut messages {
        if msg.role != "system" && msg.content.is_none() {
            msg.content = Some(ChatContent::Text(String::new()));
        }
    }

    messages
}

/// Map ResponseItem roles to OpenAI Chat Completions roles.
///
/// OpenAI Chat Completions standard roles: system, user, assistant, tool.
/// The `developer` role (Responses API) maps to `user` for Chat Completions,
/// since most providers (including DeepSeek) don't accept `developer` and
/// don't allow system messages in the middle of conversation.
/// This matches CLIProxyAPI's mapping in openai_openai-responses_request.go.
fn map_role(role: &str) -> String {
    match role {
        "user" | "developer" => "user".into(),
        "assistant" => "assistant".into(),
        "system" => "system".into(),
        _ => "user".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agere_protocol::models::ContentItem;
    use agere_protocol::models::FunctionCallOutputPayload;

    fn user_text(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "user".into(),
            content: vec![ContentItem::InputText { text: text.into() }],
            phase: None,
        }
    }

    fn assistant_text(text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: "assistant".into(),
            content: vec![ContentItem::OutputText { text: text.into() }],
            phase: None,
        }
    }

    fn fn_call_output(call_id: &str, text: &str) -> ResponseItem {
        ResponseItem::FunctionCallOutput {
            call_id: call_id.into(),
            output: FunctionCallOutputPayload::from_text(text.into()),
        }
    }

    #[test]
    fn basic_user_message() {
        let items = vec![user_text("Hello")];
        let messages = build_chat_messages_from_response_items(&items);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert!(
            matches!(&messages[0].content, Some(ChatContent::Text(t)) if t == "Hello"),
            "expected text content, got {:?}",
            messages[0].content
        );
    }

    #[test]
    fn consecutive_user_messages_not_merged() {
        // Following CLIProxyAPI: consecutive messages are NOT merged
        let items = vec![user_text("Hello"), user_text("World")];
        let messages = build_chat_messages_from_response_items(&items);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "user");
    }

    #[test]
    fn tool_output_becomes_tool_role() {
        let items = vec![
            assistant_text("Let me check"),
            fn_call_output("call_1", "Sunny"),
        ];
        let messages = build_chat_messages_from_response_items(&items);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "assistant");
        assert_eq!(messages[1].role, "tool");
        assert_eq!(messages[1].tool_call_id, Some("call_1".into()));
    }

    #[test]
    fn assistant_text_becomes_message() {
        let items = vec![assistant_text("The weather is sunny")];
        let messages = build_chat_messages_from_response_items(&items);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "assistant");
    }

    #[test]
    fn system_prompt_injected_as_first_message() {
        let items = vec![user_text("Hello")];
        let messages = build_chat_messages_with_system("You are helpful.", &items);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "system");
        assert!(
            matches!(&messages[0].content, Some(ChatContent::Text(t)) if t == "You are helpful.")
        );
    }

    #[test]
    fn empty_system_not_injected() {
        let items = vec![user_text("Hello")];
        let messages = build_chat_messages_with_system("", &items);
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn function_call_converted_to_tool_calls() {
        let items = vec![
            ResponseItem::FunctionCall {
                id: None,
                call_id: "call_abc".into(),
                name: "shell".into(),
                namespace: None,
                arguments: r#"{"command":["ls"]}"#.into(),
            },
            fn_call_output("call_abc", "file.txt"),
        ];
        let messages = build_chat_messages_from_response_items(&items);
        assert_eq!(messages.len(), 2);
        // Assistant message with tool_calls
        assert_eq!(messages[0].role, "assistant");
        assert!(messages[0].tool_calls.is_some());
        let tc = &messages[0].tool_calls.as_ref().unwrap()[0];
        assert_eq!(tc.id, "call_abc");
        assert_eq!(tc.function.name, "shell");
        // Tool output
        assert_eq!(messages[1].role, "tool");
        assert_eq!(messages[1].tool_call_id, Some("call_abc".into()));
    }

    #[test]
    fn empty_content_message_skipped() {
        let items = vec![ResponseItem::Message {
            id: None,
            role: "assistant".into(),
            content: vec![],
            phase: None,
        }];
        let messages = build_chat_messages_from_response_items(&items);
        assert!(messages.is_empty());
    }

    #[test]
    fn function_call_without_text_creates_assistant_message() {
        let items = vec![ResponseItem::FunctionCall {
            id: None,
            call_id: "call_1".into(),
            name: "read_file".into(),
            namespace: None,
            arguments: r#"{"path":"test.rs"}"#.into(),
        }];
        let messages = build_chat_messages_from_response_items(&items);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "assistant");
        // tool_calls messages can have null content
        assert!(messages[0].content.is_none());
        assert!(messages[0].tool_calls.is_some());
    }

    #[test]
    fn developer_role_mapped_to_user() {
        // developer role maps to user (matching CLIProxyAPI)
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "developer".into(),
                content: vec![ContentItem::InputText {
                    text: "dev context".into(),
                }],
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "user".into(),
                content: vec![ContentItem::InputText {
                    text: "user question".into(),
                }],
                phase: None,
            },
        ];
        let messages = build_chat_messages_from_response_items(&items);
        // Both are user messages, NOT merged (CLIProxyAPI pattern)
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "user");
    }

    #[test]
    fn multi_turn_conversation_preserved() {
        // Simulate: user Q1, assistant A1, user Q2, assistant A2, user Q3
        let items = vec![
            user_text("1+1=?"),
            assistant_text("2"),
            user_text("2+2=?"),
            assistant_text("4"),
            user_text("3+3=?"),
        ];
        let messages = build_chat_messages_with_system("You are helpful.", &items);
        // system + 5 items = 6 messages
        assert_eq!(messages.len(), 6);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[2].role, "assistant");
        assert_eq!(messages[3].role, "user");
        assert_eq!(messages[4].role, "assistant");
        assert_eq!(messages[5].role, "user");
        assert_eq!(messages[5].tool_call_id, None);
    }

    #[test]
    fn developer_context_update_with_user_question() {
        // Simulate: developer context update, then user question
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "developer".into(),
                content: vec![ContentItem::InputText {
                    text: "cwd=/tmp shell=zsh".into(),
                }],
                phase: None,
            },
            user_text("3+3=?"),
        ];
        let messages = build_chat_messages_with_system("You are a coding agent.", &items);
        // system + developer + user = 3 messages
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[1].role, "user"); // developer → user
        assert_eq!(messages[2].role, "user");
        // The user question must have content
        match &messages[2].content {
            Some(ChatContent::Text(t)) => assert!(t.contains("3+3"), "got: {t}"),
            other => panic!("expected Text content, got {other:?}"),
        }
    }
}
