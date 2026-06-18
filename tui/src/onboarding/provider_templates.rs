//! Remote + built-in provider templates surfaced in the welcome / `/provider` picker.
//!
//! Remote target is `https://api.openagere.com/provider?region=<1|2>` where
//! region=1 (China) is selected when the system locale is Chinese; region=2
//! (International) is the default for all other locales.
//! Failure to fetch falls back to locale-appropriate embedded JSON.

use agere_config::config_toml::ModelConfig;
use agere_model_provider_info::WireApi;
use serde::Deserialize;
use serde::Serialize;
use std::sync::OnceLock;
use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;

const REMOTE_PROVIDERS_URL_CN: &str = "https://api.openagere.com/provider?region=1";
const REMOTE_PROVIDERS_URL_INTL: &str = "https://api.openagere.com/provider?region=2";
const FETCH_TIMEOUT: Duration = Duration::from_secs(3);

/// Offline fallback — CN region. Embedded in the binary so the picker always
/// has *something* to show when the user has no network.
const FALLBACK_PROVIDERS_JSON_CN: &str = include_str!("providers_fallback_cn.json");

/// Offline fallback — International region.
const FALLBACK_PROVIDERS_JSON_INTL: &str = include_str!("providers_fallback.json");

/// A provider template (JSON wire representation matching `providers.json`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ProviderTemplate {
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub wire_api: WireApi,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_key: Option<String>,
    #[serde(default)]
    pub models: Vec<ModelConfig>,
}

/// State tracked by the picker while remote templates load asynchronously.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) enum TemplateLoadState {
    Loading,
    Loaded(Vec<ProviderTemplate>),
    Failed(String),
}

#[derive(Deserialize)]
struct RemoteProvidersDoc {
    #[serde(default)]
    providers: Vec<serde_json::Value>,
}

/// Return built-in templates for a specific region (used by tests).
fn builtin_templates_for(region: crate::region::Region) -> Vec<ProviderTemplate> {
    let fallback_json = match region {
        crate::region::Region::Cn => FALLBACK_PROVIDERS_JSON_CN,
        crate::region::Region::Intl => FALLBACK_PROVIDERS_JSON_INTL,
    };
    let doc: RemoteProvidersDoc =
        serde_json::from_str(fallback_json).unwrap_or(RemoteProvidersDoc {
            providers: Vec::new(),
        });
    let mut templates = decode_templates(doc.providers);
    // Guarantee at least one provider exists even if the embedded JSON is
    // somehow corrupted, so the picker is never empty.
    if templates.is_empty() {
        templates.push(ProviderTemplate {
            name: "openai".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            wire_api: WireApi::Responses,
            env_key: Some("OPENAI_API_KEY".to_string()),
            models: vec![ModelConfig {
                name: "gpt-4o".to_string(),
                context_window: Some(200_000),
            }],
        });
    }
    templates
}

/// Built-in fallback list. Mirrors what the remote providers endpoint is
/// expected to ship; kept here so the picker has something usable when the
/// network is unavailable.
///
/// The CN / Intl JSON is selected based on the detected system locale.
pub(crate) fn builtin_templates() -> Vec<ProviderTemplate> {
    builtin_templates_for(crate::region::detect_region())
}

fn providers_url() -> &'static str {
    if cfg!(test)
        && let Ok(override_url) = std::env::var("AGERE_PROVIDERS_URL_TEST")
    {
        // Leak the string to return a &'static str — acceptable in tests.
        return Box::leak(override_url.into_boxed_str());
    }
    match crate::region::detect_region() {
        crate::region::Region::Cn => REMOTE_PROVIDERS_URL_CN,
        crate::region::Region::Intl => REMOTE_PROVIDERS_URL_INTL,
    }
}

fn http_client() -> &'static reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(FETCH_TIMEOUT)
            .build()
            .expect("failed to build HTTP client")
    })
}

/// Fetch the remote providers list with single-entry fault tolerance.
///
/// On success returns whatever templates parsed cleanly; malformed entries are
/// silently skipped. On network/HTTP error returns the raw `reqwest::Error`
/// message in `Err` so the caller can show a footer hint.
pub(crate) async fn fetch_remote_templates() -> Result<Vec<ProviderTemplate>, String> {
    let url = providers_url();
    let client = http_client();
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {status}", status = resp.status()));
    }
    let doc: RemoteProvidersDoc = resp.json().await.map_err(|e| e.to_string())?;
    Ok(decode_templates(doc.providers))
}

fn decode_templates(raw: Vec<serde_json::Value>) -> Vec<ProviderTemplate> {
    raw.into_iter()
        .filter_map(|v| match serde_json::from_value::<ProviderTemplate>(v) {
            Ok(t) => Some(t),
            Err(e) => {
                tracing::debug!("skipping malformed provider template: {e}");
                None
            }
        })
        .collect()
}

/// Global cache for remote provider templates.
///
/// Fetched once at TUI startup, then refreshed every 5 minutes in the
/// background. All consumers (onboarding picker, /provider wizard) read from
/// this cache instead of spawning their own fetch — eliminates the UI
/// flickering that occurred when remote data arrived and rebuilt the list.
static REMOTE_TEMPLATE_CACHE: OnceLock<RwLock<RemoteTemplateCacheInner>> = OnceLock::new();

struct RemoteTemplateCacheInner {
    templates: Vec<ProviderTemplate>,
    state: TemplateLoadState,
    last_fetched: Option<Instant>,
}

/// Refresh interval for the background cache update.
const CACHE_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Initialise the cache and return a clone of the current state.
///
/// If this is the first call, the cache is populated with `Loading` and the
/// caller should fall back to built-in templates. A background fetch is
/// spawned and will update the cache via [`update_cache`].
fn cache_inner() -> &'static RwLock<RemoteTemplateCacheInner> {
    REMOTE_TEMPLATE_CACHE.get_or_init(|| {
        RwLock::new(RemoteTemplateCacheInner {
            templates: Vec::new(),
            state: TemplateLoadState::Loading,
            last_fetched: None,
        })
    })
}

/// Read the cached templates. Returns `None` if the cache is still loading
/// or previously failed — callers should fall back to built-in templates.
pub(crate) fn get_cached_templates() -> Option<Vec<ProviderTemplate>> {
    let inner = cache_inner().read().ok()?;
    match &inner.state {
        TemplateLoadState::Loaded(templates) if !templates.is_empty() => Some(templates.clone()),
        _ => None,
    }
}

/// Update the cache with fresh remote templates. Called by the background
/// refresh task and by the onboarding screen when its fetch completes.
pub(crate) fn update_cache(state: TemplateLoadState) {
    let mut inner = cache_inner()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match &state {
        TemplateLoadState::Loaded(templates) => {
            inner.templates = templates.clone();
        }
        TemplateLoadState::Failed(_err) => {
            // Keep existing templates on failure so the list doesn't go blank.
        }
        TemplateLoadState::Loading => {}
    }
    inner.state = state;
    inner.last_fetched = Some(Instant::now());
}

/// Check whether the cache is stale and should be refreshed.
fn cache_is_stale() -> bool {
    let inner = cache_inner()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    inner
        .last_fetched
        .map(|t| t.elapsed() >= CACHE_REFRESH_INTERVAL)
        .unwrap_or(true)
}

/// Spawn a background task that fetches remote templates and updates the
/// cache. If the cache was recently refreshed, this is a no-op.
///
/// Should be called once at TUI startup. The task loops every 5 minutes.
pub(crate) fn spawn_background_refresh_task(
    event_tx: Option<crate::app_event_sender::AppEventSender>,
) {
    if !cache_is_stale() {
        return;
    }
    tokio::spawn(async move {
        loop {
            let result = fetch_remote_templates().await;
            let state = match &result {
                Ok(templates) if !templates.is_empty() => {
                    TemplateLoadState::Loaded(templates.clone())
                }
                Ok(_) => TemplateLoadState::Failed("no providers in upstream list".to_string()),
                Err(msg) => TemplateLoadState::Failed(msg.clone()),
            };
            update_cache(state);

            // Also broadcast the result via app event so any active UI can update.
            if let Ok(templates) = &result
                && !templates.is_empty()
                && let Some(ref tx) = event_tx
            {
                tx.send(crate::app_event::AppEvent::ProviderTemplatesLoaded {
                    templates: templates.clone(),
                });
            }

            tokio::time::sleep(CACHE_REFRESH_INTERVAL).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region::Region;
    use pretty_assertions::assert_eq;
    use serial_test::serial;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    #[test]
    fn builtin_templates_have_context_window_for_every_model() {
        for region in [Region::Cn, Region::Intl] {
            let templates = builtin_templates_for(region);
            assert!(!templates.is_empty(), "{region:?} has no templates");
            for template in &templates {
                assert!(
                    !template.models.is_empty(),
                    "{} has no models",
                    template.name
                );
                for model in &template.models {
                    assert!(
                        model.context_window.is_some(),
                        "{}::{} missing context_window",
                        template.name,
                        model.name,
                    );
                }
            }
        }
    }

    #[test]
    fn intl_fallback_includes_required_providers() {
        let templates = builtin_templates_for(Region::Intl);
        let names: Vec<&str> = templates.iter().map(|t| t.name.as_str()).collect();
        for required in [
            "OpenAI",
            "Anthropic",
            "DeepSeek",
            "OpenRouter",
            "Alibaba Coding Plan",
        ] {
            assert!(
                names.contains(&required),
                "intl fallback JSON missing '{required}' (have: {names:?})",
            );
        }
    }

    #[test]
    fn cn_fallback_has_providers() {
        let templates = builtin_templates_for(Region::Cn);
        assert!(
            templates.len() >= 10,
            "cn fallback has too few providers: {}",
            templates.len()
        );
    }

    #[tokio::test]
    #[serial]
    async fn fetch_decodes_full_payload() {
        let server = MockServer::start().await;
        let body = serde_json::json!({
            "providers": [
                {
                    "name": "deepseek",
                    "base_url": "https://api.deepseek.com/v1",
                    "wire_api": "chat",
                    "env_key": "DEEPSEEK_API_KEY",
                    "models": [
                        { "name": "deepseek-chat", "context_window": 200000 }
                    ]
                }
            ]
        });
        Mock::given(method("GET"))
            .and(path("/providers.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        unsafe {
            std::env::set_var(
                "AGERE_PROVIDERS_URL_TEST",
                format!("{}/providers.json", server.uri()),
            );
        }
        let result = fetch_remote_templates().await.expect("fetch");
        unsafe {
            std::env::remove_var("AGERE_PROVIDERS_URL_TEST");
        }
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "deepseek");
        assert_eq!(result[0].models.len(), 1);
    }

    #[tokio::test]
    #[serial]
    async fn fetch_skips_malformed_entries() {
        let server = MockServer::start().await;
        let body = serde_json::json!({
            "providers": [
                { "name": "good",   "base_url": "https://good.test", "wire_api": "chat" },
                { "name": "bad",    "wire_api": "definitely-not-valid" },
                "not-an-object"
            ]
        });
        Mock::given(method("GET"))
            .and(path("/providers.json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        unsafe {
            std::env::set_var(
                "AGERE_PROVIDERS_URL_TEST",
                format!("{}/providers.json", server.uri()),
            );
        }
        let result = fetch_remote_templates().await.expect("fetch");
        unsafe {
            std::env::remove_var("AGERE_PROVIDERS_URL_TEST");
        }
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "good");
    }

    #[tokio::test]
    #[serial]
    async fn fetch_returns_err_when_server_504() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/providers.json"))
            .respond_with(ResponseTemplate::new(504))
            .mount(&server)
            .await;

        unsafe {
            std::env::set_var(
                "AGERE_PROVIDERS_URL_TEST",
                format!("{}/providers.json", server.uri()),
            );
        }
        let result = fetch_remote_templates().await;
        unsafe {
            std::env::remove_var("AGERE_PROVIDERS_URL_TEST");
        }
        assert!(result.is_err());
    }
}
