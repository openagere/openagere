use crate::catalog_overlay::CatalogModel;
use agere_model_provider_info::WireApi;
use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use std::fs as std_fs;
use std::io;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs as tokio_fs;
use tracing::error;
use tracing::info;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WireApiCatalog {
    pub(crate) etag: Option<String>,
    pub(crate) catalog_version: Option<String>,
    pub(crate) models: Vec<CatalogModel>,
}

#[derive(Clone, Debug)]
pub(crate) struct WireApiCatalogCache {
    agere_home: PathBuf,
    ttl: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireApiCatalogEnvelope {
    wire_api: WireApi,
    fetched_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    client_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    catalog_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    etag: Option<String>,
    models: Vec<CatalogModel>,
}

impl WireApiCatalogCache {
    pub(crate) fn new(agere_home: PathBuf, ttl: Duration) -> Self {
        Self { agere_home, ttl }
    }

    pub(crate) fn path_for(&self, wire_api: WireApi) -> PathBuf {
        self.agere_home
            .join("model_catalog")
            .join(format!("{wire_api}.json"))
    }

    pub(crate) async fn load_fresh(&self, wire_api: WireApi) -> Option<WireApiCatalog> {
        let envelope = self.load_envelope(wire_api).await?;
        if !matches_current_client_version(envelope.client_version.as_deref()) {
            return None;
        }
        if !is_fresh(envelope.fetched_at, self.ttl) {
            return None;
        }
        Some(envelope.into_catalog())
    }

    pub(crate) fn load_fresh_sync(&self, wire_api: WireApi) -> Option<WireApiCatalog> {
        let envelope = self.load_envelope_sync(wire_api)?;
        if !matches_current_client_version(envelope.client_version.as_deref()) {
            return None;
        }
        if !is_fresh(envelope.fetched_at, self.ttl) {
            return None;
        }
        Some(envelope.into_catalog())
    }

    pub(crate) async fn load_stale(&self, wire_api: WireApi) -> Option<WireApiCatalog> {
        let envelope = self.load_envelope(wire_api).await?;
        if !matches_current_client_version(envelope.client_version.as_deref()) {
            return None;
        }
        Some(envelope.into_catalog())
    }

    pub(crate) fn load_stale_sync(&self, wire_api: WireApi) -> Option<WireApiCatalog> {
        let envelope = self.load_envelope_sync(wire_api)?;
        if !matches_current_client_version(envelope.client_version.as_deref()) {
            return None;
        }
        Some(envelope.into_catalog())
    }

    pub(crate) async fn persist<M>(
        &self,
        wire_api: WireApi,
        models: &[M],
        etag: Option<String>,
        client_version: String,
        catalog_version: Option<String>,
    ) where
        M: Clone,
        CatalogModel: From<M>,
    {
        let envelope = WireApiCatalogEnvelope {
            wire_api,
            fetched_at: Utc::now(),
            client_version: Some(client_version),
            catalog_version,
            etag,
            models: models.iter().cloned().map(CatalogModel::from).collect(),
        };
        if let Err(err) = self.save_envelope(&envelope).await {
            error!("failed to write wire api model catalog cache: {err}");
        }
    }

    async fn load_envelope(&self, wire_api: WireApi) -> Option<WireApiCatalogEnvelope> {
        let path = self.path_for(wire_api);
        let contents = match tokio_fs::read(&path).await {
            Ok(contents) => contents,
            Err(err) if err.kind() == ErrorKind::NotFound => return None,
            Err(err) => {
                error!(
                    "failed to read wire api catalog cache {}: {err}",
                    path.display()
                );
                return None;
            }
        };
        let envelope = match serde_json::from_slice::<WireApiCatalogEnvelope>(&contents) {
            Ok(envelope) => envelope,
            Err(err) => {
                error!(
                    "failed to parse wire api catalog cache {}: {err}",
                    path.display()
                );
                return None;
            }
        };
        if envelope.wire_api != wire_api {
            info!(
                cached_wire_api = %envelope.wire_api,
                requested_wire_api = %wire_api,
                "ignoring wire api catalog cache for different wire_api"
            );
            return None;
        }
        Some(envelope)
    }

    fn load_envelope_sync(&self, wire_api: WireApi) -> Option<WireApiCatalogEnvelope> {
        let path = self.path_for(wire_api);
        let contents = match std_fs::read(&path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == ErrorKind::NotFound => return None,
            Err(err) => {
                error!(
                    "failed to read wire api catalog cache {}: {err}",
                    path.display()
                );
                return None;
            }
        };
        let envelope = match serde_json::from_slice::<WireApiCatalogEnvelope>(&contents) {
            Ok(envelope) => envelope,
            Err(err) => {
                error!(
                    "failed to parse wire api catalog cache {}: {err}",
                    path.display()
                );
                return None;
            }
        };
        if envelope.wire_api != wire_api {
            info!(
                cached_wire_api = %envelope.wire_api,
                requested_wire_api = %wire_api,
                "ignoring wire api catalog cache for different wire_api"
            );
            return None;
        }
        Some(envelope)
    }

    async fn save_envelope(&self, envelope: &WireApiCatalogEnvelope) -> io::Result<()> {
        let path = self.path_for(envelope.wire_api);
        if let Some(parent) = path.parent() {
            tokio_fs::create_dir_all(parent).await?;
        }
        let json = serde_json::to_string_pretty(envelope)
            .map_err(|err| io::Error::new(ErrorKind::InvalidData, err.to_string()))?;
        tokio::task::spawn_blocking(move || agere_utils_fs::write_atomically(&path, &json))
            .await
            .map_err(|err| io::Error::other(err.to_string()))?
    }
}

impl WireApiCatalogEnvelope {
    fn into_catalog(self) -> WireApiCatalog {
        WireApiCatalog {
            etag: self.etag,
            catalog_version: self.catalog_version,
            models: self.models,
        }
    }
}

fn is_fresh(fetched_at: DateTime<Utc>, ttl: Duration) -> bool {
    if ttl.is_zero() {
        return false;
    }
    let Ok(ttl_duration) = chrono::Duration::from_std(ttl) else {
        return false;
    };
    Utc::now().signed_duration_since(fetched_at) <= ttl_duration
}

fn matches_current_client_version(client_version: Option<&str>) -> bool {
    client_version == Some(crate::client_version_to_whole().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agere_protocol::openai_models::InputModality;
    use agere_protocol::openai_models::ModelInfo;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    fn model(slug: &str) -> ModelInfo {
        ModelInfo {
            input_modalities: Some(vec![InputModality::Text]),
            ..crate::model_info::model_info_from_slug(slug)
        }
    }

    #[tokio::test]
    async fn cache_path_is_keyed_by_wire_api_only() {
        let home = tempdir().expect("tempdir");
        let cache = WireApiCatalogCache::new(home.path().to_path_buf(), Duration::from_secs(300));

        assert_eq!(
            cache.path_for(WireApi::Responses),
            home.path().join("model_catalog").join("responses.json")
        );
        assert_eq!(
            cache.path_for(WireApi::Anthropic),
            home.path().join("model_catalog").join("anthropic.json")
        );
    }

    #[tokio::test]
    async fn load_fresh_rejects_wrong_wire_api() {
        let home = tempdir().expect("tempdir");
        let cache = WireApiCatalogCache::new(home.path().to_path_buf(), Duration::from_secs(300));
        cache
            .persist(
                WireApi::Responses,
                &[model("shared-model")],
                Some("\"etag\"".to_string()),
                crate::client_version_to_whole(),
                Some("v1".to_string()),
            )
            .await;

        assert_eq!(cache.load_fresh(WireApi::Anthropic).await, None);
    }

    #[tokio::test]
    async fn load_fresh_rejects_wrong_client_version() {
        let home = tempdir().expect("tempdir");
        let cache = WireApiCatalogCache::new(home.path().to_path_buf(), Duration::from_secs(300));
        cache
            .persist(
                WireApi::Responses,
                &[model("versioned-model")],
                Some("\"etag\"".to_string()),
                "0.0.0".to_string(),
                Some("v1".to_string()),
            )
            .await;

        assert_eq!(cache.load_fresh(WireApi::Responses).await, None);
    }

    #[tokio::test]
    async fn load_stale_keeps_expired_catalog_available() {
        let home = tempdir().expect("tempdir");
        let cache = WireApiCatalogCache::new(home.path().to_path_buf(), Duration::ZERO);
        cache
            .persist(
                WireApi::Chat,
                &[model("chat-model")],
                None,
                crate::client_version_to_whole(),
                None,
            )
            .await;

        assert_eq!(cache.load_fresh(WireApi::Chat).await, None);
        assert_eq!(
            cache
                .load_stale(WireApi::Chat)
                .await
                .expect("stale catalog")
                .models
                .iter()
                .map(|model| model.slug.as_str())
                .collect::<Vec<_>>(),
            vec!["chat-model"]
        );
    }

    #[tokio::test]
    async fn persist_replaces_existing_cache_file() {
        let home = tempdir().expect("tempdir");
        let cache = WireApiCatalogCache::new(home.path().to_path_buf(), Duration::from_secs(300));

        cache
            .persist(
                WireApi::Responses,
                &[model("old-model")],
                None,
                crate::client_version_to_whole(),
                Some("old".to_string()),
            )
            .await;
        cache
            .persist(
                WireApi::Responses,
                &[model("new-model")],
                None,
                crate::client_version_to_whole(),
                Some("new".to_string()),
            )
            .await;

        let catalog = cache
            .load_fresh(WireApi::Responses)
            .await
            .expect("fresh catalog should load after replacement");

        assert_eq!(catalog.catalog_version, Some("new".to_string()));
        assert_eq!(
            catalog
                .models
                .iter()
                .map(|model| model.slug.as_str())
                .collect::<Vec<_>>(),
            vec!["new-model"]
        );
    }

    #[tokio::test]
    async fn persist_writes_slim_catalog_models_without_display_or_context() {
        let home = tempdir().expect("tempdir");
        let cache = WireApiCatalogCache::new(home.path().to_path_buf(), Duration::from_secs(300));
        let mut cached_model = model("slim-model");
        cached_model.display_name = "Should not be persisted".to_string();
        cached_model.context_window = Some(123_456);

        cache
            .persist(
                WireApi::Responses,
                &[cached_model],
                None,
                crate::client_version_to_whole(),
                Some("slim".to_string()),
            )
            .await;

        let json = std::fs::read_to_string(cache.path_for(WireApi::Responses))
            .expect("wire api catalog cache should be readable");

        assert!(!json.contains("display_name"));
        assert!(!json.contains("context_window"));
        assert!(json.contains("input_modalities"));
    }
}
