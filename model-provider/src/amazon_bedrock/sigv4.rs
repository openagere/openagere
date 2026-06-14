use agere_api::AuthProvider;
use agere_client::Request;
use agere_client::RequestBody;
use agere_client::RequestCompression;
use agere_model_provider_info::ModelProviderAwsAuthInfo;
use agere_protocol::error::AgereErr;
use agere_protocol::error::Result;
use aws_credential_types::provider::ProvideCredentials;
use http::HeaderMap;

const BEDROCK_MANTLE_SERVICE_NAME: &str = "bedrock-mantle";
const BEDROCK_MANTLE_SUPPORTED_REGIONS: [&str; 12] = [
    "us-east-2",
    "us-east-1",
    "us-west-2",
    "ap-southeast-3",
    "ap-south-1",
    "ap-northeast-1",
    "eu-central-1",
    "eu-west-1",
    "eu-west-2",
    "eu-south-1",
    "eu-north-1",
    "sa-east-1",
];

/// Configuration for AWS auth (profile, region, service name).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AwsAuthConfig {
    pub(super) profile: Option<String>,
    pub(super) region: Option<String>,
    #[allow(dead_code)]
    pub(super) service: String,
}

/// Minimal error type for AWS auth operations.
#[derive(Debug)]
pub(super) struct AwsAuthError(String);

impl AwsAuthError {
    pub(super) fn is_retryable(&self) -> bool {
        self.0.contains("credential") || self.0.contains("timeout")
    }
}

impl std::fmt::Display for AwsAuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A request to be signed with AWS SigV4.
#[derive(Debug)]
pub(super) struct AwsRequestToSign {
    pub(super) method: http::Method,
    pub(super) url: String,
    pub(super) headers: HeaderMap,
    pub(super) body: Option<Vec<u8>>,
}

/// The result of signing an AWS request.
pub(super) struct AwsSignedRequest {
    pub(super) url: String,
    pub(super) headers: HeaderMap,
}

/// Holds loaded AWS credentials and provides SigV4 signing.
#[derive(Debug)]
pub(super) struct AwsAuthContext {
    credentials: aws_credential_types::Credentials,
    region: String,
}

impl AwsAuthContext {
    pub(super) async fn load(config: AwsAuthConfig) -> std::result::Result<Self, AwsAuthError> {
        let sdk_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;

        let credentials = match &config.profile {
            Some(profile) => {
                let loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
                    .profile_name(profile);
                let cfg = loader.load().await;
                cfg.credentials_provider()
                    .ok_or_else(|| AwsAuthError("no credentials provider for profile".to_string()))?
                    .provide_credentials()
                    .await
                    .map_err(|e| AwsAuthError(format!("failed to load credentials: {e}")))?
            }
            None => sdk_config
                .credentials_provider()
                .ok_or_else(|| AwsAuthError("no credentials provider".to_string()))?
                .provide_credentials()
                .await
                .map_err(|e| AwsAuthError(format!("failed to load credentials: {e}")))?,
        };

        let region = config.region.unwrap_or_else(|| {
            sdk_config
                .region()
                .map(|r| r.as_ref().to_string())
                .unwrap_or_else(|| "us-east-1".to_string())
        });

        Ok(Self {
            credentials,
            region,
        })
    }

    pub(super) fn region(&self) -> &str {
        &self.region
    }

    pub(super) async fn sign(
        &self,
        request: AwsRequestToSign,
    ) -> std::result::Result<AwsSignedRequest, AwsAuthError> {
        use aws_sigv4::http_request::SignableBody;
        use aws_sigv4::http_request::SignableRequest;
        use aws_sigv4::http_request::SigningSettings;
        use aws_sigv4::http_request::sign;
        use aws_sigv4::sign::v4;

        let body_bytes = request.body.unwrap_or_default();

        // Build an http::Request from the input so we can use apply_to_request_http1x
        let mut http_req = http::Request::builder()
            .method(&request.method)
            .uri(&request.url);

        for (key, value) in request.headers.iter() {
            if let Ok(val) = http::HeaderValue::from_bytes(value.as_bytes()) {
                http_req = http_req.header(key, val);
            }
        }

        let mut http_req = http_req
            .body(body_bytes.clone())
            .map_err(|e| AwsAuthError(format!("build HTTP request: {e}")))?;

        // Set up signing params
        let identity = self.credentials.clone().into();
        let signing_settings = SigningSettings::default();
        let signing_params: aws_sigv4::http_request::SigningParams<'_> =
            v4::SigningParams::builder()
                .identity(&identity)
                .region(&self.region)
                .name(BEDROCK_MANTLE_SERVICE_NAME)
                .time(std::time::SystemTime::now())
                .settings(signing_settings)
                .build()
                .map_err(|e| AwsAuthError(format!("build signing params: {e}")))?
                .into();

        // Create a signable request - collect headers into Vec for lifetime reasons
        let header_pairs: Vec<(String, String)> = request
            .headers
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    String::from_utf8_lossy(v.as_bytes()).into_owned(),
                )
            })
            .collect();

        let signable_request = SignableRequest::new(
            request.method.as_str(),
            &request.url,
            header_pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())),
            SignableBody::Bytes(&body_bytes),
        )
        .map_err(|e| AwsAuthError(format!("create signable request: {e}")))?;

        // Sign the request
        let signing_output = sign(signable_request, &signing_params)
            .map_err(|e| AwsAuthError(format!("sign request: {e}")))?;
        let (signing_instructions, _signature) = signing_output.into_parts();

        // Apply signature to the HTTP request
        signing_instructions.apply_to_request_http1x(&mut http_req);

        // Extract the signed URL and headers
        let url = http_req.uri().to_string();
        let headers = http_req.headers().clone();

        Ok(AwsSignedRequest { url, headers })
    }
}

pub(super) fn aws_auth_config(aws: &ModelProviderAwsAuthInfo) -> AwsAuthConfig {
    AwsAuthConfig {
        profile: aws.profile.clone(),
        region: region_from_config(aws),
        service: BEDROCK_MANTLE_SERVICE_NAME.to_string(),
    }
}

pub(super) fn region_from_config(aws: &ModelProviderAwsAuthInfo) -> Option<String> {
    aws.region
        .as_deref()
        .map(str::trim)
        .filter(|region| !region.is_empty())
        .map(str::to_string)
}

pub(super) fn base_url(region: &str) -> Result<String> {
    if BEDROCK_MANTLE_SUPPORTED_REGIONS.contains(&region) {
        Ok(format!("https://bedrock-mantle.{region}.api.aws/openai/v1"))
    } else {
        Err(AgereErr::Fatal(format!(
            "Amazon Bedrock Mantle does not support region `{region}`"
        )))
    }
}

pub(super) async fn resolve_provider_auth(
    aws: &ModelProviderAwsAuthInfo,
) -> Result<std::sync::Arc<dyn AuthProvider>> {
    const AWS_BEARER_TOKEN_BEDROCK_ENV_VAR: &str = "AWS_BEARER_TOKEN_BEDROCK";

    // Check for env bearer token first
    if let Some(token) = std::env::var(AWS_BEARER_TOKEN_BEDROCK_ENV_VAR)
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
    {
        let _region = region_from_config(aws).ok_or_else(|| {
            AgereErr::Fatal(
                "Amazon Bedrock bearer token auth requires \
`model_providers.amazon-bedrock.aws.region`"
                    .to_string(),
            )
        })?;
        use crate::BearerAuthProvider;
        return Ok(std::sync::Arc::new(BearerAuthProvider {
            token: Some(token),
            account_id: None,
            is_fedramp_account: false,
        }));
    }

    // Load AWS SDK auth
    let config = aws_auth_config(aws);
    let context = AwsAuthContext::load(config)
        .await
        .map_err(|e| AgereErr::Fatal(format!("failed to resolve Amazon Bedrock auth: {e}")))?;

    Ok(std::sync::Arc::new(BedrockMantleSigV4AuthProvider {
        context,
    }))
}

pub(super) async fn resolve_region(aws: &ModelProviderAwsAuthInfo) -> Result<String> {
    const AWS_BEARER_TOKEN_BEDROCK_ENV_VAR: &str = "AWS_BEARER_TOKEN_BEDROCK";

    // Check for env bearer token first
    if std::env::var(AWS_BEARER_TOKEN_BEDROCK_ENV_VAR)
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .is_some()
    {
        return region_from_config(aws).ok_or_else(|| {
            AgereErr::Fatal(
                "Amazon Bedrock bearer token auth requires \
`model_providers.amazon-bedrock.aws.region`"
                    .to_string(),
            )
        });
    }

    // Load AWS SDK auth to get region
    let config = aws_auth_config(aws);
    let context = AwsAuthContext::load(config)
        .await
        .map_err(|e| AgereErr::Fatal(format!("failed to resolve Amazon Bedrock auth: {e}")))?;
    Ok(context.region().to_string())
}

fn remove_headers_not_preserved_by_bedrock_mantle(headers: &mut HeaderMap) {
    const LEGACY_SESSION_ID_HEADER: &str = "session_id";
    headers.remove(LEGACY_SESSION_ID_HEADER);
}

/// AWS SigV4 auth provider for Bedrock Mantle OpenAI-compatible requests.
#[derive(Debug)]
struct BedrockMantleSigV4AuthProvider {
    context: AwsAuthContext,
}

#[async_trait::async_trait]
impl agere_api::AuthProvider for BedrockMantleSigV4AuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}

    async fn apply_auth(
        &self,
        request: Request,
    ) -> std::result::Result<Request, agere_api::AuthError> {
        use agere_api::AuthError;

        let mut request = request;
        remove_headers_not_preserved_by_bedrock_mantle(&mut request.headers);
        let prepared = request.prepare_body_for_send().map_err(AuthError::Build)?;
        let signed = self
            .context
            .sign(AwsRequestToSign {
                method: request.method.clone(),
                url: request.url.clone(),
                headers: prepared.headers.clone(),
                body: Some(prepared.body_bytes().to_vec()),
            })
            .await
            .map_err(|e| {
                if e.is_retryable() {
                    AuthError::Transient(e.to_string())
                } else {
                    AuthError::Build(e.to_string())
                }
            })?;

        request.url = signed.url;
        request.headers = signed.headers;
        request.body = prepared.body.map(RequestBody::Raw);
        request.compression = RequestCompression::None;
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn base_url_uses_region_endpoint() {
        assert_eq!(
            base_url("ap-northeast-1").expect("supported region"),
            "https://bedrock-mantle.ap-northeast-1.api.aws/openai/v1"
        );
    }

    #[test]
    fn base_url_rejects_unsupported_region() {
        let err = base_url("us-west-1").expect_err("unsupported region");

        assert_eq!(
            err.to_string(),
            "Fatal error: Amazon Bedrock Mantle does not support region `us-west-1`"
        );
    }

    #[test]
    fn aws_auth_config_uses_profile_and_mantle_service() {
        assert_eq!(
            aws_auth_config(&ModelProviderAwsAuthInfo {
                profile: Some("agere-bedrock".to_string()),
                region: None,
            }),
            AwsAuthConfig {
                profile: Some("agere-bedrock".to_string()),
                region: None,
                service: "bedrock-mantle".to_string(),
            }
        );
    }

    #[test]
    fn aws_auth_config_uses_configured_region() {
        assert_eq!(
            aws_auth_config(&ModelProviderAwsAuthInfo {
                profile: None,
                region: Some(" us-west-2 ".to_string()),
            }),
            AwsAuthConfig {
                profile: None,
                region: Some("us-west-2".to_string()),
                service: "bedrock-mantle".to_string(),
            }
        );
    }
}
