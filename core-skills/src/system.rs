pub(crate) use agere_skills::install_system_skills;
pub(crate) use agere_skills::system_cache_root_dir;

use agere_utils_fs::AbsolutePathBuf;

pub(crate) fn uninstall_system_skills(agere_home: &AbsolutePathBuf) {
    let _ = std::fs::remove_dir_all(system_cache_root_dir(agere_home));
}
