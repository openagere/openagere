use agere_utils_fs::AbsolutePathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredMarketplaceUpgradeError {
    pub marketplace_name: String,
    pub message: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConfiguredMarketplaceUpgradeOutcome {
    pub selected_marketplaces: Vec<String>,
    pub upgraded_roots: Vec<AbsolutePathBuf>,
    pub errors: Vec<ConfiguredMarketplaceUpgradeError>,
}

impl ConfiguredMarketplaceUpgradeOutcome {
    pub fn all_succeeded(&self) -> bool {
        self.errors.is_empty()
    }
}

pub fn configured_git_marketplace_names(
    _config_layer_stack: &agere_config::ConfigLayerStack,
) -> Vec<String> {
    // Stub: no configured git marketplaces in this build.
    Vec::new()
}

pub fn upgrade_configured_git_marketplaces(
    _agere_home: &std::path::Path,
    _config_layer_stack: &agere_config::ConfigLayerStack,
    _marketplace_name: Option<&str>,
) -> ConfiguredMarketplaceUpgradeOutcome {
    // Stub: marketplace upgrade is not supported in this build.
    ConfiguredMarketplaceUpgradeOutcome {
        selected_marketplaces: Vec::new(),
        upgraded_roots: Vec::new(),
        errors: Vec::new(),
    }
}
