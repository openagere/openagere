#![cfg(not(target_os = "windows"))]

use std::sync::Arc;

use agere_core::AgereThread;
use agere_protocol::protocol::EventMsg;
use agere_protocol::protocol::Op;
use agere_protocol::protocol::UndoCompletedEvent;
use anyhow::Result;
use core_test_support::test_agere::TestAgereHarness;
use core_test_support::test_agere::test_agere;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;

async fn undo_harness() -> Result<TestAgereHarness> {
    TestAgereHarness::with_builder(test_agere().with_model("gpt-5.4")).await
}

async fn invoke_undo(agere: &Arc<AgereThread>) -> Result<UndoCompletedEvent> {
    agere.submit(Op::Undo).await?;
    let event = wait_for_event_match(agere, |msg| match msg {
        EventMsg::UndoCompleted(done) => Some(done.clone()),
        _ => None,
    })
    .await;
    Ok(event)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_reports_feature_removal() -> Result<()> {
    let harness = undo_harness().await?;
    let agere = Arc::clone(&harness.test().agere);

    let event = invoke_undo(&agere).await?;

    assert!(!event.success, "expected undo to fail");
    assert_eq!(
        event.message.as_deref(),
        Some("Undo is no longer available.")
    );

    Ok(())
}
