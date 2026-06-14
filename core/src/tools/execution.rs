//! Shared approval and managed-access orchestration traits used by tool runtimes.
//!
//! Consolidates the approval flow primitives (`ApprovalDecision`, `ApprovalStore`,
//! `ApprovalCtx`, `Approvable`) together with execution orchestration traits
//! and helpers (`ToolRuntime`, etc.).

use crate::execution::ExecutionPermissionLevel;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::state::SessionServices;
use crate::tools::hook_names::HookToolName;
use crate::tools::network_approval::NetworkApprovalSpec;
use agere_network_proxy::NetworkProxy;
use agere_protocol::approvals::ExecPolicyAmendment;
use agere_protocol::approvals::NetworkApprovalContext;
use agere_protocol::error::AgereErr;
use agere_protocol::permissions::FileSystemAccessLevel;
use agere_protocol::permissions::FileSystemAccessPolicy;
use agere_protocol::protocol::AskForApproval;
use agere_protocol::protocol::ReviewDecision;
use futures::Future;
use futures::future::BoxFuture;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

#[derive(Clone, Default, Debug)]
pub(crate) struct ApprovalStore {
    // Store serialized keys for generic caching across requests.
    map: HashMap<String, ReviewDecision>,
}

impl ApprovalStore {
    pub fn get<K>(&self, key: &K) -> Option<ReviewDecision>
    where
        K: Serialize,
    {
        let s = serde_json::to_string(key).ok()?;
        self.map.get(&s).cloned()
    }

    pub fn put<K>(&mut self, key: K, value: ReviewDecision)
    where
        K: Serialize,
    {
        if let Ok(s) = serde_json::to_string(&key) {
            self.map.insert(s, value);
        }
    }
}

/// Takes a vector of approval keys and returns a ReviewDecision.
/// There will be one key in most cases, but apply_patch can modify multiple files at once.
///
/// - If all keys are already approved for session, we skip prompting.
/// - If the user approves for session, we store the decision for each key individually
///   so future requests touching any subset can also skip prompting.
pub(crate) async fn with_cached_approval<K, F, Fut>(
    services: &SessionServices,
    // Name of the tool, used for metrics collection.
    tool_name: &str,
    keys: Vec<K>,
    fetch: F,
) -> ReviewDecision
where
    K: Serialize,
    F: FnOnce() -> Fut,
    Fut: Future<Output = ReviewDecision>,
{
    // To be defensive here, don't bother with checking the cache if keys are empty.
    if keys.is_empty() {
        return fetch().await;
    }

    let already_approved = {
        let store = services.tool_approvals.lock().await;
        keys.iter()
            .all(|key| matches!(store.get(key), Some(ReviewDecision::ApprovedForSession)))
    };

    if already_approved {
        return ReviewDecision::ApprovedForSession;
    }

    let decision = fetch().await;

    services.session_telemetry.counter(
        "agere.approval.requested",
        /*inc*/ 1,
        &[
            ("tool", tool_name),
            ("approved", decision.to_opaque_string()),
        ],
    );

    if matches!(decision, ReviewDecision::ApprovedForSession) {
        let mut store = services.tool_approvals.lock().await;
        for key in keys {
            store.put(key, ReviewDecision::ApprovedForSession);
        }
    }

    decision
}

#[derive(Clone)]
pub(crate) struct ApprovalCtx<'a> {
    pub session: &'a Arc<Session>,
    pub turn: &'a Arc<TurnContext>,
    pub call_id: &'a str,
    /// Guardian review lifecycle ID for this approval, when guardian is reviewing it.
    ///
    /// This is separate from `call_id`: `call_id` identifies the tool item under
    /// review, while this ID identifies the review itself. Keeping both lets
    /// denial handling, overrides, and app-server notifications refer to the
    /// review without overloading the tool call ID as a review ID.
    pub guardian_review_id: Option<String>,
    pub retry_reason: Option<String>,
    pub network_approval_context: Option<NetworkApprovalContext>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PermissionRequestPayload {
    pub tool_name: HookToolName,
    pub tool_input: serde_json::Value,
}

impl PermissionRequestPayload {
    pub(crate) fn bash(command: String, description: Option<String>) -> Self {
        let mut tool_input = serde_json::Map::new();
        tool_input.insert("command".to_string(), serde_json::Value::String(command));
        if let Some(description) = description {
            tool_input.insert(
                "description".to_string(),
                serde_json::Value::String(description),
            );
        }

        Self {
            tool_name: HookToolName::bash(),
            tool_input: serde_json::Value::Object(tool_input),
        }
    }
}

/// Specifies what tool orchestrator should do with a given tool call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExecApprovalRequirement {
    /// No approval required for this tool call.
    Skip {
        /// Proposed execpolicy amendment to skip future approvals for similar commands
        /// Only applies if the command fails to run and prompts the user to retry.
        proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    },
    /// Approval required for this tool call.
    NeedsApproval {
        reason: Option<String>,
        /// Proposed execpolicy amendment to skip future approvals for similar commands
        /// See core/src/exec_policy.rs for more details on how proposed_execpolicy_amendment is determined.
        proposed_execpolicy_amendment: Option<ExecPolicyAmendment>,
    },
    /// Execution forbidden for this tool call.
    Forbidden { reason: String },
}

impl ExecApprovalRequirement {
    pub fn proposed_execpolicy_amendment(&self) -> Option<&ExecPolicyAmendment> {
        match self {
            Self::NeedsApproval {
                proposed_execpolicy_amendment: Some(prefix),
                ..
            } => Some(prefix),
            Self::Skip {
                proposed_execpolicy_amendment: Some(prefix),
                ..
            } => Some(prefix),
            _ => None,
        }
    }
}

/// - Never, OnFailure: do not ask
/// - OnRequest: ask unless filesystem access is unrestricted
/// - Granular: ask unless filesystem access is unrestricted, but auto-reject
///   when granular access-escalation approval is disabled.
/// - UnlessTrusted: always ask
pub(crate) fn default_exec_approval_requirement(
    policy: AskForApproval,
    file_system_access_policy: &FileSystemAccessPolicy,
) -> ExecApprovalRequirement {
    let needs_approval = match policy {
        AskForApproval::Never | AskForApproval::OnFailure => false,
        AskForApproval::OnRequest | AskForApproval::Granular(_) => {
            matches!(
                file_system_access_policy.kind,
                FileSystemAccessLevel::Restricted
            )
        }
        AskForApproval::UnlessTrusted => true,
    };

    if needs_approval
        && matches!(
            policy,
            AskForApproval::Granular(granular_config)
                if !granular_config.allows_access_approval()
        )
    {
        ExecApprovalRequirement::Forbidden {
            reason: "approval policy disallowed access-escalation approval prompt".to_string(),
        }
    } else if needs_approval {
        ExecApprovalRequirement::NeedsApproval {
            reason: None,
            proposed_execpolicy_amendment: None,
        }
    } else {
        ExecApprovalRequirement::Skip {
            proposed_execpolicy_amendment: None,
        }
    }
}

pub(crate) fn managed_network_for_permission_level(
    network: Option<&NetworkProxy>,
    permission_level: ExecutionPermissionLevel,
) -> Option<&NetworkProxy> {
    if permission_level.requires_escalated_permissions() {
        None
    } else {
        network
    }
}

pub(crate) trait Approvable<Req> {
    type ApprovalKey: Hash + Eq + Clone + Debug + Serialize;

    // In most cases (shell, unified_exec), a request will have a single approval key.
    //
    // However, apply_patch needs session "Allow, don't ask again" semantics that
    // apply to multiple atomic targets (e.g., apply_patch approves per file path). Returning
    // a list of keys lets the runtime treat the request as approved-for-session only if
    // *all* keys are already approved, while still caching approvals per-key so future
    // requests touching a subset can be auto-approved.
    fn approval_keys(&self, req: &Req) -> Vec<Self::ApprovalKey>;

    fn should_bypass_approval(&self, policy: AskForApproval, already_approved: bool) -> bool {
        if already_approved {
            // We do not ask one more time
            return true;
        }
        matches!(policy, AskForApproval::Never)
    }

    /// Return `Some(_)` to specify a custom exec approval requirement, or `None`
    /// to fall back to policy-based default.
    fn exec_approval_requirement(&self, _req: &Req) -> Option<ExecApprovalRequirement> {
        None
    }

    /// Return hook input for approval-time policy hooks when this runtime wants
    /// hook evaluation to run before guardian or user approval.
    fn permission_request_payload(&self, _req: &Req) -> Option<PermissionRequestPayload> {
        None
    }

    /// Decide we can request an approval for less restricted (escalated) execution.
    fn wants_no_access_approval(&self, policy: AskForApproval) -> bool {
        match policy {
            AskForApproval::OnFailure => true,
            AskForApproval::UnlessTrusted => true,
            AskForApproval::Never => false,
            AskForApproval::OnRequest => false,
            AskForApproval::Granular(granular_config) => granular_config.access_approval,
        }
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a Req,
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision>;
}

pub(crate) struct ToolCtx {
    pub session: Arc<Session>,
    pub turn: Arc<TurnContext>,
    pub call_id: String,
    pub tool_name: String,
}

#[derive(Debug)]
pub(crate) enum ToolError {
    Rejected(String),
    Agere(AgereErr),
}

pub(crate) trait ToolRuntime<Req, Out>: Approvable<Req> {
    fn escalate_on_failure(&self) -> bool {
        true
    }

    fn network_approval_spec(&self, _req: &Req, _ctx: &ToolCtx) -> Option<NetworkApprovalSpec> {
        None
    }

    async fn run(&mut self, req: &Req, ctx: &ToolCtx) -> Result<Out, ToolError>;
}
