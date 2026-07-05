use crate::catalog_overlay::CatalogModel;
use crate::wire_api_catalog::WireApiCatalog;
use agere_login::default_client::build_reqwest_client;
use agere_model_provider_info::WireApi;
use agere_protocol::error::AgereErr;
use agere_protocol::error::ConnectionFailedError;
use agere_protocol::error::Result as CoreResult;
use agere_protocol::error::UnexpectedResponseError;
use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

const DEFAULT_CATALOG_URL: &str = "https://api.openagere.com/model-catalog";
const FETCH_TIMEOUT: Duration = Duration::from_secs(3);

#[async_trait]
pub(crate) trait WireApiCatalogClient: std::fmt::Debug + Send + Sync {
    async fn fetch(
        &self,
        wire_api: WireApi,
        client_version: &str,
        etag: Option<&str>,
    ) -> CoreResult<WireApiCatalog>;
}

#[derive(Debug)]
pub(crate) struct OpenAgereWireApiCatalogClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct RemoteCatalogResponse {
    #[serde(default)]
    #[serde(alias = "catalog_version")]
    version: Option<String>,
    #[serde(default)]
    models: Vec<CatalogModel>,
}

impl OpenAgereWireApiCatalogClient {
    pub(crate) fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: build_reqwest_client(),
        }
    }
}

impl Default for OpenAgereWireApiCatalogClient {
    fn default() -> Self {
        Self::new(DEFAULT_CATALOG_URL.to_string())
    }
}

#[async_trait]
impl WireApiCatalogClient for OpenAgereWireApiCatalogClient {
    async fn fetch(
        &self,
        wire_api: WireApi,
        client_version: &str,
        etag: Option<&str>,
    ) -> CoreResult<WireApiCatalog> {
        let mut request = self
            .client
            .get(&self.base_url)
            .timeout(FETCH_TIMEOUT)
            .query(&[
                ("wire_api", wire_api.to_string()),
                ("client_version", client_version.to_string()),
            ]);
        if let Some(etag) = etag {
            request = request.header(http::header::IF_NONE_MATCH, etag);
        }
        let response = request
            .send()
            .await
            .map_err(|source| AgereErr::ConnectionFailed(ConnectionFailedError { source }))?;
        if response.status() == http::StatusCode::NOT_MODIFIED {
            return Ok(WireApiCatalog {
                etag: etag.map(str::to_string),
                version: None,
                models: Vec::new(),
            });
        }
        if !response.status().is_success() {
            return Err(AgereErr::UnexpectedStatus(UnexpectedResponseError {
                status: response.status(),
                body: response.text().await.unwrap_or_default(),
                url: Some(self.base_url.clone()),
                cf_ray: None,
                request_id: None,
                identity_authorization_error: None,
                identity_error_code: None,
            }));
        }
        let response_etag = response
            .headers()
            .get(http::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body: RemoteCatalogResponse = response
            .json()
            .await
            .map_err(|source| AgereErr::ConnectionFailed(ConnectionFailedError { source }))?;
        if body.models.is_empty() {
            return Err(AgereErr::InvalidRequest(
                "wire api catalog returned no models".to_string(),
            ));
        }
        Ok(WireApiCatalog {
            etag: response_etag,
            version: body.version,
            models: body.models,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    #[tokio::test]
    async fn fetch_catalog_sends_wire_api_query() {
        let server = MockServer::start().await;
        let model = crate::model_info::model_info_from_slug("remote-catalog-model");
        let originator = agere_login::default_client::originator().value;
        Mock::given(method("GET"))
            .and(path("/model-catalog"))
            .and(header("originator", originator.as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "version": "v1",
                "models": [{
                    "slug": model.slug,
                    "input_modalities": ["text", "image"]
                }]
            })))
            .mount(&server)
            .await;

        let client = OpenAgereWireApiCatalogClient::new(format!("{}/model-catalog", server.uri()));
        let catalog = client
            .fetch(WireApi::Responses, "0.3.24", None)
            .await
            .expect("fetch");

        assert_eq!(catalog.version, Some("v1".to_string()));
        assert_eq!(catalog.models[0].slug, "remote-catalog-model");
        assert_eq!(
            catalog.models[0].input_modalities,
            Some(vec![
                agere_protocol::openai_models::InputModality::Text,
                agere_protocol::openai_models::InputModality::Image
            ])
        );
    }
}
