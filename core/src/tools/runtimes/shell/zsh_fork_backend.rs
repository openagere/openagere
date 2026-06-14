/// Zsh-fork backend stub — removed with legacy shell fork plumbing.
/// Kept as a placeholder module to avoid breaking module declarations.
use super::ShellRequest;
use crate::execution::ExecRequest;
use crate::tools::execution::ToolCtx;
use crate::tools::execution::ToolError;
use crate::tools::runtimes::unified_exec::UnifiedExecRequest;
use agere_protocol::exec_output::ExecToolCallOutput;
use agere_tools::ZshForkConfig;

pub(crate) struct PreparedUnifiedExecSpawn {
    pub(crate) exec_request: ExecRequest,
}

/// Stub: zsh-fork backend has been removed. Always returns `Ok(None)`.
pub(crate) async fn maybe_run_shell_command(
    _req: &ShellRequest,
    _ctx: &ToolCtx,
    _command: &[String],
) -> Result<Option<ExecToolCallOutput>, ToolError> {
    tracing::warn!("ZshFork backend has been removed");
    Ok(None)
}

/// Stub: zsh-fork backend has been removed. Always returns `Ok(None)`.
pub(crate) async fn maybe_prepare_unified_exec(
    _req: &UnifiedExecRequest,
    _ctx: &ToolCtx,
    _exec_request: ExecRequest,
    _zsh_fork_config: &ZshForkConfig,
) -> Result<Option<PreparedUnifiedExecSpawn>, ToolError> {
    tracing::warn!("ZshFork backend has been removed");
    Ok(None)
}
