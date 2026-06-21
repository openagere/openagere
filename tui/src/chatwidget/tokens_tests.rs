use super::*;
use crate::app_event::AppEvent;
use crate::chatwidget::tests::make_chatwidget_manual_with_sender;
use agere_app_server_protocol::GetProviderUsageResponse;
use agere_app_server_protocol::ProviderUsageTotal;
use pretty_assertions::assert_eq;

fn empty_usage_response() -> GetProviderUsageResponse {
    GetProviderUsageResponse {
        total: ProviderUsageTotal {
            total_tokens: 0,
            input_tokens: 0,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            peak_daily_tokens: 0,
            longest_running_turn_sec: None,
            daily_buckets: vec![],
        },
        providers: vec![],
    }
}

#[test]
fn loaded_state_freezes_chart_anchor_date_at_completion() {
    let state = Arc::new(RwLock::new(TokenActivityState::Loading));
    let handle = TokenActivityHandle {
        state: Arc::clone(&state),
    };
    let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("valid date");

    handle.finish_with_today(Ok(empty_usage_response()), today);

    let state = state.read().expect("token activity state poisoned");
    match &*state {
        TokenActivityState::Loaded {
            today: loaded_today,
            ..
        } => {
            assert_eq!(*loaded_today, today);
        }
        other => panic!("expected loaded state, got {other:?}"),
    }
}

#[tokio::test]
async fn flushing_active_cell_retries_completed_usage_output_insertion() {
    let (mut chat, _app_event_tx, mut app_event_rx, _op_rx) =
        make_chatwidget_manual_with_sender().await;
    chat.add_token_activity_output(TokenActivityView::Daily);
    let request_id = match app_event_rx.try_recv() {
        Ok(AppEvent::RefreshTokenActivity { request_id }) => request_id,
        other => panic!("expected token activity refresh request, got {other:?}"),
    };
    chat.active_cell = Some(Box::new(PlainHistoryCell::new(vec!["active".into()])));

    assert!(chat.finish_token_activity_refresh(request_id, Ok(empty_usage_response())));
    assert!(chat.completed_token_activity.is_some());

    chat.flush_active_cell();

    assert!(matches!(
        app_event_rx.try_recv(),
        Ok(AppEvent::InsertHistoryCell(_))
    ));
    assert!(matches!(
        app_event_rx.try_recv(),
        Ok(AppEvent::CommitPendingUsageOutput)
    ));
}
