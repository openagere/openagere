use crate::catalog_overlay::CatalogModel;
use agere_model_provider_info::WireApi;
use serde::Deserialize;
use serde::Serialize;
use std::fs as std_fs;
use std::io;
use std::io::ErrorKind;
use std::path::PathBuf;
use tokio::fs as tokio_fs;
use tracing::error;
use tracing::info;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WireApiCatalog {
    pub(crate) etag: Option<String>,
    pub(crate) version: Option<String>,
    pub(crate) models: Vec<CatalogModel>,
}

#[derive(Clone, Debug)]
pub(crate) struct WireApiCatalogCache {
    agere_home: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireApiCatalogEnvelope {
    wire_api: WireApi,
    #[serde(default)]
    version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    etag: Option<String>,
    models: Vec<CatalogModel>,
}

impl WireApiCatalogCache {
    pub(crate) fn new(agere_home: PathBuf) -> Self {
        Self { agere_home }
    }

    pub(crate) fn path_for(&self, wire_api: WireApi) -> PathBuf {
        self.agere_home
            .join("model_catalog")
            .join(format!("{wire_api}.json"))
    }

    pub(crate) async fn load(&self, wire_api: WireApi) -> Option<WireApiCatalog> {
        self.load_envelope(wire_api)
            .await
            .map(WireApiCatalogEnvelope::into_catalog)
    }

    pub(crate) fn load_sync(&self, wire_api: WireApi) -> Option<WireApiCatalog> {
        self.load_envelope_sync(wire_api)
            .map(WireApiCatalogEnvelope::into_catalog)
    }

    pub(crate) async fn persist<M>(
        &self,
        wire_api: WireApi,
        models: &[M],
        etag: Option<String>,
        version: Option<String>,
    ) where
        M: Clone,
        CatalogModel: From<M>,
    {
        let envelope = WireApiCatalogEnvelope {
            wire_api,
            version: version.unwrap_or_default(),
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
            version: Some(self.version).filter(|v| !v.is_empty()),
            models: self.models,
        }
    }
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
        let cache = WireApiCatalogCache::new(home.path().to_path_buf());

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
    async fn load_rejects_wrong_wire_api() {
        let home = tempdir().expect("tempdir");
        let cache = WireApiCatalogCache::new(home.path().to_path_buf());
        cache
            .persist(
                WireApi::Responses,
                &[model("shared-model")],
                Some("\"etag\"".to_string()),
                Some("v1".to_string()),
            )
            .await;

        assert_eq!(cache.load(WireApi::Anthropic).await, None);
    }

    #[tokio::test]
    async fn load_always_returns_cache_regardless_of_version() {
        let home = tempdir().expect("tempdir");
        let cache = WireApiCatalogCache::new(home.path().to_path_buf());
        cache
            .persist(
                WireApi::Responses,
                &[model("versioned-model")],
                Some("\"etag\"".to_string()),
                Some("v1".to_string()),
            )
            .await;

        assert!(cache.load(WireApi::Responses).await.is_some());
    }

    #[tokio::test]
    async fn load_returns_catalog() {
        let home = tempdir().expect("tempdir");
        let cache = WireApiCatalogCache::new(home.path().to_path_buf());
        cache
            .persist(WireApi::Chat, &[model("chat-model")], None, None)
            .await;

        assert_eq!(
            cache
                .load(WireApi::Chat)
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
        let cache = WireApiCatalogCache::new(home.path().to_path_buf());

        cache
            .persist(
                WireApi::Responses,
                &[model("old-model")],
                None,
                Some("old".to_string()),
            )
            .await;
        cache
            .persist(
                WireApi::Responses,
                &[model("new-model")],
                None,
                Some("new".to_string()),
            )
            .await;

        let catalog = cache
            .load(WireApi::Responses)
            .await
            .expect("fresh catalog should load after replacement");

        assert_eq!(catalog.version, Some("new".to_string()));
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
        let cache = WireApiCatalogCache::new(home.path().to_path_buf());
        let mut cached_model = model("slim-model");
        cached_model.display_name = "Should not be persisted".to_string();
        cached_model.context_window = Some(123_456);

        cache
            .persist(
                WireApi::Responses,
                &[cached_model],
                None,
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
