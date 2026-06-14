//! Apply Patch runtime: executes verified patches under the orchestrator.
//!
//! Assumes `apply_patch` verification/approval happened upstream. Reuses the
//! selected turn environment filesystem for both local and remote turns, with
//! filesystem limits enforced by the explicit filesystem access context when present.
use crate::exec::is_likely_policy_denial;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::review_approval_request;
use crate::tools::execution::Approvable;
use crate::tools::execution::ApprovalCtx;
use crate::tools::execution::ExecApprovalRequirement;
use crate::tools::execution::PermissionRequestPayload;
use crate::tools::execution::ToolCtx;
use crate::tools::execution::ToolError;
use crate::tools::execution::ToolRuntime;
use crate::tools::execution::with_cached_approval;
use crate::tools::hook_names::HookToolName;
use agere_apply_patch::ApplyPatchAction;
use agere_exec_server::FileSystemAccessContext;
use agere_protocol::error::AgereErr;
use agere_protocol::error::ExecErr;
use agere_protocol::exec_output::ExecToolCallOutput;
use agere_protocol::exec_output::StreamOutput;
use agere_protocol::models::AdditionalPermissionProfile;
use agere_protocol::protocol::AskForApproval;
use agere_protocol::protocol::Event;
use agere_protocol::protocol::EventMsg;
use agere_protocol::protocol::ExecCommandOutputDeltaEvent;
use agere_protocol::protocol::ExecOutputStream;
use agere_protocol::protocol::FileChange;
use agere_protocol::protocol::ReviewDecision;
use agere_utils_fs::AbsolutePathBuf;
use futures::future::BoxFuture;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug)]
pub struct ApplyPatchRequest {
    pub action: ApplyPatchAction,
    pub file_paths: Vec<AbsolutePathBuf>,
    pub changes: std::collections::HashMap<PathBuf, FileChange>,
    pub exec_approval_requirement: ExecApprovalRequirement,
    pub additional_permissions: Option<AdditionalPermissionProfile>,
    pub permissions_preapproved: bool,
}

#[derive(Default)]
pub struct ApplyPatchRuntime;

impl ApplyPatchRuntime {
    pub fn new() -> Self {
        Self
    }

    fn build_guardian_review_request(
        req: &ApplyPatchRequest,
        call_id: &str,
    ) -> GuardianApprovalRequest {
        GuardianApprovalRequest::ApplyPatch {
            id: call_id.to_string(),
            cwd: req.action.cwd.clone(),
            files: req.file_paths.clone(),
            patch: req.action.patch.clone(),
        }
    }

    fn file_system_access_context_for_attempt(
        _req: &ApplyPatchRequest,
    ) -> Option<FileSystemAccessContext> {
        // No separate apply_patch filesystem context layer — always None.
        None
    }

    async fn emit_output_delta(ctx: &ToolCtx, stream: ExecOutputStream, chunk: &[u8]) {
        if chunk.is_empty() {
            return;
        }

        let event = Event {
            id: ctx.turn.sub_id.clone(),
            msg: EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                call_id: ctx.call_id.clone(),
                stream,
                chunk: chunk.to_vec(),
            }),
        };
        let _ = ctx.session.get_tx_event().send(event).await;
    }
}

impl Approvable<ApplyPatchRequest> for ApplyPatchRuntime {
    type ApprovalKey = AbsolutePathBuf;

    fn approval_keys(&self, req: &ApplyPatchRequest) -> Vec<Self::ApprovalKey> {
        req.file_paths.clone()
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a ApplyPatchRequest,
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        let retry_reason = ctx.retry_reason.clone();
        let approval_keys = self.approval_keys(req);
        let changes = req.changes.clone();
        let guardian_review_id = ctx.guardian_review_id.clone();
        Box::pin(async move {
            if let Some(review_id) = guardian_review_id {
                let action = ApplyPatchRuntime::build_guardian_review_request(req, ctx.call_id);
                return review_approval_request(session, turn, review_id, action, retry_reason)
                    .await;
            }
            if req.permissions_preapproved && retry_reason.is_none() {
                return ReviewDecision::Approved;
            }
            if let Some(reason) = retry_reason {
                let rx_approve = session
                    .request_patch_approval(
                        turn,
                        call_id,
                        changes.clone(),
                        Some(reason),
                        /*grant_root*/ None,
                    )
                    .await;
                return rx_approve.await.unwrap_or_default();
            }

            with_cached_approval(
                &session.services,
                "apply_patch",
                approval_keys,
                || async move {
                    let rx_approve = session
                        .request_patch_approval(
                            turn, call_id, changes, /*reason*/ None, /*grant_root*/ None,
                        )
                        .await;
                    rx_approve.await.unwrap_or_default()
                },
            )
            .await
        })
    }

    fn wants_no_access_approval(&self, policy: AskForApproval) -> bool {
        match policy {
            AskForApproval::Never => false,
            AskForApproval::Granular(granular_config) => granular_config.allows_access_approval(),
            AskForApproval::OnFailure => true,
            AskForApproval::OnRequest => true,
            AskForApproval::UnlessTrusted => true,
        }
    }

    // apply_patch approvals are decided upstream by assess_patch_safety.
    //
    // This override ensures the orchestrator runs the patch approval flow when required instead
    // of falling back to the global exec approval policy.
    fn exec_approval_requirement(
        &self,
        req: &ApplyPatchRequest,
    ) -> Option<ExecApprovalRequirement> {
        Some(req.exec_approval_requirement.clone())
    }

    fn permission_request_payload(
        &self,
        req: &ApplyPatchRequest,
    ) -> Option<PermissionRequestPayload> {
        Some(PermissionRequestPayload {
            tool_name: HookToolName::apply_patch(),
            tool_input: serde_json::json!({ "command": req.action.patch }),
        })
    }
}

impl ToolRuntime<ApplyPatchRequest, ExecToolCallOutput> for ApplyPatchRuntime {
    async fn run(
        &mut self,
        req: &ApplyPatchRequest,
        ctx: &ToolCtx,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let environment = ctx.turn.environment.as_ref().ok_or_else(|| {
            ToolError::Rejected("apply_patch is unavailable in this session".to_string())
        })?;
        let started_at = Instant::now();
        let fs = environment.get_filesystem();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let result = agere_apply_patch::apply_patch(
            &req.action.patch,
            &req.action.cwd,
            &mut stdout,
            &mut stderr,
            fs.as_ref(),
        )
        .await;
        let stdout = String::from_utf8_lossy(&stdout).into_owned();
        let stderr = String::from_utf8_lossy(&stderr).into_owned();
        Self::emit_output_delta(ctx, ExecOutputStream::Stdout, stdout.as_bytes()).await;
        Self::emit_output_delta(ctx, ExecOutputStream::Stderr, stderr.as_bytes()).await;
        let exit_code = if result.is_ok() { 0 } else { 1 };
        let output = ExecToolCallOutput {
            exit_code,
            stdout: StreamOutput::new(stdout.clone()),
            stderr: StreamOutput::new(stderr.clone()),
            aggregated_output: StreamOutput::new(format!("{stdout}{stderr}")),
            duration: started_at.elapsed(),
            timed_out: false,
        };
        if result.is_err() && is_likely_policy_denial(&output) {
            return Err(ToolError::Agere(AgereErr::Exec(ExecErr::Denied {
                output: Box::new(output),
                network_policy_decision: None,
            })));
        }
        Ok(output)
    }
}

#[cfg(test)]
#[path = "apply_patch_tests.rs"]
mod tests;
