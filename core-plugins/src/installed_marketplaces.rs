use std::fs;
use std::path::Path;
use std::path::PathBuf;

use agere_utils_fs::AbsolutePathBuf;

use crate::marketplace::find_marketplace_manifest_path;

pub const INSTALLED_MARKETPLACES_DIR: &str = ".tmp/marketplaces";

pub fn marketplace_install_root(agere_home: &Path) -> PathBuf {
    agere_home.join(INSTALLED_MARKETPLACES_DIR)
}

pub fn installed_marketplace_roots_from_layer_stack(
    _config_layer_stack: &agere_config::ConfigLayerStack,
    agere_home: &Path,
) -> Vec<AbsolutePathBuf> {
    let install_root = marketplace_install_root(agere_home);
    let Ok(entries) = fs::read_dir(&install_root) else {
        return Vec::new();
    };

    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            find_marketplace_manifest_path(&path)?;
            AbsolutePathBuf::try_from(path).ok()
        })
        .collect()
}

pub fn resolve_configured_marketplace_root(
    _marketplace_name: &str,
    _marketplace: &toml::Value,
    default_install_root: &Path,
) -> Option<PathBuf> {
    // Stub: return the default install root for any marketplace.
    Some(default_install_root.join(_marketplace_name))
}
