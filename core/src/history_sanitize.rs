use std::borrow::Cow;

use agere_model_provider_info::WireApi;
use agere_protocol::models::ContentItem;
use agere_protocol::models::ReasoningItemContent;
use agere_protocol::models::ReasoningItemReasoningSummary;
use agere_protocol::models::ResponseItem;

/// 返回对目标协议安全的历史副本。仅在跨协议会导致畸形或语义错配时调整 `Reasoning` 项；
/// 其余项原样保留。各协议 translator 仍负责最终映射，本函数提供集中、可测的前置规整。
///
/// 快路径：若当前 `wire_api` 下没有任何项需要改动，则借用原切片（`Cow::Borrowed`），
/// 避免在常见请求路径上做无谓的整段克隆；仅在确有项被修改时才拥有新 `Vec`。
pub(crate) fn sanitize_history_for_wire_api<'a>(
    wire_api: WireApi,
    items: &'a [ResponseItem],
) -> Cow<'a, [ResponseItem]> {
    sanitize_history_for_wire_api_in_context(
        wire_api,
        items,
        HistorySanitizeContext::CurrentProvider,
    )
}

pub(crate) fn sanitize_history_for_provider_switch<'a>(
    wire_api: WireApi,
    items: &'a [ResponseItem],
) -> Cow<'a, [ResponseItem]> {
    sanitize_history_for_wire_api_in_context(
        wire_api,
        items,
        HistorySanitizeContext::ProviderSwitch,
    )
}

enum HistorySanitizeContext {
    CurrentProvider,
    ProviderSwitch,
}

fn sanitize_history_for_wire_api_in_context<'a>(
    wire_api: WireApi,
    items: &'a [ResponseItem],
    context: HistorySanitizeContext,
) -> Cow<'a, [ResponseItem]> {
    // Fast path: if nothing needs changing for this wire_api, borrow the input unchanged.
    let needs_change = items.iter().any(|item| match item {
        ResponseItem::Reasoning {
            encrypted_content,
            signature,
            ..
        } => match wire_api {
            WireApi::Chat => encrypted_content.is_some() || signature.is_some(),
            WireApi::Responses => signature.is_some(),
            WireApi::Anthropic => match context {
                HistorySanitizeContext::CurrentProvider => false,
                HistorySanitizeContext::ProviderSwitch => signature.is_none(),
            },
        },
        _ => false,
    });
    if !needs_change {
        return Cow::Borrowed(items);
    }

    let mut dropped_encrypted = 0usize;
    let out: Vec<ResponseItem> = items
        .iter()
        .map(|item| match item {
            ResponseItem::Reasoning {
                id,
                summary,
                content,
                encrypted_content,
                signature,
            } => match wire_api {
                WireApi::Anthropic
                    if matches!(context, HistorySanitizeContext::ProviderSwitch)
                        && signature.is_none() =>
                {
                    ResponseItem::Message {
                        id: None,
                        role: "assistant".to_string(),
                        content: vec![ContentItem::InputText {
                            text: reasoning_text(summary, content),
                        }],
                        phase: None,
                    }
                }
                _ => {
                    let (encrypted_content, signature) = match wire_api {
                        WireApi::Chat => {
                            if encrypted_content.is_some() {
                                dropped_encrypted += 1;
                            }
                            (None, None)
                        }
                        WireApi::Responses => {
                            if signature.is_some() {
                                if encrypted_content.is_some() {
                                    dropped_encrypted += 1;
                                }
                                (None, None)
                            } else {
                                (encrypted_content.clone(), None)
                            }
                        }
                        WireApi::Anthropic => (encrypted_content.clone(), signature.clone()),
                    };
                    ResponseItem::Reasoning {
                        id: id.clone(),
                        summary: summary.clone(),
                        content: content.clone(),
                        encrypted_content,
                        signature,
                    }
                }
            },
            other => other.clone(),
        })
        .collect();
    if dropped_encrypted > 0 {
        tracing::warn!(
            dropped = dropped_encrypted,
            wire_api = %wire_api,
            "sanitize_history: dropped provider-specific encrypted reasoning"
        );
    }
    Cow::Owned(out)
}

fn reasoning_text(
    summary: &[ReasoningItemReasoningSummary],
    content: &Option<Vec<ReasoningItemContent>>,
) -> String {
    if let Some(content) = content {
        content
            .iter()
            .map(|item| match item {
                ReasoningItemContent::ReasoningText { text }
                | ReasoningItemContent::Text { text } => text.as_str(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        summary
            .iter()
            .map(|summary| match summary {
                ReasoningItemReasoningSummary::SummaryText { text } => text.as_str(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_history_for_provider_switch;
    use super::sanitize_history_for_wire_api;
    use agere_model_provider_info::WireApi;
    use agere_protocol::models::ContentItem;
    use agere_protocol::models::ReasoningItemReasoningSummary;
    use agere_protocol::models::ResponseItem;

    fn reasoning_with(enc: Option<&str>, sig: Option<&str>) -> ResponseItem {
        ResponseItem::Reasoning {
            id: String::new(),
            summary: vec![ReasoningItemReasoningSummary::SummaryText { text: "s".into() }],
            content: None,
            encrypted_content: enc.map(str::to_string),
            signature: sig.map(str::to_string),
        }
    }

    #[test]
    fn chat_strips_encrypted_and_signature() {
        let items = vec![reasoning_with(Some("E"), Some("S"))];
        let out = sanitize_history_for_wire_api(WireApi::Chat, &items);
        match &out[0] {
            ResponseItem::Reasoning {
                encrypted_content,
                signature,
                summary,
                ..
            } => {
                assert!(encrypted_content.is_none());
                assert!(signature.is_none());
                assert_eq!(summary.len(), 1);
            }
            _ => panic!("expected reasoning"),
        }
    }

    #[test]
    fn responses_drops_anthropic_signed_encrypted_content() {
        let items = vec![reasoning_with(Some("E"), Some("S"))];
        let out = sanitize_history_for_wire_api(WireApi::Responses, &items);
        match &out[0] {
            ResponseItem::Reasoning {
                encrypted_content,
                signature,
                ..
            } => {
                assert!(encrypted_content.is_none());
                assert!(signature.is_none());
            }
            _ => panic!("expected reasoning"),
        }
    }

    #[test]
    fn responses_keeps_unsigned_encrypted_content() {
        let items = vec![reasoning_with(Some("E"), None)];
        let out = sanitize_history_for_wire_api(WireApi::Responses, &items);
        match &out[0] {
            ResponseItem::Reasoning {
                encrypted_content,
                signature,
                ..
            } => {
                assert_eq!(encrypted_content.as_deref(), Some("E"));
                assert!(signature.is_none());
            }
            _ => panic!("expected reasoning"),
        }
    }

    #[test]
    fn anthropic_preserves_pair() {
        let items = vec![reasoning_with(Some("E"), Some("S"))];
        let out = sanitize_history_for_wire_api(WireApi::Anthropic, &items);
        match &out[0] {
            ResponseItem::Reasoning {
                encrypted_content,
                signature,
                ..
            } => {
                assert_eq!(encrypted_content.as_deref(), Some("E"));
                assert_eq!(signature.as_deref(), Some("S"));
            }
            _ => panic!("expected reasoning"),
        }
    }

    #[test]
    fn anthropic_downgrades_unsigned_reasoning_to_message() {
        let items = vec![reasoning_with(None, None)];

        let out = sanitize_history_for_provider_switch(WireApi::Anthropic, &items);

        assert_eq!(
            out.as_ref(),
            &[ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::InputText {
                    text: "s".to_string(),
                }],
                phase: None,
            }]
        );
    }

    #[test]
    fn anthropic_current_provider_preserves_unsigned_reasoning() {
        let items = vec![reasoning_with(None, None)];

        let out = sanitize_history_for_wire_api(WireApi::Anthropic, &items);

        assert!(matches!(out, std::borrow::Cow::Borrowed(_)));
        assert_eq!(out.as_ref(), items.as_slice());
    }

    #[test]
    fn fast_path_returns_borrowed_when_no_change_needed() {
        // Anthropic signed reasoning is native thinking history and must borrow.
        let items = vec![reasoning_with(Some("E"), Some("S"))];
        assert!(matches!(
            sanitize_history_for_wire_api(WireApi::Anthropic, &items),
            std::borrow::Cow::Borrowed(_)
        ));

        // Responses only drops signatures; with no signature there's nothing to do: borrow.
        let items = vec![reasoning_with(Some("E"), None)];
        assert!(matches!(
            sanitize_history_for_wire_api(WireApi::Responses, &items),
            std::borrow::Cow::Borrowed(_)
        ));

        // Chat with a clean reasoning item (no enc, no sig) borrows too.
        let items = vec![reasoning_with(None, None)];
        assert!(matches!(
            sanitize_history_for_wire_api(WireApi::Chat, &items),
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn non_reasoning_items_untouched() {
        let items = vec![ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "hello".to_string(),
            }],
            phase: None,
        }];
        let out = sanitize_history_for_wire_api(WireApi::Chat, &items);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], ResponseItem::Message { .. }));
    }

    fn message(role: &str, text: &str) -> ResponseItem {
        ResponseItem::Message {
            id: None,
            role: role.to_string(),
            content: vec![ContentItem::InputText {
                text: text.to_string(),
            }],
            phase: None,
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        for wire_api in [WireApi::Chat, WireApi::Responses, WireApi::Anthropic] {
            let out = sanitize_history_for_wire_api(wire_api, &[]);
            assert!(out.is_empty());
        }
    }

    #[test]
    fn chat_drops_encrypted_without_signature() {
        // enc=Some, sig=None — encrypted must still be dropped under Chat.
        let items = vec![reasoning_with(Some("E"), None)];
        let out = sanitize_history_for_wire_api(WireApi::Chat, &items);
        match &out[0] {
            ResponseItem::Reasoning {
                encrypted_content,
                signature,
                ..
            } => {
                assert!(encrypted_content.is_none());
                assert!(signature.is_none());
            }
            _ => panic!("expected reasoning"),
        }
    }

    #[test]
    fn chat_reasoning_without_encrypted_is_unchanged() {
        // enc=None, sig=None — nothing to drop; remains a clean reasoning item.
        let items = vec![reasoning_with(None, None)];
        let out = sanitize_history_for_wire_api(WireApi::Chat, &items);
        match &out[0] {
            ResponseItem::Reasoning {
                encrypted_content,
                signature,
                summary,
                ..
            } => {
                assert!(encrypted_content.is_none());
                assert!(signature.is_none());
                assert_eq!(summary.len(), 1);
            }
            _ => panic!("expected reasoning"),
        }
    }

    #[test]
    fn multi_item_history_preserves_order_and_sanitizes_per_protocol() {
        // [Message(user), Reasoning(enc+sig), Message(assistant)]
        let history = vec![
            message("user", "hi"),
            reasoning_with(Some("E"), Some("S")),
            message("assistant", "hello"),
        ];

        for wire_api in [WireApi::Chat, WireApi::Responses, WireApi::Anthropic] {
            let out = sanitize_history_for_wire_api(wire_api, &history);
            // Length + ordering preserved.
            assert_eq!(out.len(), history.len());

            // Non-reasoning items unchanged.
            assert_eq!(out[0], history[0]);
            assert_eq!(out[2], history[2]);

            // Reasoning sanitized per protocol.
            match &out[1] {
                ResponseItem::Reasoning {
                    encrypted_content,
                    signature,
                    ..
                } => match wire_api {
                    WireApi::Chat => {
                        assert!(encrypted_content.is_none());
                        assert!(signature.is_none());
                    }
                    WireApi::Responses => {
                        assert!(encrypted_content.is_none());
                        assert!(signature.is_none());
                    }
                    WireApi::Anthropic => {
                        assert_eq!(encrypted_content.as_deref(), Some("E"));
                        assert_eq!(signature.as_deref(), Some("S"));
                    }
                },
                _ => panic!("expected reasoning at index 1"),
            }
        }
    }

    #[test]
    fn chat_drops_multiple_encrypted_items() {
        // Several encrypted reasoning items — all stripped (and a single aggregated
        // warn is emitted internally; behavior verified via the stripping result).
        let items = vec![
            reasoning_with(Some("E1"), Some("S1")),
            message("user", "between"),
            reasoning_with(Some("E2"), None),
            reasoning_with(None, None),
        ];
        let out = sanitize_history_for_wire_api(WireApi::Chat, &items);
        assert_eq!(out.len(), 4);
        for idx in [0usize, 2, 3] {
            match &out[idx] {
                ResponseItem::Reasoning {
                    encrypted_content,
                    signature,
                    ..
                } => {
                    assert!(encrypted_content.is_none());
                    assert!(signature.is_none());
                }
                _ => panic!("expected reasoning at index {idx}"),
            }
        }
        assert!(matches!(out[1], ResponseItem::Message { .. }));
    }
}
