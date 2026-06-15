use crate::store::PluginInstallResult;
use agere_plugin::PluginId;
use agere_plugin::PluginIdError;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ValidatedRemotePluginBundle {
    pub plugin_id: PluginId,
    pub plugin_version: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RemotePluginBundleInstallError {
    #[error("backend did not return a release version for remote plugin `{remote_plugin_id}`")]
    MissingReleaseVersion { remote_plugin_id: String },

    #[error(
        "backend returned an invalid release version for remote plugin `{remote_plugin_id}`: {message}"
    )]
    InvalidReleaseVersion {
        remote_plugin_id: String,
        message: String,
    },

    #[error("backend did not return a download URL for remote plugin `{remote_plugin_id}`")]
    MissingBundleDownloadUrl { remote_plugin_id: String },

    #[error(
        "backend returned an invalid local plugin id for remote plugin `{remote_plugin_id}`: {source}"
    )]
    InvalidPluginId {
        remote_plugin_id: String,
        #[source]
        source: PluginIdError,
    },

    #[error("{0}")]
    InvalidBundle(String),

    #[error("{0}")]
    Store(#[from] super::store::PluginStoreError),

    #[error("remote plugin bundle access is not supported in this build")]
    Unsupported(String),
}

pub fn validate_remote_plugin_bundle(
    remote_plugin_id: &str,
    remote_marketplace_name: &str,
    plugin_name: &str,
    release_version: Option<&str>,
    bundle_download_url: Option<&str>,
) -> Result<ValidatedRemotePluginBundle, RemotePluginBundleInstallError> {
    let plugin_id = PluginId::new(plugin_name.to_string(), remote_marketplace_name.to_string())
        .map_err(|source| RemotePluginBundleInstallError::InvalidPluginId {
            remote_plugin_id: remote_plugin_id.to_string(),
            source,
        })?;
    let plugin_version = release_version
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .ok_or_else(|| RemotePluginBundleInstallError::MissingReleaseVersion {
            remote_plugin_id: remote_plugin_id.to_string(),
        })?
        .to_string();
    let _bundle_download_url = bundle_download_url
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .ok_or_else(
            || RemotePluginBundleInstallError::MissingBundleDownloadUrl {
                remote_plugin_id: remote_plugin_id.to_string(),
            },
        )?;
    Ok(ValidatedRemotePluginBundle {
        plugin_id,
        plugin_version,
    })
}

pub async fn download_and_install_remote_plugin_bundle(
    _agere_home: PathBuf,
    _bundle: ValidatedRemotePluginBundle,
) -> Result<PluginInstallResult, RemotePluginBundleInstallError> {
    Err(RemotePluginBundleInstallError::Unsupported(
        "remote plugin bundle install is not supported in this build".into(),
    ))
}
