pub mod installed_marketplaces;
pub mod loader;
pub mod manifest;
pub mod marketplace;
pub mod marketplace_add;
pub mod marketplace_remove;
pub mod marketplace_upgrade;
pub mod remote;
pub mod remote_bundle;
pub mod remote_legacy;
pub mod startup_sync;
pub mod store;
pub mod toggles;

pub const OPENAGERE_CURATED_MARKETPLACE_NAME: &str = "openagere-curated";
pub const OPENAGERE_BUNDLED_MARKETPLACE_NAME: &str = "openagere-bundled";
