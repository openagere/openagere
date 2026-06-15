/*
Runtime: unified exec

Handles approval + managed exec orchestration for unified exec requests, delegating to
the process manager to spawn PTYs once an ExecRequest is prepared.
*/
use crate::access_policy_transforms::effective_permission_profile;
use crate::command_canonicalization::canonicalize_command_for_approval;
use crate::exec::ExecCapturePolicy;
use crate::exec::ExecExpiration;
use crate::execution::ExecServerEnvConfig;
use crate::execution::ExecutionPermissionLevel;
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
use crate::unified_exec::NoopSpawnLifecycle;
use crate::unified_exec::UnifiedExecError;
use crate::unified_exec::UnifiedExecProcess;
use crate::unified_exec::UnifiedExecProcessManager;
use agere_network_proxy::NetworkProxy;
use agere_protocol::error::AgereErr;
use agere_protocol::error::ExecErr;
use agere_protocol::models::AdditionalPermissionProfile;
use agere_protocol::protocol::ReviewDecision;
use agere_shell_command::powershell::prefix_powershell_script_with_utf8;
use agere_tools::UnifiedExecShellMode;
use agere_utils_fs::AbsolutePathBuf;
use futures::future::BoxFuture;
use std::collections::HashMap;

/// Request payload used by the unified-exec runtime after approvals and
/// permission-level preferences have been resolved for the current turn.
#[derive(Clone, Debug)]
pub struct UnifiedExecRequest {
    pub command: Vec<String>,
    pub hook_command: String,
    pub process_id: i32,
    pub cwd: AbsolutePathBuf,
    pub env: HashMap<String, String>,
    pub exec_server_env_config: Option<ExecServerEnvConfig>,
    pub explicit_env_overrides: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub tty: bool,
    pub permission_level: ExecutionPermissionLevel,
    pub additional_permissions: Option<AdditionalPermissionProfile>,
    #[cfg(unix)]
    pub additional_permissions_preapproved: bool,
    pub justification: Option<String>,
    pub exec_approval_requirement: ExecApprovalRequirement,
}

/// Cache key for approval decisions that can be reused across equivalent
/// unified-exec launches.
#[derive(serde::Serialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct UnifiedExecApprovalKey {
    pub command: Vec<String>,
    pub cwd: AbsolutePathBuf,
    pub tty: bool,
    pub permission_level: ExecutionPermissionLevel,
    pub additional_permissions: Option<AdditionalPermissionProfile>,
}

/// Runtime adapter that keeps policy and exec orchestration on the
/// unified-exec side while delegating process startup to the manager.
pub struct UnifiedExecRuntime<'a> {
    manager: &'a UnifiedExecProcessManager,
    shell_mode: UnifiedExecShellMode,
}

impl<'a> UnifiedExecRuntime<'a> {
    /// Creates a runtime bound to the shared unified-exec process manager.
    pub fn new(manager: &'a UnifiedExecProcessManager, shell_mode: UnifiedExecShellMode) -> Self {
        Self {
            manager,
            shell_mode,
        }
    }
}

impl Approvable<UnifiedExecRequest> for UnifiedExecRuntime<'_> {
    type ApprovalKey = UnifiedExecApprovalKey;

    fn approval_keys(&self, req: &UnifiedExecRequest) -> Vec<Self::ApprovalKey> {
        vec![UnifiedExecApprovalKey {
            command: canonicalize_command_for_approval(&req.command),
            cwd: req.cwd.clone(),
            tty: req.tty,
            permission_level: req.permission_level,
            additional_permissions: req.additional_permissions.clone(),
        }]
    }

    fn start_approval_async<'b>(
        &'b mut self,
        req: &'b UnifiedExecRequest,
        ctx: ApprovalCtx<'b>,
    ) -> BoxFuture<'b, ReviewDecision> {
        let keys = self.approval_keys(req);
        let session = ctx.session;
        let turn = ctx.turn;
        let call_id = ctx.call_id.to_string();
        let command = req.command.clone();
        let cwd = req.cwd.clone();
        let retry_reason = ctx.retry_reason.clone();
        let reason = retry_reason.clone().or_else(|| req.justification.clone());
        let guardian_review_id = ctx.guardian_review_id.clone();
        Box::pin(async move {
            if let Some(review_id) = guardian_review_id {
                return review_approval_request(
                    session,
                    turn,
                    review_id,
                    GuardianApprovalRequest::ExecCommand {
                        id: call_id,
                        command,
                        cwd: cwd.clone(),
                        permission_level: req.permission_level,
                        additional_permissions: req.additional_permissions.clone(),
                        justification: req.justification.clone(),
                        tty: req.tty,
                    },
                    retry_reason,
                )
                .await;
            }
            with_cached_approval(&session.services, "unified_exec", keys, || async move {
                let available_decisions = None;
                session
                    .request_command_approval(
                        turn,
                        call_id,
                        /*approval_id*/ None,
                        command,
                        cwd.clone(),
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

    fn exec_approval_requirement(
        &self,
        req: &UnifiedExecRequest,
    ) -> Option<ExecApprovalRequirement> {
        Some(req.exec_approval_requirement.clone())
    }

    fn permission_request_payload(
        &self,
        req: &UnifiedExecRequest,
    ) -> Option<PermissionRequestPayload> {
        Some(PermissionRequestPayload::bash(
            req.hook_command.clone(),
            req.justification.clone(),
        ))
    }
}

impl<'a> ToolRuntime<UnifiedExecRequest, UnifiedExecProcess> for UnifiedExecRuntime<'a> {
    fn network_approval_spec(
        &self,
        req: &UnifiedExecRequest,
        ctx: &ToolCtx,
    ) -> Option<NetworkApprovalSpec> {
        let network =
            managed_network_for_permission_level(req.network.as_ref(), req.permission_level)?;
        Some(NetworkApprovalSpec {
            network: Some(network.clone()),
            mode: NetworkApprovalMode::Deferred,
            trigger: GuardianNetworkAccessTrigger {
                call_id: ctx.call_id.clone(),
                tool_name: ctx.tool_name.clone(),
                command: req.command.clone(),
                cwd: req.cwd.clone(),
                permission_level: req.permission_level,
                additional_permissions: req.additional_permissions.clone(),
                justification: req.justification.clone(),
                tty: Some(req.tty),
            },
            command: req.hook_command.clone(),
        })
    }

    async fn run(
        &mut self,
        req: &UnifiedExecRequest,
        ctx: &ToolCtx,
    ) -> Result<UnifiedExecProcess, ToolError> {
        let session_shell = ctx.session.user_shell();
        let managed_network =
            managed_network_for_permission_level(req.network.as_ref(), req.permission_level);
        let mut env = exec_env_for_permission_level(&req.env, req.permission_level);
        if let Some(network) = managed_network {
            network.apply_to_env(&mut env);
        }
        let environment_is_remote = ctx
            .turn
            .environment
            .as_ref()
            .is_some_and(|environment| environment.is_remote());
        let base_command = if environment_is_remote {
            req.command.to_vec()
        } else {
            maybe_wrap_shell_lc_with_snapshot(
                &req.command,
                session_shell.as_ref(),
                &req.cwd,
                &req.explicit_env_overrides,
                &env,
            )
        };
        let command = if matches!(session_shell.shell_type, ShellType::PowerShell) {
            prefix_powershell_script_with_utf8(&base_command)
        } else {
            base_command
        };

        if let UnifiedExecShellMode::ZshFork(_zsh_fork_config) = &self.shell_mode {
            tracing::warn!(
                "UnifiedExec ZshFork backend specified, but it has been removed; falling back to direct execution",
            );
        }

        // Build ExecRequest directly.
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

        let mut exec_request = crate::execution::ExecRequest::new(
            command,
            req.cwd.clone(),
            env,
            managed_network.cloned(),
            ExecExpiration::DefaultTimeout,
            ExecCapturePolicy::ShellTool,
            permission_profile,
            None, // arg0
        );
        exec_request.exec_server_env_config = req.exec_server_env_config.clone();

        let Some(environment) = ctx.turn.environment.as_ref() else {
            return Err(ToolError::Rejected(
                "exec_command is unavailable in this session".to_string(),
            ));
        };
        self.manager
            .open_session_with_exec_env(
                req.process_id,
                &exec_request,
                req.tty,
                Box::new(NoopSpawnLifecycle),
                environment.as_ref(),
            )
            .await
            .map_err(|err| match err {
                UnifiedExecError::PolicyDenied { output, .. } => {
                    ToolError::Agere(AgereErr::Exec(ExecErr::Denied {
                        output: Box::new(output),
                        network_policy_decision: None,
                    }))
                }
                other => ToolError::Rejected(other.to_string()),
            })
    }
}
