use std::sync::Arc;
use std::sync::PoisonError;
use std::sync::RwLock;

use agere_model_provider_info::WireApi;
use agere_protocol::config_types::CollaborationModeMask;
use agere_protocol::openai_models::ModelInfo;
use agere_protocol::openai_models::ModelsResponse;
use async_trait::async_trait;
use tokio::sync::TryLockError;

#[cfg(test)]
use crate::catalog_overlay::CatalogModel;
use crate::catalog_overlay::CatalogOverlay;
use crate::manager::ModelsManager;
use crate::manager::RefreshStrategy;
use crate::manager::SharedModelsManager;

/// 每会话私有、可在 provider 切换时热替换 inner 的 `ModelsManager` 包装。
///
/// 所有方法都委托给当前 inner，使 catalog 与 picker auth 语义一起反映新 provider。
#[derive(Debug)]
pub struct SwappableModelsManager {
    inner: RwLock<SharedModelsManager>,
}

impl SwappableModelsManager {
    /// Construct a wrapper around `inner`.
    pub fn new(inner: SharedModelsManager) -> Self {
        Self {
            inner: RwLock::new(inner),
        }
    }

    /// Hot-swap the inner manager.
    pub fn swap(&self, new_inner: SharedModelsManager) {
        *self.inner.write().unwrap_or_else(PoisonError::into_inner) = new_inner;
    }

    /// Snapshot the current inner manager (clone the `Arc` and drop the lock).
    fn current(&self) -> SharedModelsManager {
        Arc::clone(&self.inner.read().unwrap_or_else(PoisonError::into_inner))
    }
}

#[async_trait]
impl ModelsManager for SwappableModelsManager {
    async fn raw_model_catalog(&self, refresh_strategy: RefreshStrategy) -> ModelsResponse {
        self.current().raw_model_catalog(refresh_strategy).await
    }

    async fn get_remote_models(&self) -> Vec<ModelInfo> {
        self.current().get_remote_models().await
    }

    fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError> {
        self.current().try_get_remote_models()
    }

    fn auth_manager(&self) -> Option<Arc<agere_login::AuthManager>> {
        self.current().auth_manager()
    }

    fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        self.current().list_collaboration_modes()
    }

    fn wire_api(&self) -> WireApi {
        self.current().wire_api()
    }

    async fn wire_api_overlay_catalog(&self) -> Option<CatalogOverlay> {
        self.current().wire_api_overlay_catalog().await
    }

    fn try_wire_api_overlay_catalog(&self) -> Option<CatalogOverlay> {
        self.current().try_wire_api_overlay_catalog()
    }

    async fn refresh_if_new_etag(&self, etag: String) {
        self.current().refresh_if_new_etag(etag).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collaboration_mode_presets::CollaborationModesConfig;
    use crate::manager::RefreshStrategy;
    use crate::manager::StaticModelsManager;
    use crate::model_info;
    use crate::test_support::static_manager_with_models;
    use agere_login::AgereAuth;
    use agere_login::AuthManager;
    use agere_protocol::openai_models::InputModality;
    use async_trait::async_trait;

    #[derive(Debug)]
    struct OverlayModelsManager {
        overlay: CatalogOverlay,
    }

    #[async_trait]
    impl ModelsManager for OverlayModelsManager {
        async fn raw_model_catalog(&self, _refresh_strategy: RefreshStrategy) -> ModelsResponse {
            ModelsResponse { models: Vec::new() }
        }

        async fn get_remote_models(&self) -> Vec<ModelInfo> {
            Vec::new()
        }

        fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError> {
            Ok(Vec::new())
        }

        fn auth_manager(&self) -> Option<Arc<AuthManager>> {
            None
        }

        fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
            Vec::new()
        }

        fn wire_api(&self) -> WireApi {
            WireApi::Responses
        }

        async fn wire_api_overlay_catalog(&self) -> Option<CatalogOverlay> {
            Some(self.overlay.clone())
        }

        fn try_wire_api_overlay_catalog(&self) -> Option<CatalogOverlay> {
            Some(self.overlay.clone())
        }

        async fn refresh_if_new_etag(&self, _etag: String) {}
    }

    #[tokio::test]
    async fn swap_replaces_inner_catalog() {
        let first = static_manager_with_models(&["model-a"]);
        let second = static_manager_with_models(&["model-b"]);
        let swappable = SwappableModelsManager::new(first);

        let before = swappable.raw_model_catalog(RefreshStrategy::Offline).await;
        assert!(before.models.iter().any(|m| m.slug == "model-a"));

        swappable.swap(second);

        let after = swappable.raw_model_catalog(RefreshStrategy::Offline).await;
        assert!(after.models.iter().any(|m| m.slug == "model-b"));
        assert!(!after.models.iter().any(|m| m.slug == "model-a"));
    }

    #[tokio::test]
    async fn auth_manager_follows_current_inner_across_swap() {
        let first = static_manager_with_models(&["model-a"]);
        let second_auth =
            AuthManager::from_auth_for_testing(AgereAuth::create_dummy_chatgpt_auth_for_testing());
        let second: SharedModelsManager = Arc::new(StaticModelsManager::new(
            Some(Arc::clone(&second_auth)),
            ModelsResponse { models: Vec::new() },
            CollaborationModesConfig::default(),
            WireApi::Responses,
        ));
        let swappable = SwappableModelsManager::new(first);
        assert!(swappable.auth_manager().is_none());
        swappable.swap(second);
        assert!(swappable.auth_manager().is_some());
    }

    #[tokio::test]
    async fn wire_api_overlay_catalog_follows_current_inner() {
        let mut overlay_model = model_info::model_info_from_slug("overlay");
        overlay_model.input_modalities = Some(vec![InputModality::Text, InputModality::Image]);
        let inner: SharedModelsManager = Arc::new(OverlayModelsManager {
            overlay: CatalogOverlay {
                models: vec![CatalogModel::from(overlay_model.clone())],
            },
        });
        let swappable = SwappableModelsManager::new(inner);

        let overlay = swappable
            .wire_api_overlay_catalog()
            .await
            .expect("swappable should delegate overlay catalog to current inner");

        assert_eq!(
            overlay.models[0].input_modalities,
            overlay_model.input_modalities
        );
    }

    #[test]
    fn try_wire_api_overlay_catalog_follows_current_inner() {
        let mut overlay_model = model_info::model_info_from_slug("overlay");
        overlay_model.input_modalities = Some(vec![InputModality::Text, InputModality::Image]);
        let inner: SharedModelsManager = Arc::new(OverlayModelsManager {
            overlay: CatalogOverlay {
                models: vec![CatalogModel::from(overlay_model.clone())],
            },
        });
        let swappable = SwappableModelsManager::new(inner);

        let overlay = swappable
            .try_wire_api_overlay_catalog()
            .expect("swappable should delegate synchronous overlay catalog to current inner");

        assert_eq!(
            overlay.models[0].input_modalities,
            overlay_model.input_modalities
        );
    }
}
