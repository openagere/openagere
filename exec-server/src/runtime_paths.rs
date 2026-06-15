use std::path::PathBuf;

use agere_utils_fs::AbsolutePathBuf;

/// Expected `argv0` basename for the Linux helper alias when Agere re-invokes
/// itself (see the `agere-arg0` crate).
pub const AGERE_LINUX_HELPER_ARG0: &str = "agere-linux-helper";

/// Runtime paths needed by exec-server child processes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecServerRuntimePaths {
    /// Stable path to the Agere executable used to launch hidden helper modes.
    pub agere_self_exe: AbsolutePathBuf,
    /// Path to the Linux helper alias used when argv0 must re-enter Agere as
    /// [`AGERE_LINUX_HELPER_ARG0`].
    pub agere_linux_exe: Option<AbsolutePathBuf>,
}

impl ExecServerRuntimePaths {
    pub fn from_optional_paths(
        agere_self_exe: Option<PathBuf>,
        agere_linux_exe: Option<PathBuf>,
    ) -> std::io::Result<Self> {
        let agere_self_exe = agere_self_exe.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Agere executable path is not configured",
            )
        })?;
        Self::new(agere_self_exe, agere_linux_exe)
    }

    pub fn new(agere_self_exe: PathBuf, agere_linux_exe: Option<PathBuf>) -> std::io::Result<Self> {
        Ok(Self {
            agere_self_exe: absolute_path(agere_self_exe)?,
            agere_linux_exe: agere_linux_exe.map(absolute_path).transpose()?,
        })
    }
}

fn absolute_path(path: PathBuf) -> std::io::Result<AbsolutePathBuf> {
    AbsolutePathBuf::from_absolute_path(path.as_path())
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err))
}
