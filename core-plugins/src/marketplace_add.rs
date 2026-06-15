use agere_utils_fs::AbsolutePathBuf;
use std::path::PathBuf;

mod source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceAddRequest {
    pub source: String,
    pub ref_name: Option<String>,
    pub sparse_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketplaceAddOutcome {
    pub marketplace_name: String,
    pub source_display: String,
    pub installed_root: AbsolutePathBuf,
    pub already_added: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum MarketplaceAddError {
    #[error("{0}")]
    InvalidRequest(String),
    #[error("{0}")]
    Internal(String),
}

pub async fn add_marketplace(
    _agere_home: PathBuf,
    _request: MarketplaceAddRequest,
) -> Result<MarketplaceAddOutcome, MarketplaceAddError> {
    Err(MarketplaceAddError::Internal(
        "marketplace add is not supported in this build".into(),
    ))
}

pub fn is_local_marketplace_source(
    _source: &str,
    _explicit_ref: Option<String>,
) -> Result<bool, MarketplaceAddError> {
    Err(MarketplaceAddError::Internal(
        "marketplace source inspection is not supported in this build".into(),
    ))
}
