use anyhow::Result;
use agere_core::config::Constrained;
use agere_protocol::protocol::AskForApproval;
use agere_protocol::protocol::COLLABORATION_MODE_CLOSE_TAG;
use agere_protocol::protocol::COLLABORATION_MODE_OPEN_TAG;
use agere_protocol::protocol::EventMsg;
use agere_protocol::protocol::Op;
use agere_protocol::protocol::RolloutItem;
use agere_protocol::protocol::RolloutLine;
use agere_protocol::protocol::ENVIRONMENT_CONTEXT_OPEN_TAG;
use agere_protocol::config_types::CollaborationMode;
use agere_protocol::config_types::ModeKind;
use agere_protocol::config_types::Settings;
use agere_protocol::models::ContentItem;
use agere_protocol::models::ResponseItem;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_agere::test_agere;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;

fn collab_mode_with_instructions(instructions: Option<&str>) -> CollaborationMode {
    CollaborationMode {
        mode: ModeKind::Default,
        settings: Settings {
            model: "gpt-5.4".to_string(),
            reasoning_effort: None,
            developer_instructions: instructions.map(str::to_string),
        },
    }
}

fn collab_xml(text: &str) -> String {
    format!("{COLLABORATION_MODE_OPEN_TAG}{text}{COLLABORATION_MODE_CLOSE_TAG}")
}

async fn read_rollout_text(path: &Path) -> anyhow::Result<String> {
    for _ in 0..50 {
        if path.exists()
            && let Ok(text) = std::fs::read_to_string(path)
            && !text.trim().is_empty()
        {
            return Ok(text);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    Ok(std::fs::read_to_string(path)?)
}

fn rollout_developer_texts(text: &str) -> Vec<String> {
    let mut texts = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let rollout: RolloutLine = match serde_json::from_str(trimmed) {
            Ok(rollout) => rollout,
            Err(_) => continue,
        };
        if let RolloutItem::ResponseItem(ResponseItem::Message { role, content, .. }) =
            rollout.item
            && role == "developer"
        {
            for item in content {
                if let ContentItem::InputText { text } = item {
                    texts.push(text);
                }
            }
        }
    }
    texts
}

fn rollout_environment_texts(text: &str) -> Vec<String> {
    let mut texts = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let rollout: RolloutLine = match serde_json::from_str(trimmed) {
            Ok(rollout) => rollout,
            Err(_) => continue,
        };
        if let RolloutItem::ResponseItem(ResponseItem::Message { role, content, .. }) =
            rollout.item
            && role == "user"
        {
            for item in content {
                if let ContentItem::InputText { text } = item
                    && text.starts_with(ENVIRONMENT_CONTEXT_OPEN_TAG)
                {
                    texts.push(text);
                }
            }
        }
    }
    texts
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn override_turn_context_without_user_turn_does_not_record_permissions_update() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_agere().with_config(|config| {
        config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
    });
    let test = builder.build(&server).await?;

    test.agere
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: Some(AskForApproval::Never),
            approvals_reviewer: None,
            permission_profile: None,
            windows_execution_restriction_level: None,
            model: None,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    test.agere.submit(Op::Shutdown).await?;
    wait_for_event(&test.agere, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    let rollout_path = test.agere.rollout_path().expect("rollout path");
    let rollout_text = read_rollout_text(&rollout_path).await?;
    let developer_texts = rollout_developer_texts(&rollout_text);
    let approval_texts: Vec<&String> = developer_texts
        .iter()
        .filter(|text| text.contains("`approval_policy`"))
        .collect();
    assert!(
        approval_texts.is_empty(),
        "did not expect permissions updates before a new user turn: {approval_texts:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn override_turn_context_without_user_turn_does_not_record_environment_update() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = test_agere().build(&server).await?;
    let new_cwd = TempDir::new()?;

    test.agere
        .submit(Op::OverrideTurnContext {
            cwd: Some(new_cwd.path().to_path_buf()),
            approval_policy: None,
            approvals_reviewer: None,
            permission_profile: None,
            windows_execution_restriction_level: None,
            model: None,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: None,
            personality: None,
        })
        .await?;

    test.agere.submit(Op::Shutdown).await?;
    wait_for_event(&test.agere, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    let rollout_path = test.agere.rollout_path().expect("rollout path");
    let rollout_text = read_rollout_text(&rollout_path).await?;
    let env_texts = rollout_environment_texts(&rollout_text);
    assert!(
        env_texts.is_empty(),
        "did not expect environment updates before a new user turn: {env_texts:?}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn override_turn_context_without_user_turn_does_not_record_collaboration_update() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = test_agere().build(&server).await?;
    let collab_text = "override collaboration instructions";
    let collaboration_mode = collab_mode_with_instructions(Some(collab_text));

    test.agere
        .submit(Op::OverrideTurnContext {
            cwd: None,
            approval_policy: None,
            approvals_reviewer: None,
            permission_profile: None,
            windows_execution_restriction_level: None,
            model: None,
            effort: None,
            summary: None,
            service_tier: None,
            collaboration_mode: Some(collaboration_mode),
            personality: None,
        })
        .await?;

    test.agere.submit(Op::Shutdown).await?;
    wait_for_event(&test.agere, |ev| matches!(ev, EventMsg::ShutdownComplete)).await;

    let rollout_path = test.agere.rollout_path().expect("rollout path");
    let rollout_text = read_rollout_text(&rollout_path).await?;
    let developer_texts = rollout_developer_texts(&rollout_text);
    let collab_text = collab_xml(collab_text);
    let collab_count = developer_texts
        .iter()
        .filter(|text| text.as_str() == collab_text.as_str())
        .count();
    assert_eq!(collab_count, 0);

    Ok(())
}
