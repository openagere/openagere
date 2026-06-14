use crate::translate::response::SseState;
use crate::translate::response::handle_sse_event;
use crate::types::SseEvent;
use agere_api::ApiError;
use agere_api::ResponseEvent;
use agere_client::ByteStream;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;
use tracing::debug;
use tracing::error;
use tracing::trace;

/// Process an Anthropic SSE byte stream, emitting ResponseEvents to the channel.
pub(crate) async fn process_anthropic_sse(
    stream: ByteStream,
    tx_event: mpsc::Sender<Result<ResponseEvent, ApiError>>,
    idle_timeout: Duration,
) {
    let mut sse_stream = stream.eventsource();
    let mut response_error: Option<ApiError> = None;
    let mut state = SseState::new();

    loop {
        let response = timeout(idle_timeout, sse_stream.next()).await;

        let sse = match response {
            Ok(Some(Ok(sse))) => sse,
            Ok(Some(Err(e))) => {
                debug!("Anthropic SSE error: {e:#}");
                let _ = tx_event.send(Err(ApiError::Stream(e.to_string()))).await;
                return;
            }
            Ok(None) => {
                let error = response_error.unwrap_or_else(|| {
                    ApiError::Stream("stream closed before message_delta".into())
                });
                let _ = tx_event.send(Err(error)).await;
                return;
            }
            Err(_) => {
                let _ = tx_event
                    .send(Err(ApiError::Stream("idle timeout waiting for SSE".into())))
                    .await;
                return;
            }
        };

        trace!("Anthropic SSE: {}", &sse.data);

        let event: SseEvent = match serde_json::from_str(&sse.data) {
            Ok(event) => event,
            Err(e) => {
                debug!("Failed to parse Anthropic SSE: {e}, data: {}", &sse.data);
                continue;
            }
        };

        // Log any Anthropic API error immediately
        if let SseEvent::Error { error: ref err } = event {
            error!(
                "Anthropic API error: type={} message={}",
                err.error_type, err.message
            );
        }

        for result in handle_sse_event(&event, &mut state) {
            match result {
                Ok(response_event) => {
                    let is_completed = matches!(response_event, ResponseEvent::Completed { .. });
                    if tx_event.send(Ok(response_event)).await.is_err() {
                        return;
                    }
                    if is_completed {
                        return;
                    }
                }
                Err(error) => {
                    response_error = Some(error);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use futures::stream;
    use std::time::Duration;

    #[tokio::test]
    async fn full_text_lifecycle() {
        let body = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-sonnet-4-6\",\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":10,\"output_tokens\":3}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        );

        let events = collect_sse(body).await;
        assert_eq!(events.len(), 4);
        assert!(matches!(&events[0], Ok(ResponseEvent::OutputItemAdded(_))));
        assert!(matches!(&events[1], Ok(ResponseEvent::OutputTextDelta(t)) if t == "Hello"));
        assert!(matches!(
            &events[2],
            Ok(ResponseEvent::OutputItemDone { .. })
        ));
        assert!(matches!(
            &events[3],
            Ok(ResponseEvent::Completed {
                end_turn: Some(true),
                ..
            })
        ));
    }

    #[tokio::test]
    async fn error_event_propagates() {
        let body = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Server busy\"}}\n\n";
        let events = collect_sse(body).await;
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], Err(ApiError::ServerOverloaded)));
    }

    #[tokio::test]
    async fn ping_skipped() {
        let body = concat!(
            "event: ping\ndata: {\"type\":\"ping\"}\n\n",
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"m\",\"usage\":{\"input_tokens\":1,\"output_tokens\":0}}}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":1,\"output_tokens\":1}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        );
        let events = collect_sse(body).await;
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], Ok(ResponseEvent::Completed { .. })));
    }

    #[tokio::test]
    async fn tool_use_lifecycle() {
        let body = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_2\",\"model\":\"m\",\"usage\":{\"input_tokens\":20,\"output_tokens\":0}}}\n\n",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_001\",\"name\":\"get_weather\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"city\\\":\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"SF\\\"}\"}}\n\n",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"tool_use\"},\"usage\":{\"input_tokens\":20,\"output_tokens\":5}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        );
        let events = collect_sse(body).await;
        assert_eq!(events.len(), 5);
        assert!(matches!(&events[0], Ok(ResponseEvent::OutputItemAdded(_))));
        assert!(matches!(
            &events[1],
            Ok(ResponseEvent::ToolCallInputDelta { .. })
        ));
        assert!(matches!(
            &events[2],
            Ok(ResponseEvent::ToolCallInputDelta { .. })
        ));
        assert!(matches!(
            &events[3],
            Ok(ResponseEvent::OutputItemDone {
                item: agere_protocol::models::ResponseItem::FunctionCall { .. },
                ..
            })
        ));
        assert!(matches!(
            &events[4],
            Ok(ResponseEvent::Completed {
                end_turn: Some(false),
                ..
            })
        ));
    }

    #[tokio::test]
    async fn redacted_thinking_lifecycle() {
        let body = concat!(
            "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_3\",\"model\":\"m\",\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}}\n\n",
            "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"redacted_thinking\",\"data\":\"ZW5jcnlwdGVk\"}}\n\n",
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"sig_redacted\"}}\n\n",
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"input_tokens\":10,\"output_tokens\":0}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        );
        let events = collect_sse(body).await;
        // Expect: OutputItemAdded(Reasoning with encrypted_content), OutputItemDone(Reasoning with encrypted_content+signature), Completed
        assert_eq!(events.len(), 3);

        // Verify OutputItemAdded carries encrypted_content
        match &events[0] {
            Ok(ResponseEvent::OutputItemAdded(item)) => match item {
                agere_protocol::models::ResponseItem::Reasoning {
                    encrypted_content,
                    signature,
                    ..
                } => {
                    assert_eq!(encrypted_content.as_deref(), Some("ZW5jcnlwdGVk"));
                    assert!(signature.is_none());
                }
                other => panic!("expected Reasoning item, got {other:?}"),
            },
            other => panic!("expected OutputItemAdded, got {other:?}"),
        }

        // Verify OutputItemDone carries both encrypted_content and signature
        match &events[1] {
            Ok(ResponseEvent::OutputItemDone { item, .. }) => match item {
                agere_protocol::models::ResponseItem::Reasoning {
                    encrypted_content,
                    signature,
                    ..
                } => {
                    assert_eq!(encrypted_content.as_deref(), Some("ZW5jcnlwdGVk"));
                    assert_eq!(signature.as_deref(), Some("sig_redacted"));
                }
                other => panic!("expected Reasoning item, got {other:?}"),
            },
            other => panic!("expected OutputItemDone, got {other:?}"),
        }

        assert!(matches!(&events[2], Ok(ResponseEvent::Completed { .. })));
    }

    async fn collect_sse(body: &str) -> Vec<Result<ResponseEvent, ApiError>> {
        let chunks: Vec<Result<Bytes, agere_client::TransportError>> =
            vec![Ok(Bytes::from(body.to_string()))];
        let byte_stream: ByteStream = Box::pin(stream::iter(chunks));

        let (tx, mut rx) = mpsc::channel::<Result<ResponseEvent, ApiError>>(16);
        tokio::spawn(process_anthropic_sse(
            byte_stream,
            tx,
            Duration::from_secs(5),
        ));

        let mut events = Vec::new();
        while let Some(ev) = rx.recv().await {
            let is_terminal = matches!(ev, Ok(ResponseEvent::Completed { .. }) | Err(_));
            events.push(ev);
            if is_terminal {
                break;
            }
        }
        events
    }
}
