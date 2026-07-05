use super::cache::ModelsCacheManager;
use crate::catalog_overlay;
use crate::catalog_overlay::CatalogOverlay;
use crate::collaboration_mode_presets::CollaborationModesConfig;
use crate::collaboration_mode_presets::builtin_collaboration_mode_presets;
use crate::config::ModelsManagerConfig;
use crate::model_info;
use crate::wire_api_catalog::WireApiCatalogCache;
use crate::wire_api_catalog_client::OpenAgereWireApiCatalogClient;
use crate::wire_api_catalog_client::WireApiCatalogClient;
use agere_login::AuthManager;
use agere_model_provider_info::WireApi;
use agere_protocol::config_types::CollaborationModeMask;
use agere_protocol::error::Result as CoreResult;
use agere_protocol::openai_models::ModelInfo;
use agere_protocol::openai_models::ModelPreset;
use agere_protocol::openai_models::ModelsResponse;
use async_trait::async_trait;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::sync::TryLockError;
use tracing::Instrument as _;
use tracing::error;
use tracing::info;

const MODEL_CACHE_FILE: &str = "models_cache.json";
const DEFAULT_MODEL_CACHE_TTL: Duration = Duration::from_secs(300);

/// Remote endpoint used by the wire API-compatible model manager.
///
/// Implementations own provider-specific auth and transport details. The model
/// manager owns refresh policy, cache behavior, and catalog merging; it calls
/// this endpoint only when it decides a remote refresh should happen.
#[async_trait]
pub trait ModelsEndpointClient: fmt::Debug + Send + Sync {
    /// Returns whether this provider can refresh models without Agere backend auth.
    fn can_refresh_without_agere_backend(&self) -> bool {
        false
    }

    /// Returns whether the currently resolved auth can use Agere backend-only models.
    async fn uses_agere_backend(&self) -> bool;

    /// Fetches the latest remote model catalog and optional ETag.
    async fn list_models(
        &self,
        client_version: &str,
    ) -> CoreResult<(Vec<ModelInfo>, Option<String>)>;
}

/// Strategy for refreshing available models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshStrategy {
    /// Always fetch from the network, ignoring cache.
    Online,
    /// Only use cached data, never fetch from the network.
    Offline,
    /// Use cache if available and fresh, otherwise fetch from the network.
    OnlineIfUncached,
}

impl RefreshStrategy {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Offline => "offline",
            Self::OnlineIfUncached => "online_if_uncached",
        }
    }
}

impl fmt::Display for RefreshStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

type SharedModelsEndpointClient = Arc<dyn ModelsEndpointClient>;
type SharedWireApiCatalogClient = Arc<dyn WireApiCatalogClient>;

/// Coordinates model discovery plus cached metadata on disk.
#[async_trait]
pub trait ModelsManager: fmt::Debug + Send + Sync {
    /// List all available models, refreshing according to the specified strategy.
    ///
    /// Returns model presets sorted by priority and filtered by auth mode and visibility.
    async fn list_models(&self, refresh_strategy: RefreshStrategy) -> Vec<ModelPreset> {
        async move {
            let catalog = self.raw_model_catalog(refresh_strategy).await;
            self.build_available_models(catalog.models)
        }
        .instrument(tracing::info_span!(
            "list_models",
            refresh_strategy = %refresh_strategy
        ))
        .await
    }

    /// Return the active raw model catalog, refreshing according to the specified strategy.
    async fn raw_model_catalog(&self, refresh_strategy: RefreshStrategy) -> ModelsResponse;

    /// Return the current in-memory remote model catalog without refreshing or loading cache state.
    async fn get_remote_models(&self) -> Vec<ModelInfo>;

    /// Attempt to return the current in-memory remote model catalog without blocking.
    ///
    /// Returns an error if the internal lock cannot be acquired.
    fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError>;

    /// Return the auth manager used for picker filtering.
    fn auth_manager(&self) -> Option<Arc<AuthManager>>;

    /// Build picker-ready presets from the active catalog snapshot.
    fn build_available_models(&self, mut remote_models: Vec<ModelInfo>) -> Vec<ModelPreset> {
        remote_models.sort_by_key(|a| a.priority);

        let wire_api = self.wire_api();
        let mut presets: Vec<ModelPreset> = remote_models
            .into_iter()
            .map(|model| model_info::with_effective_input_modalities_for_wire_api(model, wire_api))
            .map(Into::into)
            .collect();
        let uses_agere_backend = self
            .auth_manager()
            .is_some_and(|auth_manager| auth_manager.current_auth_uses_agere_backend());
        presets = ModelPreset::filter_by_auth(presets, uses_agere_backend);

        ModelPreset::mark_default_by_picker_visibility(&mut presets);

        presets
    }

    /// List collaboration mode presets.
    ///
    /// Returns a static set of presets seeded with the configured model.
    fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask>;

    /// Attempt to list models without blocking, using the current cached state.
    ///
    /// Returns an error if the internal lock cannot be acquired.
    fn try_list_models(&self) -> Result<Vec<ModelPreset>, TryLockError> {
        let wire_api_catalog = self.try_wire_api_overlay_catalog();
        let mut overlays: Vec<CatalogOverlay> = Vec::new();
        if let Some(wire_api_catalog) = wire_api_catalog {
            overlays.push(wire_api_catalog);
        }
        let remote_models = self
            .try_get_remote_models()?
            .into_iter()
            .map(|model| catalog_overlay::apply_catalog_overlay(model, &overlays))
            .collect();
        Ok(self.build_available_models(remote_models))
    }

    // todo(aibrahim): should be visible to core only and sent on session_configured event
    /// Get the model identifier to use, refreshing according to the specified strategy.
    ///
    /// If `model` is provided, returns it directly. Otherwise selects the default based on
    /// auth mode and available models.
    async fn get_default_model(
        &self,
        model: &Option<String>,
        refresh_strategy: RefreshStrategy,
    ) -> String {
        async move {
            if let Some(model) = model.as_ref() {
                return model.to_string();
            }
            default_model_from_available(self.list_models(refresh_strategy).await)
        }
        .instrument(tracing::info_span!(
            "get_default_model",
            model.provided = model.is_some(),
            refresh_strategy = %refresh_strategy
        ))
        .await
    }

    // todo(aibrahim): look if we can tighten it to pub(crate)
    /// Look up model metadata, applying remote overrides and config adjustments.
    async fn get_model_info(&self, model: &str, config: &ModelsManagerConfig) -> ModelInfo {
        async move {
            let remote_models = self.get_remote_models().await;
            let wire_api_catalog = self.wire_api_overlay_catalog().await;
            construct_model_info_from_candidates(
                model,
                &remote_models,
                config,
                self.wire_api(),
                wire_api_catalog,
            )
        }
        .instrument(tracing::info_span!("get_model_info", model = model))
        .await
    }

    /// Return the wire API protocol used by this manager.
    fn wire_api(&self) -> WireApi;

    /// Return the wire_api catalog overlay for metadata fill, if available.
    async fn wire_api_overlay_catalog(&self) -> Option<CatalogOverlay> {
        None
    }

    /// Return the local wire_api catalog overlay without awaiting or fetching remotely.
    fn try_wire_api_overlay_catalog(&self) -> Option<CatalogOverlay> {
        None
    }

    /// Refresh models if the provided ETag differs from the cached ETag.
    ///
    /// Uses `Online` strategy to fetch latest models when ETags differ.
    async fn refresh_if_new_etag(&self, etag: String);
}

/// Shared model manager handle used across runtime services.
pub type SharedModelsManager = Arc<dyn ModelsManager>;

/// Wire-API-compatible model manager backed by bundled models, cache, and `/models`.
#[derive(Debug)]
pub struct WireApiModelsManager {
    remote_models: RwLock<Vec<ModelInfo>>,
    collaboration_modes_config: CollaborationModesConfig,
    etag: RwLock<Option<String>>,
    cache_manager: ModelsCacheManager,
    endpoint_client: SharedModelsEndpointClient,
    auth_manager: Option<Arc<AuthManager>>,
    wire_api: WireApi,
    wire_api_catalog_cache: WireApiCatalogCache,
    wire_api_catalog_client: SharedWireApiCatalogClient,
    wire_api_catalog_refresh_inflight: Arc<AtomicBool>,
}

/// Static model manager backed by an authoritative in-process catalog.
#[derive(Debug)]
pub struct StaticModelsManager {
    remote_models: Vec<ModelInfo>,
    collaboration_modes_config: CollaborationModesConfig,
    auth_manager: Option<Arc<AuthManager>>,
    wire_api: WireApi,
    wire_api_catalog_cache: Option<WireApiCatalogCache>,
}

impl WireApiModelsManager {
    /// Construct a wire-API-compatible remote model manager.
    pub fn new(
        agere_home: PathBuf,
        wire_api: WireApi,
        endpoint_client: Arc<dyn ModelsEndpointClient>,
        auth_manager: Option<Arc<AuthManager>>,
        collaboration_modes_config: CollaborationModesConfig,
    ) -> Self {
        Self::new_with_wire_api_catalog_client(
            agere_home,
            wire_api,
            endpoint_client,
            auth_manager,
            collaboration_modes_config,
            Arc::new(OpenAgereWireApiCatalogClient::default()),
        )
    }

    fn new_with_wire_api_catalog_client(
        agere_home: PathBuf,
        wire_api: WireApi,
        endpoint_client: Arc<dyn ModelsEndpointClient>,
        auth_manager: Option<Arc<AuthManager>>,
        collaboration_modes_config: CollaborationModesConfig,
        wire_api_catalog_client: SharedWireApiCatalogClient,
    ) -> Self {
        let cache_path = agere_home.join(MODEL_CACHE_FILE);
        let cache_manager = ModelsCacheManager::new(cache_path, DEFAULT_MODEL_CACHE_TTL);
        let wire_api_catalog_cache = WireApiCatalogCache::new(agere_home, DEFAULT_MODEL_CACHE_TTL);
        let remote_models = load_remote_models_from_file().unwrap_or_default();
        Self {
            remote_models: RwLock::new(remote_models),
            collaboration_modes_config,
            etag: RwLock::new(None),
            cache_manager,
            endpoint_client,
            auth_manager,
            wire_api,
            wire_api_catalog_cache,
            wire_api_catalog_client,
            wire_api_catalog_refresh_inflight: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl StaticModelsManager {
    /// Construct a static model manager from an authoritative catalog.
    pub fn new(
        auth_manager: Option<Arc<AuthManager>>,
        model_catalog: ModelsResponse,
        collaboration_modes_config: CollaborationModesConfig,
        wire_api: WireApi,
    ) -> Self {
        Self::new_inner(
            auth_manager,
            model_catalog,
            collaboration_modes_config,
            wire_api,
            None,
        )
    }

    /// Construct a static model manager that can fill missing model metadata from the local wire
    /// API catalog cache.
    pub fn new_with_wire_api_catalog_cache(
        auth_manager: Option<Arc<AuthManager>>,
        model_catalog: ModelsResponse,
        collaboration_modes_config: CollaborationModesConfig,
        wire_api: WireApi,
        agere_home: PathBuf,
    ) -> Self {
        Self::new_inner(
            auth_manager,
            model_catalog,
            collaboration_modes_config,
            wire_api,
            Some(WireApiCatalogCache::new(
                agere_home,
                DEFAULT_MODEL_CACHE_TTL,
            )),
        )
    }

    fn new_inner(
        auth_manager: Option<Arc<AuthManager>>,
        model_catalog: ModelsResponse,
        collaboration_modes_config: CollaborationModesConfig,
        wire_api: WireApi,
        wire_api_catalog_cache: Option<WireApiCatalogCache>,
    ) -> Self {
        Self {
            remote_models: model_catalog.models,
            collaboration_modes_config,
            auth_manager,
            wire_api,
            wire_api_catalog_cache,
        }
    }
}

#[async_trait]
impl ModelsManager for WireApiModelsManager {
    async fn raw_model_catalog(&self, refresh_strategy: RefreshStrategy) -> ModelsResponse {
        if let Err(err) = self.refresh_available_models(refresh_strategy).await {
            error!("failed to refresh available models: {err}");
        }
        if !matches!(refresh_strategy, RefreshStrategy::Offline) {
            self.refresh_wire_api_overlay_catalog_in_background().await;
        }
        let wire_api_catalog = self.wire_api_overlay_catalog().await;
        let mut overlays: Vec<CatalogOverlay> = Vec::new();
        if let Some(wire_api_catalog) = wire_api_catalog {
            overlays.push(wire_api_catalog);
        }
        let models = self
            .get_remote_models()
            .await
            .into_iter()
            .map(|model| catalog_overlay::apply_catalog_overlay(model, &overlays))
            .collect();
        ModelsResponse { models }
    }

    async fn get_remote_models(&self) -> Vec<ModelInfo> {
        self.remote_models.read().await.clone()
    }

    fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError> {
        Ok(self.remote_models.try_read()?.clone())
    }

    fn auth_manager(&self) -> Option<Arc<AuthManager>> {
        self.auth_manager.clone()
    }

    fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        builtin_collaboration_mode_presets(self.collaboration_modes_config)
    }

    fn wire_api(&self) -> WireApi {
        self.wire_api
    }

    async fn wire_api_overlay_catalog(&self) -> Option<CatalogOverlay> {
        if let Some(catalog) = self.wire_api_catalog_cache.load_fresh(self.wire_api).await {
            return Some(CatalogOverlay {
                models: catalog.models,
            });
        }

        self.wire_api_catalog_cache
            .load_stale(self.wire_api)
            .await
            .map(|catalog| CatalogOverlay {
                models: catalog.models,
            })
    }

    fn try_wire_api_overlay_catalog(&self) -> Option<CatalogOverlay> {
        if let Some(catalog) = self.wire_api_catalog_cache.load_fresh_sync(self.wire_api) {
            return Some(CatalogOverlay {
                models: catalog.models,
            });
        }

        self.wire_api_catalog_cache
            .load_stale_sync(self.wire_api)
            .map(|catalog| CatalogOverlay {
                models: catalog.models,
            })
    }

    async fn refresh_if_new_etag(&self, etag: String) {
        let current_etag = self.get_etag().await;
        if current_etag.clone().is_some() && current_etag.as_deref() == Some(etag.as_str()) {
            if let Err(err) = self.cache_manager.renew_cache_ttl().await {
                error!("failed to renew cache TTL: {err}");
            }
            return;
        }
        if let Err(err) = self.refresh_available_models(RefreshStrategy::Online).await {
            error!("failed to refresh available models: {err}");
        }
    }
}

impl WireApiModelsManager {
    async fn refresh_wire_api_overlay_catalog_in_background(&self) {
        if self
            .wire_api_catalog_cache
            .load_fresh(self.wire_api)
            .await
            .is_some()
        {
            return;
        }

        let stale_catalog = self.wire_api_catalog_cache.load_stale(self.wire_api).await;
        let client_version = crate::client_version_to_whole();
        if self
            .wire_api_catalog_refresh_inflight
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            let wire_api = self.wire_api;
            let wire_api_catalog_cache = self.wire_api_catalog_cache.clone();
            let wire_api_catalog_client = Arc::clone(&self.wire_api_catalog_client);
            let refresh_inflight = Arc::clone(&self.wire_api_catalog_refresh_inflight);
            let stale_catalog_for_refresh = stale_catalog;
            tokio::spawn(async move {
                let etag = stale_catalog_for_refresh
                    .as_ref()
                    .and_then(|catalog| catalog.etag.as_deref());
                match wire_api_catalog_client
                    .fetch(wire_api, &client_version, etag)
                    .await
                {
                    Ok(catalog) if catalog.models.is_empty() => {
                        if let Some(stale_catalog) = stale_catalog_for_refresh {
                            wire_api_catalog_cache
                                .persist(
                                    wire_api,
                                    &stale_catalog.models,
                                    catalog.etag.or(stale_catalog.etag.clone()),
                                    client_version,
                                    stale_catalog.catalog_version.clone(),
                                )
                                .await;
                        }
                    }
                    Ok(catalog) => {
                        wire_api_catalog_cache
                            .persist(
                                wire_api,
                                &catalog.models,
                                catalog.etag.clone(),
                                client_version,
                                catalog.catalog_version,
                            )
                            .await;
                    }
                    Err(err) => {
                        error!("failed to fetch wire api model catalog: {err}");
                    }
                }
                refresh_inflight.store(false, Ordering::Release);
            });
        }
    }

    /// Refresh available models according to the specified strategy.
    async fn refresh_available_models(&self, refresh_strategy: RefreshStrategy) -> CoreResult<()> {
        if !self.should_refresh_models().await {
            if matches!(
                refresh_strategy,
                RefreshStrategy::Offline | RefreshStrategy::OnlineIfUncached
            ) {
                self.try_load_cache().await;
            }
            return Ok(());
        }

        match refresh_strategy {
            RefreshStrategy::Offline => {
                self.try_load_cache().await;
                Ok(())
            }
            RefreshStrategy::OnlineIfUncached => {
                if self.try_load_cache().await {
                    info!("models cache: using cached models for OnlineIfUncached");
                    return Ok(());
                }
                info!("models cache: cache miss, fetching remote models");
                self.fetch_and_update_models().await
            }
            RefreshStrategy::Online => self.fetch_and_update_models().await,
        }
    }

    async fn fetch_and_update_models(&self) -> CoreResult<()> {
        let client_version = crate::client_version_to_whole();
        let (models, etag) = self.endpoint_client.list_models(&client_version).await?;
        self.apply_endpoint_models(models.clone()).await;
        *self.etag.write().await = etag.clone();
        self.cache_manager
            .persist_cache(&models, etag, client_version)
            .await;
        Ok(())
    }

    async fn should_refresh_models(&self) -> bool {
        self.endpoint_client.uses_agere_backend().await
            || self.endpoint_client.can_refresh_without_agere_backend()
    }

    async fn should_replace_with_endpoint_models(&self) -> bool {
        self.endpoint_client.can_refresh_without_agere_backend()
            && !self.endpoint_client.uses_agere_backend().await
    }

    async fn get_etag(&self) -> Option<String> {
        self.etag.read().await.clone()
    }

    async fn apply_endpoint_models(&self, models: Vec<ModelInfo>) {
        if self.should_replace_with_endpoint_models().await {
            *self.remote_models.write().await = models;
        } else {
            self.apply_remote_models(models).await;
        }
    }

    async fn apply_remote_models(&self, models: Vec<ModelInfo>) {
        let mut existing_models = load_remote_models_from_file().unwrap_or_default();
        for model in models {
            if let Some(existing_index) = existing_models
                .iter()
                .position(|existing| existing.slug == model.slug)
            {
                existing_models[existing_index] = model;
            } else {
                existing_models.push(model);
            }
        }
        *self.remote_models.write().await = existing_models;
    }

    async fn try_load_cache(&self) -> bool {
        let _timer =
            agere_otel::start_global_timer("agere.remote_models.load_cache.duration_ms", &[]);
        let client_version = crate::client_version_to_whole();
        info!(client_version, "models cache: evaluating cache eligibility");
        let cache = match self.cache_manager.load_fresh(&client_version).await {
            Some(cache) => cache,
            None => {
                info!("models cache: no usable cache entry");
                return false;
            }
        };
        let models = cache.models.clone();
        *self.etag.write().await = cache.etag.clone();
        self.apply_endpoint_models(models.clone()).await;
        info!(
            models_count = models.len(),
            etag = ?cache.etag,
            "models cache: cache entry applied"
        );
        true
    }
}

#[async_trait]
impl ModelsManager for StaticModelsManager {
    async fn raw_model_catalog(&self, _refresh_strategy: RefreshStrategy) -> ModelsResponse {
        let wire_api_catalog = self.wire_api_overlay_catalog().await;
        let mut overlays: Vec<CatalogOverlay> = Vec::new();
        if let Some(wire_api_catalog) = wire_api_catalog {
            overlays.push(wire_api_catalog);
        }
        let models = self
            .get_remote_models()
            .await
            .into_iter()
            .map(|model| catalog_overlay::apply_catalog_overlay(model, &overlays))
            .collect();
        ModelsResponse { models }
    }

    async fn get_remote_models(&self) -> Vec<ModelInfo> {
        self.remote_models.clone()
    }

    fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError> {
        Ok(self.remote_models.clone())
    }

    fn auth_manager(&self) -> Option<Arc<AuthManager>> {
        self.auth_manager.clone()
    }

    fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        builtin_collaboration_mode_presets(self.collaboration_modes_config)
    }

    fn wire_api(&self) -> WireApi {
        self.wire_api
    }

    async fn wire_api_overlay_catalog(&self) -> Option<CatalogOverlay> {
        let cache = self.wire_api_catalog_cache.as_ref()?;
        if let Some(catalog) = cache.load_fresh(self.wire_api).await {
            return Some(CatalogOverlay {
                models: catalog.models,
            });
        }

        cache
            .load_stale(self.wire_api)
            .await
            .map(|catalog| CatalogOverlay {
                models: catalog.models,
            })
    }

    fn try_wire_api_overlay_catalog(&self) -> Option<CatalogOverlay> {
        let cache = self.wire_api_catalog_cache.as_ref()?;
        if let Some(catalog) = cache.load_fresh_sync(self.wire_api) {
            return Some(CatalogOverlay {
                models: catalog.models,
            });
        }

        cache
            .load_stale_sync(self.wire_api)
            .map(|catalog| CatalogOverlay {
                models: catalog.models,
            })
    }

    async fn refresh_if_new_etag(&self, _etag: String) {}
}

fn load_remote_models_from_file() -> Result<Vec<ModelInfo>, std::io::Error> {
    Ok(crate::bundled_models_response()?.models)
}

fn default_model_from_available(available: Vec<ModelPreset>) -> String {
    available
        .iter()
        .find(|model| model.is_default)
        .or_else(|| available.first())
        .map(|model| model.model.clone())
        .unwrap_or_default()
}

fn find_model_by_longest_prefix(model: &str, candidates: &[ModelInfo]) -> Option<ModelInfo> {
    let mut best: Option<ModelInfo> = None;
    for candidate in candidates {
        if !model.starts_with(&candidate.slug) {
            continue;
        }
        let is_better_match = if let Some(current) = best.as_ref() {
            candidate.slug.len() > current.slug.len()
        } else {
            true
        };
        if is_better_match {
            best = Some(candidate.clone());
        }
    }
    best
}

fn find_model_by_namespaced_suffix(model: &str, candidates: &[ModelInfo]) -> Option<ModelInfo> {
    let (namespace, suffix) = model.split_once('/')?;
    if suffix.contains('/') {
        return None;
    }
    if !namespace
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    find_model_by_longest_prefix(suffix, candidates)
}

pub(crate) fn construct_model_info_from_candidates(
    model: &str,
    candidates: &[ModelInfo],
    config: &ModelsManagerConfig,
    wire_api: WireApi,
    wire_api_catalog: Option<CatalogOverlay>,
) -> ModelInfo {
    let remote = find_model_by_longest_prefix(model, candidates)
        .or_else(|| find_model_by_namespaced_suffix(model, candidates));
    let model_info = if let Some(remote) = remote {
        ModelInfo {
            slug: model.to_string(),
            used_fallback_model_metadata: false,
            ..remote
        }
    } else {
        model_info::model_info_from_slug_for_wire_api(model, wire_api)
    };
    let mut overlays: Vec<CatalogOverlay> = Vec::new();
    if let Some(model_catalog) = config.model_catalog.as_ref() {
        overlays.push(CatalogOverlay::from_models_response(model_catalog));
    }
    if let Some(wire_api_catalog) = wire_api_catalog {
        overlays.push(wire_api_catalog);
    }
    let model_info = catalog_overlay::apply_catalog_overlay(model_info, &overlays);
    let model_info = model_info::with_config_overrides(model_info, config);
    model_info::with_effective_input_modalities_for_wire_api(model_info, wire_api)
}

#[cfg(test)]
#[path = "manager_tests.rs"]
mod tests;
