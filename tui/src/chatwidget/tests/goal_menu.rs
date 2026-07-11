use super::*;

#[tokio::test]
async fn goal_menu_active_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();

    chat.show_goal_summary(test_goal(
        thread_id,
        AppThreadGoalStatus::Active,
        /*token_budget*/ Some(80_000),
    ));

    assert_chatwidget_snapshot!("goal_menu_active", rendered_goal_summary(&mut rx));
}

#[tokio::test]
async fn goal_menu_paused_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();

    chat.show_goal_summary(test_goal(
        thread_id,
        AppThreadGoalStatus::Paused,
        /*token_budget*/ None,
    ));

    assert_chatwidget_snapshot!("goal_menu_paused", rendered_goal_summary(&mut rx));
}

#[tokio::test]
async fn goal_menu_blocked_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();

    chat.show_goal_summary(test_goal(
        thread_id,
        AppThreadGoalStatus::Blocked,
        /*token_budget*/ None,
    ));

    assert_chatwidget_snapshot!("goal_menu_blocked", rendered_goal_summary(&mut rx));
}

#[tokio::test]
async fn goal_menu_usage_limited_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();

    chat.show_goal_summary(test_goal(
        thread_id,
        AppThreadGoalStatus::UsageLimited,
        /*token_budget*/ None,
    ));

    assert_chatwidget_snapshot!("goal_menu_usage_limited", rendered_goal_summary(&mut rx));
}

#[tokio::test]
async fn goal_menu_budget_limited_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();

    chat.show_goal_summary(test_goal(
        thread_id,
        AppThreadGoalStatus::BudgetLimited,
        /*token_budget*/ Some(80_000),
    ));

    assert_chatwidget_snapshot!("goal_menu_budget_limited", rendered_goal_summary(&mut rx));
}

#[tokio::test]
async fn resume_paused_goal_prompt_snapshot() {
    let (mut chat, _rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();

    chat.show_resume_paused_goal_prompt(
        thread_id,
        "Keep improving the bare goal command until it feels calm and useful.".to_string(),
    );

    assert_chatwidget_snapshot!(
        "resume_paused_goal_prompt",
        render_bottom_popup(&chat, /*width*/ 100)
    );
}

#[tokio::test]
async fn resume_paused_goal_prompt_default_resumes_goal() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();

    chat.show_resume_paused_goal_prompt(thread_id, "Finish the paused goal.".to_string());
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    match rx.try_recv() {
        Ok(AppEvent::SetThreadGoalStatus {
            thread_id: event_thread_id,
            status,
        }) => {
            assert_eq!(event_thread_id, thread_id);
            assert_eq!(status, AppThreadGoalStatus::Active);
        }
        other => panic!("expected SetThreadGoalStatus event, got {other:?}"),
    }
    assert!(chat.no_modal_or_popup_active());
}

#[tokio::test]
async fn resume_paused_goal_prompt_can_leave_goal_paused() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();

    chat.show_resume_paused_goal_prompt(thread_id, "Finish the paused goal.".to_string());
    chat.handle_key_event(KeyEvent::from(KeyCode::Down));
    chat.handle_key_event(KeyEvent::from(KeyCode::Enter));

    assert!(matches!(rx.try_recv(), Err(TryRecvError::Empty)));
    assert!(chat.no_modal_or_popup_active());
}

#[tokio::test]
async fn goal_edit_prompt_updates_existing_goal() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    let goal = test_goal(
        thread_id,
        AppThreadGoalStatus::Blocked,
        /*token_budget*/ Some(80_000),
    );

    chat.show_goal_edit_prompt(thread_id, goal);
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let event = rx.try_recv().expect("expected goal update event");
    let AppEvent::SetThreadGoalDraft {
        thread_id: actual_thread_id,
        draft,
        mode,
    } = event
    else {
        panic!("expected SetThreadGoalDraft, got {event:?}");
    };
    assert_eq!(actual_thread_id, thread_id);
    assert_eq!(
        draft.objective,
        "Keep improving the bare goal command until it feels calm and useful."
    );
    assert_eq!(
        mode,
        crate::app_event::ThreadGoalSetMode::UpdateExisting {
            status: AppThreadGoalStatus::Blocked,
            token_budget: Some(80_000),
        }
    );
}

#[tokio::test]
async fn goal_edit_prompt_reactivates_complete_goal() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let thread_id = ThreadId::new();
    let goal = test_goal(
        thread_id,
        AppThreadGoalStatus::Complete,
        /*token_budget*/ None,
    );

    chat.show_goal_edit_prompt(thread_id, goal);
    chat.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let event = rx.try_recv().expect("expected goal update event");
    assert_matches!(
        event,
        AppEvent::SetThreadGoalDraft {
            thread_id: actual_thread_id,
            mode:
                crate::app_event::ThreadGoalSetMode::UpdateExisting {
                    status: AppThreadGoalStatus::Active,
                    token_budget: None,
                },
            ..
        } if actual_thread_id == thread_id
    );
}

fn test_goal(
    thread_id: ThreadId,
    status: AppThreadGoalStatus,
    token_budget: Option<i64>,
) -> AppThreadGoal {
    AppThreadGoal {
        thread_id: thread_id.to_string(),
        objective: "Keep improving the bare goal command until it feels calm and useful."
            .to_string(),
        status,
        token_budget,
        tokens_used: 12_500,
        time_used_seconds: 90,
        created_at: 1_776_272_400,
        updated_at: 1_776_272_460,
    }
}

fn rendered_goal_summary(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<crate::app_event::AppEvent>,
) -> String {
    drain_insert_history(rx)
        .iter()
        .map(|lines| lines_to_single_string(lines))
        .collect::<Vec<_>>()
        .join("\n")
}
