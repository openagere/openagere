#![allow(warnings, clippy::all)]

use super::*;
use crate::list::parse_cursor;
use agere_protocol::ThreadId;
use chrono::DateTime;
use chrono::NaiveDateTime;
use chrono::Timelike;
use chrono::Utc;
use pretty_assertions::assert_eq;
use std::future;
use std::io::Write;
use uuid::Uuid;

#[test]
fn cursor_to_anchor_normalizes_timestamp_format() {
    let ts_str = "2026-01-27T12-34-56";
    let cursor = parse_cursor(ts_str).expect("cursor should parse");
    let anchor = cursor_to_anchor(Some(&cursor)).expect("anchor should parse");

    let naive =
        NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%dT%H-%M-%S").expect("ts should parse");
    let expected_ts = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc)
        .with_nanosecond(0)
        .expect("nanosecond");

    assert_eq!(anchor.ts, expected_ts);
}

#[tokio::test]
async fn init_completes_rollout_backfill_before_returning() {
    let home = tempfile::tempdir().expect("temp dir");
    let config = RolloutConfig {
        agere_home: home.path().to_path_buf(),
        sqlite_home: home.path().to_path_buf(),
        cwd: home.path().to_path_buf(),
        model_provider_id: "test-provider".to_string(),
        generate_memories: true,
    };
    let uuid = Uuid::from_u128(42);
    let thread_id = ThreadId::from_string(&uuid.to_string()).expect("thread id");
    let day_dir = home.path().join("sessions/2025/01/03");
    std::fs::create_dir_all(&day_dir).expect("sessions dir");
    let rollout_path = day_dir.join(format!("rollout-2025-01-03T12-00-00-{uuid}.jsonl"));
    let mut file = std::fs::File::create(&rollout_path).expect("rollout file");
    let meta = serde_json::json!({
        "timestamp": "2025-01-03T12-00-00",
        "type": "session_meta",
        "payload": {
            "id": uuid,
            "timestamp": "2025-01-03T12-00-00",
            "cwd": home.path(),
            "originator": "test_originator",
            "cli_version": "test_version",
            "source": "cli",
            "model_provider": "test-provider"
        },
    });
    writeln!(file, "{meta}").expect("write meta");
    let user_event = serde_json::json!({
        "timestamp": "2025-01-03T12-00-01",
        "type": "event_msg",
        "payload": {
            "type": "user_message",
            "message": "Hello from backfill",
            "kind": "plain"
        },
    });
    writeln!(file, "{user_event}").expect("write user event");

    let runtime = init(&config).await.expect("state db should initialize");

    let backfill_state = runtime
        .get_backfill_state()
        .await
        .expect("read backfill state");
    assert_eq!(backfill_state.status, agere_state::BackfillStatus::Complete);
    let metadata = runtime
        .get_thread(thread_id)
        .await
        .expect("read thread metadata")
        .expect("thread should be backfilled before init returns");
    assert_eq!(metadata.rollout_path, rollout_path);
}

#[tokio::test]
async fn init_returns_handle_when_backfill_is_owned_by_peer() {
    let home = tempfile::tempdir().expect("temp dir");
    let config = RolloutConfig {
        agere_home: home.path().to_path_buf(),
        sqlite_home: home.path().to_path_buf(),
        cwd: home.path().to_path_buf(),
        model_provider_id: "test-provider".to_string(),
        generate_memories: true,
    };
    let peer =
        agere_state::StateRuntime::init(home.path().to_path_buf(), "test-provider".to_string())
            .await
            .expect("peer state db should initialize");
    let start_second = chrono::Utc::now().timestamp();
    while chrono::Utc::now().timestamp() == start_second {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        peer.try_claim_backfill(/*lease_seconds*/ 3600)
            .await
            .expect("peer should claim backfill")
    );
    let peer_for_lease_renewal = peer.clone();
    let lease_renewal = tokio::spawn(async move {
        for _ in 0..3000 {
            peer_for_lease_renewal
                .mark_backfill_running()
                .await
                .expect("peer should renew backfill lease");
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    });
    tokio::task::yield_now().await;

    let runtime = init(&config)
        .await
        .expect("state db should remain available while peer owns backfill");

    lease_renewal.abort();
    let backfill_state = runtime
        .get_backfill_state()
        .await
        .expect("read backfill state");
    assert_ne!(backfill_state.status, agere_state::BackfillStatus::Pending);
}

#[tokio::test]
async fn init_backfill_gate_is_bounded_when_this_process_runs_backfill() {
    let home = tempfile::tempdir().expect("temp dir");
    let config = RolloutConfig {
        agere_home: home.path().to_path_buf(),
        sqlite_home: home.path().to_path_buf(),
        cwd: home.path().to_path_buf(),
        model_provider_id: "test-provider".to_string(),
        generate_memories: true,
    };
    let runtime =
        agere_state::StateRuntime::init(home.path().to_path_buf(), "test-provider".to_string())
            .await
            .expect("state db should initialize");
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();

    let started = std::time::Instant::now();
    wait_for_backfill_gate(runtime.clone(), config, move |runtime, _config| {
        tokio::spawn(async move {
            runtime
                .mark_backfill_running()
                .await
                .expect("mark backfill running");
            let _ = started_tx.send(());
            future::pending::<()>().await;
        })
    })
    .await
    .expect("backfill gate should time out without failing startup");

    started_rx.await.expect("backfill task should start");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(1),
        "startup backfill gate took {:?}",
        started.elapsed()
    );
}
