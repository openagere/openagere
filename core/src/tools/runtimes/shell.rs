/*
Runtime: shell

Executes shell requests under the orchestrator: asks for approval when needed,
builds an ExecRequest directly, and runs it.
*/
#[cfg(unix)]
pub(crate) mod unix_escalation;
pub(crate) mod zsh_fork_backend;

use crate::access_policy_transforms::effective_permission_profile;
use crate::command_canonicalization::canonicalize_command_for_approval;
use crate::exec::ExecCapturePolicy;
use crate::execution::ExecutionPermissionLevel;
use crate::execution::execute_exec_request;
use crate::guardian::GuardianApprovalRequest;
use crate::guardian::GuardianNetworkAccessTrigger;
use crate::guardian::review_approval_request;
use crate::shell::ShellType;
use crate::tools::execution::Approvable;
use crate::tools::execution::ApprovalCtx;
use crate::tools::execution::ExecApprovalRequirement;
use crate::tools::execution::PermissionRequestPayload;
use crate::tools::execution::ToolCtx;
use crate::tools::execution::ToolError;
use crate::tools::execution::ToolRuntime;
use crate::tools::execution::managed_network_for_permission_level;
use crate::tools::execution::with_cached_approval;
use crate::tools::network_approval::NetworkApprovalMode;
use crate::tools::network_approval::NetworkApprovalSpec;
use crate::tools::runtimes::exec_env_for_permission_level;
use crate::tools::runtimes::maybe_wrap_shell_lc_with_snapshot;
use agere_network_proxy::NetworkProxy;
use agere_protocol::exec_output::ExecToolCallOutput;
use agere_protocol::models::AdditionalPermissionProfile;
use agere_protocol::protocol::ReviewDecision;
use agere_shell_command::powershell::prefix_powershell_script_with_utf8;
use agere_utils_fs::AbsolutePathBuf;
use futures::future::BoxFuture;
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct ShellRequest {
    pub command: Vec<String>,
    pub hook_command: String,
    pub cwd: AbsolutePathBuf,
    pub timeout_ms: Option<u64>,
    pub env: HashMap<String, String>,
    pub explicit_env_overrides: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub permission_level: ExecutionPermissionLevel,
    pub additional_permissions: Option<AdditionalPermissionProfile>,
    #[cfg(unix)]
    pub additional_permissions_preapproved: bool,
    pub justification: Option<String>,
    pub exec_approval_requirement: ExecApprovalRequirement,
}

/// Selects `ShellRuntime` behavior for different callers.
///
/// Note: `Generic` is not the same as `ShellCommandClassic`.
/// `Generic` means "no `shell_command`-specific backend behavior" (used by the
/// generic `shell` tool path). The `ShellCommand*` variants are only for the
/// `shell_command` tool family.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum ShellRuntimeBackend {
    /// Tool-agnostic/default runtime path.
    ///
    /// Uses the normal `ShellRuntime` execution flow without enabling any
    /// `shell_command`-specific backend selection.
    #[default]
    Generic,
    /// Legacy backend for the `shell_command` tool.
    ///
    /// Keeps `shell_command` on the standard shell runtime flow without the
    /// zsh-fork shell-escalation adapter.
    ShellCommandClassic,
    /// zsh-fork backend for the `shell_command` tool.
    ///
    /// On Unix, attempts to run via the zsh-fork + `agere-shell-escalation`
    /// adapter, with fallback to the standard shell runtime flow if
    /// prerequisites are not met.
    ShellCommandZshFork,
}

#[derive(Default)]
pub struct ShellRuntime {
    backend: ShellRuntimeBackend,
}

#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ApprovalKey {
    command: Vec<String>,
    cwd: AbsolutePathBuf,
    permission_level: ExecutionPermissionLevel,
    additional_permissions: Option<AdditionalPermissionProfile>,
}

impl ShellRuntime {
    pub fn new() -> Self {
        Self {
            backend: ShellRuntimeBackend::Generic,
        }
    }

    pub(crate) fn for_shell_command(backend: ShellRuntimeBackend) -> Self {
        Self { backend }
    }

    fn stdout_stream(ctx: &ToolCtx) -> Option<crate::exec::StdoutStream> {
        Some(crate::exec::StdoutStream {
            sub_id: ctx.turn.sub_id.clone(),
            call_id: ctx.call_id.clone(),
            tx_event: ctx.session.get_tx_event(),
        })
    }
}

impl Approvable<ShellRequest> for ShellRuntime {
    type ApprovalKey = ApprovalKey;

    fn approval_keys(&self, req: &ShellRequest) -> Vec<Self::ApprovalKey> {
        vec![ApprovalKey {
            command: canonicalize_command_for_approval(&req.command),
            cwd: req.cwd.clone(),
            permission_level: req.permission_level,
            additional_permissions: req.additional_permissions.clone(),
        }]
    }

    fn start_approval_async<'a>(
        &'a mut self,
        req: &'a ShellRequest,
        ctx: ApprovalCtx<'a>,
    ) -> BoxFuture<'a, ReviewDecision> {
        let keys = self.approval_keys(req);
        let command = req.command.clone();
        let cwd = req.cwd.clone();
        let retry_reason = ctx.retry_reason.clone();
        let reason = retry_reason.clone().or_else(|| req.justification.clone());
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        let guardian_review_id = ctx.guardian_review_id.clone();
        Box::pin(async move {
            if let Some(review_id) = guardian_review_id {
                return review_approval_request(
                    session,
                    turn,
                    review_id,
                    GuardianApprovalRequest::Shell {
                        id: call_id,
                        command,
                        cwd: cwd.clone(),
                        permission_level: req.permission_level,
                        additional_permissions: req.additional_permissions.clone(),
                        justification: req.justification.clone(),
                    },
                    retry_reason,
                )
                .await;
            }
            with_cached_approval(&session.services, "shell", keys, move || async move {
                let available_decisions = None;
                session
                    .request_command_approval(
                        turn,
                        call_id,
                        /*approval_id*/ None,
                        command,
                        cwd,
                        reason,
                        ctx.network_approval_context.clone(),
                        req.exec_approval_requirement
                            .proposed_execpolicy_amendment()
                            .cloned(),
                        req.additional_permissions.clone(),
                        available_decisions,
                    )
                    .await
            })
            .await
        })
    }

    fn exec_approval_requirement(&self, req: &ShellRequest) -> Option<ExecApprovalRequirement> {
        Some(req.exec_approval_requirement.clone())
    }

    fn permission_request_payload(&self, req: &ShellRequest) -> Option<PermissionRequestPayload> {
        Some(PermissionRequestPayload::bash(
            req.hook_command.clone(),
            req.justification.clone(),
        ))
    }
}

impl ToolRuntime<ShellRequest, ExecToolCallOutput> for ShellRuntime {
    fn network_approval_spec(
        &self,
        req: &ShellRequest,
        ctx: &ToolCtx,
    ) -> Option<NetworkApprovalSpec> {
        let network =
            managed_network_for_permission_level(req.network.as_ref(), req.permission_level)?;
        Some(NetworkApprovalSpec {
            network: Some(network.clone()),
            mode: NetworkApprovalMode::Immediate,
            trigger: GuardianNetworkAccessTrigger {
                call_id: ctx.call_id.clone(),
                tool_name: ctx.tool_name.clone(),
                command: req.command.clone(),
                cwd: req.cwd.clone(),
                permission_level: req.permission_level,
                additional_permissions: req.additional_permissions.clone(),
                justification: req.justification.clone(),
                tty: None,
            },
            command: req.hook_command.clone(),
        })
    }

    async fn run(
        &mut self,
        req: &ShellRequest,
        ctx: &ToolCtx,
    ) -> Result<ExecToolCallOutput, ToolError> {
        let session_shell = ctx.session.user_shell();
        let managed_network =
            managed_network_for_permission_level(req.network.as_ref(), req.permission_level);
        let env = exec_env_for_permission_level(&req.env, req.permission_level);

        // Add network-disabled marker when outbound network is restricted for this attempt.
        let mut env = env;
        let permission_profile = effective_permission_profile(
            &ctx.turn.permission_profile,
            req.additional_permissions.as_ref(),
        );
        let (_, network_access_policy) = permission_profile.to_runtime_permissions();
        if !network_access_policy.is_enabled() {
            env.insert(
                crate::spawn::AGERE_NETWORK_DISABLED_ENV_VAR.to_string(),
                "1".to_string(),
            );
        }

        let command = maybe_wrap_shell_lc_with_snapshot(
            &req.command,
            session_shell.as_ref(),
            &req.cwd,
            &req.explicit_env_overrides,
            &env,
        );
        let command = if matches!(session_shell.shell_type, ShellType::PowerShell) {
            prefix_powershell_script_with_utf8(&command)
        } else {
            command
        };

        if self.backend == ShellRuntimeBackend::ShellCommandZshFork {
            tracing::warn!(
                "ZshFork backend specified, but it has been removed; falling back to normal execution",
            );
        }

        let exec_request = crate::execution::ExecRequest::new(
            command,
            req.cwd.clone(),
            env,
            managed_network.cloned(),
            req.timeout_ms.into(),
            ExecCapturePolicy::ShellTool,
            permission_profile,
            None, // arg0
        );
        let out = execute_exec_request(exec_request, Self::stdout_stream(ctx), None)
            .await
            .map_err(ToolError::Agere)?;
        Ok(out)
    }
}
