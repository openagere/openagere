//! Exercises `ChatWidget` event handling and rendering invariants.
//!
//! These tests treat the widget as the adapter between `agere_protocol::protocol::EventMsg` inputs and
//! the TUI output. Many assertions are snapshot-based so that layout regressions and status/header
//! changes show up as stable, reviewable diffs.

pub(super) use super::*;
pub(super) use crate::app_event::AppEvent;
pub(super) use crate::app_event::ExitMode;
#[cfg(not(target_os = "linux"))]
pub(super) use crate::app_event::RealtimeAudioDeviceKind;
pub(super) use crate::app_event_sender::AppEventSender;
pub(super) use crate::bottom_pane::LocalImageAttachment;
pub(super) use crate::bottom_pane::MentionBinding;
pub(super) use crate::bottom_pane::QueuedInputAction;
pub(super) use crate::chatwidget::realtime::RealtimeConversationPhase;
pub(super) use crate::history_cell::UserHistoryCell;
pub(super) use crate::legacy_core::config::Config;
pub(super) use crate::legacy_core::config::ConfigBuilder;
pub(super) use crate::legacy_core::config::Constrained;
pub(super) use crate::legacy_core::config::ConstraintError;
pub(super) use crate::model_catalog::ModelCatalog;
pub(super) use crate::terminal_info::Multiplexer;
pub(super) use crate::terminal_info::TerminalInfo;
pub(super) use crate::terminal_info::TerminalName;
pub(super) use crate::test_backend::VT100Backend;
pub(super) use crate::test_support::PathBufExt;
pub(super) use crate::test_support::test_path_buf;
pub(super) use crate::test_support::test_path_display;
pub(super) use crate::tui::FrameRequester;
pub(super) use agere_app_server_protocol::AddCreditsNudgeCreditType;
pub(super) use agere_app_server_protocol::AddCreditsNudgeEmailStatus;
pub(super) use agere_app_server_protocol::AdditionalFileSystemPermissions as AppServerAdditionalFileSystemPermissions;
pub(super) use agere_app_server_protocol::AdditionalNetworkPermissions as AppServerAdditionalNetworkPermissions;
pub(super) use agere_app_server_protocol::AdditionalPermissionProfile as AppServerAdditionalPermissionProfile;
pub(super) use agere_app_server_protocol::AppSummary;
pub(super) use agere_app_server_protocol::AutoReviewDecisionSource as AppServerGuardianApprovalReviewDecisionSource;
pub(super) use agere_app_server_protocol::CollabAgentState as AppServerCollabAgentState;
pub(super) use agere_app_server_protocol::CollabAgentStatus as AppServerCollabAgentStatus;
pub(super) use agere_app_server_protocol::CollabAgentTool as AppServerCollabAgentTool;
pub(super) use agere_app_server_protocol::CollabAgentToolCallStatus as AppServerCollabAgentToolCallStatus;
pub(super) use agere_app_server_protocol::CommandAction as AppServerCommandAction;
pub(super) use agere_app_server_protocol::CommandExecutionRequestApprovalParams as AppServerCommandExecutionRequestApprovalParams;
pub(super) use agere_app_server_protocol::CommandExecutionSource as AppServerCommandExecutionSource;
pub(super) use agere_app_server_protocol::CommandExecutionStatus as AppServerCommandExecutionStatus;
pub(super) use agere_app_server_protocol::ConfigWarningNotification;
pub(super) use agere_app_server_protocol::ErrorNotification;
pub(super) use agere_app_server_protocol::FileUpdateChange;
pub(super) use agere_app_server_protocol::GuardianApprovalReview;
pub(super) use agere_app_server_protocol::GuardianApprovalReviewAction as AppServerGuardianApprovalReviewAction;
pub(super) use agere_app_server_protocol::GuardianApprovalReviewStatus;
pub(super) use agere_app_server_protocol::GuardianCommandSource as AppServerGuardianCommandSource;
pub(super) use agere_app_server_protocol::GuardianRiskLevel as AppServerGuardianRiskLevel;
pub(super) use agere_app_server_protocol::GuardianUserAuthorization as AppServerGuardianUserAuthorization;
pub(super) use agere_app_server_protocol::GuardianWarningNotification;
pub(super) use agere_app_server_protocol::HookCompletedNotification as AppServerHookCompletedNotification;
pub(super) use agere_app_server_protocol::HookEventName as AppServerHookEventName;
pub(super) use agere_app_server_protocol::HookExecutionMode as AppServerHookExecutionMode;
pub(super) use agere_app_server_protocol::HookHandlerType as AppServerHookHandlerType;
pub(super) use agere_app_server_protocol::HookOutputEntry as AppServerHookOutputEntry;
pub(super) use agere_app_server_protocol::HookOutputEntryKind as AppServerHookOutputEntryKind;
pub(super) use agere_app_server_protocol::HookRunStatus as AppServerHookRunStatus;
pub(super) use agere_app_server_protocol::HookRunSummary as AppServerHookRunSummary;
pub(super) use agere_app_server_protocol::HookScope as AppServerHookScope;
pub(super) use agere_app_server_protocol::HookStartedNotification as AppServerHookStartedNotification;
pub(super) use agere_app_server_protocol::ItemCompletedNotification;
pub(super) use agere_app_server_protocol::ItemGuardianApprovalReviewCompletedNotification;
pub(super) use agere_app_server_protocol::ItemGuardianApprovalReviewStartedNotification;
pub(super) use agere_app_server_protocol::ItemStartedNotification;
pub(super) use agere_app_server_protocol::MarketplaceAddResponse;
pub(super) use agere_app_server_protocol::MarketplaceInterface;
pub(super) use agere_app_server_protocol::McpServerStartupState;
pub(super) use agere_app_server_protocol::McpServerStatusDetail;
pub(super) use agere_app_server_protocol::McpServerStatusUpdatedNotification;
pub(super) use agere_app_server_protocol::ModelVerification as AppServerModelVerification;
pub(super) use agere_app_server_protocol::ModelVerificationNotification;
pub(super) use agere_app_server_protocol::PatchApplyStatus as AppServerPatchApplyStatus;
pub(super) use agere_app_server_protocol::PatchChangeKind;
pub(super) use agere_app_server_protocol::PermissionsRequestApprovalParams as AppServerPermissionsRequestApprovalParams;
pub(super) use agere_app_server_protocol::PluginAuthPolicy;
pub(super) use agere_app_server_protocol::PluginDetail;
pub(super) use agere_app_server_protocol::PluginInstallPolicy;
pub(super) use agere_app_server_protocol::PluginInterface;
pub(super) use agere_app_server_protocol::PluginListResponse;
pub(super) use agere_app_server_protocol::PluginMarketplaceEntry;
pub(super) use agere_app_server_protocol::PluginReadResponse;
pub(super) use agere_app_server_protocol::PluginSource;
pub(super) use agere_app_server_protocol::PluginSummary;
pub(super) use agere_app_server_protocol::ReasoningSummaryTextDeltaNotification;
pub(super) use agere_app_server_protocol::ServerNotification;
pub(super) use agere_app_server_protocol::SkillSummary;
pub(super) use agere_app_server_protocol::ThreadClosedNotification;
pub(super) use agere_app_server_protocol::ThreadItem as AppServerThreadItem;
pub(super) use agere_app_server_protocol::Turn as AppServerTurn;
pub(super) use agere_app_server_protocol::TurnCompletedNotification;
pub(super) use agere_app_server_protocol::TurnError as AppServerTurnError;
pub(super) use agere_app_server_protocol::TurnStartedNotification;
pub(super) use agere_app_server_protocol::TurnStatus as AppServerTurnStatus;
pub(super) use agere_app_server_protocol::UserInput as AppServerUserInput;
pub(super) use agere_app_server_protocol::WarningNotification;
pub(super) use agere_config::AppRequirementToml;
pub(super) use agere_config::AppsRequirementsToml;
pub(super) use agere_config::ConfigLayerStack;
pub(super) use agere_config::ConfigRequirements;
pub(super) use agere_config::ConfigRequirementsToml;
pub(super) use agere_config::RequirementSource;
pub(super) use agere_config::types::ApprovalsReviewer;
pub(super) use agere_config::types::Notifications;
#[cfg(target_os = "windows")]
pub(super) use agere_config::types::WindowsAccessModeToml;
pub(super) use agere_core_plugins::OPENAGERE_CURATED_MARKETPLACE_NAME;
pub(super) use agere_core_skills::model::SkillMetadata;
pub(super) use agere_features::FEATURES;
pub(super) use agere_features::Feature;
pub(super) use agere_git_utils::CommitLogEntry;
pub(super) use agere_models_manager::collaboration_mode_presets::CollaborationModesConfig;
pub(super) use agere_otel::RuntimeMetricsSummary;
pub(super) use agere_otel::SessionTelemetry;
pub(super) use agere_protocol::ThreadId;
pub(super) use agere_protocol::account::PlanType;
pub(super) use agere_protocol::config_types::CollaborationMode;
pub(super) use agere_protocol::config_types::ModeKind;
pub(super) use agere_protocol::config_types::Personality;
pub(super) use agere_protocol::config_types::ServiceTier;
pub(super) use agere_protocol::config_types::Settings;
pub(super) use agere_protocol::items::AgentMessageContent;
pub(super) use agere_protocol::items::AgentMessageItem;
pub(super) use agere_protocol::items::PlanItem;
pub(super) use agere_protocol::items::TurnItem;
pub(super) use agere_protocol::items::UserMessageItem;
pub(super) use agere_protocol::models::FileSystemPermissions;
pub(super) use agere_protocol::models::MessagePhase;
pub(super) use agere_protocol::models::NetworkPermissions;
pub(super) use agere_protocol::models::PermissionProfile;
pub(super) use agere_protocol::openai_models::ModelInfo;
pub(super) use agere_protocol::openai_models::ModelPreset;
pub(super) use agere_protocol::openai_models::ModelsResponse;
pub(super) use agere_protocol::openai_models::ReasoningEffortPreset;
pub(super) use agere_protocol::openai_models::default_input_modalities;
pub(super) use agere_protocol::parse_command::ParsedCommand;
pub(super) use agere_protocol::plan_tool::PlanItemArg;
pub(super) use agere_protocol::plan_tool::StepStatus;
pub(super) use agere_protocol::plan_tool::UpdatePlanArgs;
pub(super) use agere_protocol::protocol::AgentMessageDeltaEvent;
pub(super) use agere_protocol::protocol::AgentMessageEvent;
pub(super) use agere_protocol::protocol::AgentReasoningDeltaEvent;
pub(super) use agere_protocol::protocol::AgentReasoningEvent;
pub(super) use agere_protocol::protocol::AgentStatus;
pub(super) use agere_protocol::protocol::AgereErrorInfo;
pub(super) use agere_protocol::protocol::ApplyPatchApprovalRequestEvent;
pub(super) use agere_protocol::protocol::BackgroundEventEvent;
pub(super) use agere_protocol::protocol::CollabAgentSpawnBeginEvent;
pub(super) use agere_protocol::protocol::CollabAgentSpawnEndEvent;
pub(super) use agere_protocol::protocol::CreditsSnapshot;
pub(super) use agere_protocol::protocol::ErrorEvent;
pub(super) use agere_protocol::protocol::Event;
pub(super) use agere_protocol::protocol::EventMsg;
pub(super) use agere_protocol::protocol::ExecApprovalRequestEvent;
pub(super) use agere_protocol::protocol::ExecCommandBeginEvent;
pub(super) use agere_protocol::protocol::ExecCommandEndEvent;
pub(super) use agere_protocol::protocol::ExecCommandSource;
pub(super) use agere_protocol::protocol::ExecCommandStatus as CoreExecCommandStatus;
pub(super) use agere_protocol::protocol::ExecPolicyAmendment;
pub(super) use agere_protocol::protocol::ExitedReviewModeEvent;
pub(super) use agere_protocol::protocol::FileChange;
pub(super) use agere_protocol::protocol::GuardianAssessmentAction;
pub(super) use agere_protocol::protocol::GuardianAssessmentDecisionSource;
pub(super) use agere_protocol::protocol::GuardianAssessmentEvent;
pub(super) use agere_protocol::protocol::GuardianAssessmentStatus;
pub(super) use agere_protocol::protocol::GuardianCommandSource;
pub(super) use agere_protocol::protocol::GuardianRiskLevel;
pub(super) use agere_protocol::protocol::GuardianUserAuthorization;
pub(super) use agere_protocol::protocol::ImageGenerationEndEvent;
pub(super) use agere_protocol::protocol::ItemCompletedEvent;
pub(super) use agere_protocol::protocol::McpStartupCompleteEvent;
pub(super) use agere_protocol::protocol::McpStartupStatus;
pub(super) use agere_protocol::protocol::McpStartupUpdateEvent;
pub(super) use agere_protocol::protocol::ModelVerification as CoreModelVerification;
pub(super) use agere_protocol::protocol::ModelVerificationEvent;
pub(super) use agere_protocol::protocol::NonSteerableTurnKind;
pub(super) use agere_protocol::protocol::Op;
pub(super) use agere_protocol::protocol::PatchApplyBeginEvent;
pub(super) use agere_protocol::protocol::PatchApplyEndEvent;
pub(super) use agere_protocol::protocol::PatchApplyStatus as CorePatchApplyStatus;
pub(super) use agere_protocol::protocol::RateLimitReachedType;
pub(super) use agere_protocol::protocol::RateLimitSnapshot;
pub(super) use agere_protocol::protocol::RateLimitWindow;
pub(super) use agere_protocol::protocol::RealtimeConversationClosedEvent;
pub(super) use agere_protocol::protocol::RealtimeConversationRealtimeEvent;
pub(super) use agere_protocol::protocol::RealtimeEvent;
pub(super) use agere_protocol::protocol::ReviewRequest;
pub(super) use agere_protocol::protocol::ReviewTarget;
pub(super) use agere_protocol::protocol::SessionConfiguredEvent;
pub(super) use agere_protocol::protocol::SessionSource;
pub(super) use agere_protocol::protocol::SkillScope;
pub(super) use agere_protocol::protocol::StreamErrorEvent;
pub(super) use agere_protocol::protocol::TerminalInteractionEvent;
pub(super) use agere_protocol::protocol::ThreadRolledBackEvent;
pub(super) use agere_protocol::protocol::TokenCountEvent;
pub(super) use agere_protocol::protocol::TokenUsage;
pub(super) use agere_protocol::protocol::TokenUsageInfo;
pub(super) use agere_protocol::protocol::TurnCompleteEvent;
pub(super) use agere_protocol::protocol::TurnStartedEvent;
pub(super) use agere_protocol::protocol::UndoCompletedEvent;
pub(super) use agere_protocol::protocol::UndoStartedEvent;
pub(super) use agere_protocol::protocol::ViewImageToolCallEvent;
pub(super) use agere_protocol::protocol::WarningEvent;
pub(super) use agere_protocol::request_permissions::RequestPermissionProfile;
pub(super) use agere_protocol::request_user_input::RequestUserInputEvent;
pub(super) use agere_protocol::request_user_input::RequestUserInputQuestion;
pub(super) use agere_protocol::request_user_input::RequestUserInputQuestionOption;
pub(super) use agere_protocol::user_input::TextElement;
pub(super) use agere_protocol::user_input::UserInput;
pub(super) use agere_utils_approval_presets::builtin_approval_presets;
pub(super) use agere_utils_fs::AbsolutePathBuf;
pub(super) use assert_matches::assert_matches;
pub(super) use crossterm::event::KeyCode;
pub(super) use crossterm::event::KeyEvent;
pub(super) use crossterm::event::KeyModifiers;
pub(super) use insta::assert_snapshot;
pub(super) use serde_json::json;
#[cfg(target_os = "windows")]
pub(super) use serial_test::serial;
pub(super) use std::collections::BTreeMap;
pub(super) use std::collections::HashMap;
pub(super) use std::collections::HashSet;
pub(super) use std::path::PathBuf;
pub(super) use tempfile::NamedTempFile;
pub(super) use tempfile::tempdir;
pub(super) use tokio::sync::mpsc::error::TryRecvError;
pub(super) use tokio::sync::mpsc::unbounded_channel;
pub(super) use toml::Value as TomlValue;

pub(super) fn chatwidget_snapshot_dir() -> PathBuf {
    let snapshot_file = agere_utils_cargo_bin::find_resource!(
        "src/chatwidget/snapshots/agere_tui__chatwidget__tests__chatwidget_tall.snap"
    )
    .expect("snapshot file");
    snapshot_file
        .parent()
        .unwrap_or_else(|| panic!("snapshot file has no parent: {}", snapshot_file.display()))
        .to_path_buf()
}

macro_rules! assert_chatwidget_snapshot {
    ($name:expr, $value:expr $(,)?) => {{
        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);
        settings.set_snapshot_path(crate::chatwidget::tests::chatwidget_snapshot_dir());
        settings.bind(|| {
            insta::assert_snapshot!(format!("agere_tui__chatwidget__tests__{}", $name), $value);
        });
    }};
    ($name:expr, $value:expr, @$snapshot:literal $(,)?) => {{
        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);
        settings.set_snapshot_path(crate::chatwidget::tests::chatwidget_snapshot_dir());
        settings.bind(|| {
            insta::assert_snapshot!(
                format!("agere_tui__chatwidget__tests__{}", $name),
                &($value),
                @$snapshot
            );
        });
    }};
}

mod app_server;
mod approval_requests;
mod background_events;
mod composer_submission;
mod exec_flow;
mod goal_menu;
mod guardian;
mod helpers;
mod history_replay;
mod mcp_startup;
mod permissions;
mod plan_mode;
mod popups_and_settings;
mod review_mode;
mod side;
mod slash_commands;
mod status_and_layout;
mod status_command_tests;
mod status_surface_previews;
mod terminal_title;

pub(crate) use helpers::make_chatwidget_manual_with_sender;
pub(crate) use helpers::set_chatgpt_auth;
pub(crate) use helpers::set_fast_mode_test_catalog;
pub(super) use helpers::*;
