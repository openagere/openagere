/// Unix escalation stub — removed with legacy exec escalation plumbing.
use super::ShellRequest;
use crate::execution::ExecRequest;
use crate::tools::execution::ToolCtx;
use crate::tools::execution::ToolError;
use agere_protocol::exec_output::ExecToolCallOutput;
use std::path::Path;

pub(crate) struct PreparedUnifiedExecZshFork {
    pub(crate) exec_request: ExecRequest,
}

/// Stub: zsh-fork unix escalation has been removed. Always returns `Ok(None)`.
pub(super) async fn try_run_zsh_fork(
    _req: &ShellRequest,
    _ctx: &ToolCtx,
    _command: &[String],
) -> Result<Option<ExecToolCallOutput>, ToolError> {
    tracing::warn!("ZshFork unix escalation has been removed");
    Ok(None)
}

/// Stub: always returns `Ok(None)`.
pub(crate) async fn prepare_unified_exec_zsh_fork(
    _req: &crate::tools::runtimes::unified_exec::UnifiedExecRequest,
    _ctx: &ToolCtx,
    _exec_request: ExecRequest,
    _shell_zsh_path: &Path,
    _main_execve_wrapper_exe: &Path,
) -> Result<Option<PreparedUnifiedExecZshFork>, ToolError> {
    tracing::warn!("ZshFork unix escalation has been removed");
    Ok(None)
}
