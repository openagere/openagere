use super::*;
use agere_utils_fs::AbsolutePathBufGuard;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[test]
fn test_deserialize_ollama_model_provider_toml() {
    let azure_provider_toml = r#"
base_url = "http://localhost:11434/v1"
        "#;
    let expected_provider = ModelProviderInfo {
        base_url: Some("http://localhost:11434/v1".into()),
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        auth: None,
        aws: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        websocket_connect_timeout_ms: None,
        requires_provider_auth: false,
        supports_websockets: false,
    };

    let provider: ModelProviderInfo = toml::from_str(azure_provider_toml).unwrap();
    assert_eq!(expected_provider, provider);
}

#[test]
fn test_deserialize_azure_model_provider_toml() {
    let azure_provider_toml = r#"
base_url = "https://xxxxx.openai.azure.com/openai"
env_key = "AZURE_OPENAI_API_KEY"
query_params = { api-version = "2025-04-01-preview" }
        "#;
    let expected_provider = ModelProviderInfo {
        base_url: Some("https://xxxxx.openai.azure.com/openai".into()),
        env_key: Some("AZURE_OPENAI_API_KEY".into()),
        env_key_instructions: None,
        experimental_bearer_token: None,
        auth: None,
        aws: None,
        wire_api: WireApi::Responses,
        query_params: Some(maplit::hashmap! {
            "api-version".to_string() => "2025-04-01-preview".to_string(),
        }),
        http_headers: None,
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        websocket_connect_timeout_ms: None,
        requires_provider_auth: false,
        supports_websockets: false,
    };

    let provider: ModelProviderInfo = toml::from_str(azure_provider_toml).unwrap();
    assert_eq!(expected_provider, provider);
}

#[test]
fn test_deserialize_example_model_provider_toml() {
    let azure_provider_toml = r#"
base_url = "https://example.com"
env_key = "API_KEY"
http_headers = { "X-Example-Header" = "example-value" }
env_http_headers = { "X-Example-Env-Header" = "EXAMPLE_ENV_VAR" }
        "#;
    let expected_provider = ModelProviderInfo {
        base_url: Some("https://example.com".into()),
        env_key: Some("API_KEY".into()),
        env_key_instructions: None,
        experimental_bearer_token: None,
        auth: None,
        aws: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: Some(maplit::hashmap! {
            "X-Example-Header".to_string() => "example-value".to_string(),
        }),
        env_http_headers: Some(maplit::hashmap! {
            "X-Example-Env-Header".to_string() => "EXAMPLE_ENV_VAR".to_string(),
        }),
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        websocket_connect_timeout_ms: None,
        requires_provider_auth: false,
        supports_websockets: false,
    };

    let provider: ModelProviderInfo = toml::from_str(azure_provider_toml).unwrap();
    assert_eq!(expected_provider, provider);
}

#[test]
fn test_deserialize_chat_wire_api() {
    let provider_toml = r#"
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
wire_api = "chat"
        "#;

    let info = toml::from_str::<ModelProviderInfo>(provider_toml).unwrap();
    assert_eq!(info.wire_api, WireApi::Chat);
}

#[test]
fn test_deserialize_websocket_connect_timeout() {
    let provider_toml = r#"
base_url = "https://api.openai.com/v1"
websocket_connect_timeout_ms = 15000
supports_websockets = true
        "#;

    let provider: ModelProviderInfo = toml::from_str(provider_toml).unwrap();
    assert_eq!(provider.websocket_connect_timeout_ms, Some(15_000));
}

#[test]
fn test_supports_remote_compaction_for_openai() {
    let provider = ModelProviderInfo::create_openai_provider(/*base_url*/ None);

    assert!(provider.supports_remote_compaction());
}

#[test]
fn test_supports_remote_compaction_for_azure_name() {
    let provider = ModelProviderInfo {
        base_url: Some("https://example.openai.azure.com/openai/v1".into()),
        env_key: Some("AZURE_OPENAI_API_KEY".into()),
        env_key_instructions: None,
        experimental_bearer_token: None,
        auth: None,
        aws: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        websocket_connect_timeout_ms: None,
        requires_provider_auth: false,
        supports_websockets: false,
    };

    assert!(provider.supports_remote_compaction());
}

#[test]
fn test_supports_remote_compaction_for_non_openai_non_azure_provider() {
    let provider = ModelProviderInfo {
        base_url: Some("https://example.com/v1".into()),
        env_key: Some("API_KEY".into()),
        env_key_instructions: None,
        experimental_bearer_token: None,
        auth: None,
        aws: None,
        wire_api: WireApi::Responses,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        websocket_connect_timeout_ms: None,
        requires_provider_auth: false,
        supports_websockets: false,
    };

    assert!(!provider.supports_remote_compaction());
}

#[test]
fn test_deserialize_provider_auth_config_is_rejected() {
    let base_dir = tempdir().unwrap();
    let provider_toml = r#"
[auth]
command = "./scripts/print-token"
args = ["--format=text"]
        "#;

    let provider: ModelProviderInfo = {
        let _guard = AbsolutePathBufGuard::new(base_dir.path());
        toml::from_str(provider_toml).unwrap()
    };

    assert_eq!(
        provider.validate(),
        Err("provider auth is not supported; use env_key or experimental_bearer_token".to_string())
    );
}

#[test]
fn test_deserialize_provider_aws_config() {
    let provider_toml = r#"
base_url = "https://bedrock.example.com/v1"

[aws]
profile = "agere-bedrock"
region = "us-west-2"
        "#;

    let provider: ModelProviderInfo = toml::from_str(provider_toml).unwrap();

    assert_eq!(
        provider.aws,
        Some(ModelProviderAwsAuthInfo {
            profile: Some("agere-bedrock".to_string()),
            region: Some("us-west-2".to_string()),
        })
    );
}

#[test]
fn test_create_amazon_bedrock_provider() {
    assert_eq!(
        ModelProviderInfo::create_amazon_bedrock_provider(/*aws*/ None),
        ModelProviderInfo {
            base_url: Some("https://bedrock-mantle.us-east-1.api.aws/openai/v1".to_string()),
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            auth: None,
            aws: Some(ModelProviderAwsAuthInfo {
                profile: None,
                region: None,
            }),
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            websocket_connect_timeout_ms: None,
            requires_provider_auth: false,
            supports_websockets: false,
        }
    );
}

#[test]
fn test_built_in_model_providers_include_amazon_bedrock() {
    let providers = built_in_model_providers(/*openai_base_url*/ None);

    assert_eq!(
        providers
            .get(AMAZON_BEDROCK_PROVIDER_ID)
            .map(ModelProviderInfo::is_amazon_bedrock),
        Some(true)
    );
}

#[test]
fn test_merge_configured_model_providers_adds_custom_provider() {
    let custom_provider = ModelProviderInfo {
        base_url: Some("https://example.com/v1".to_string()),
        ..ModelProviderInfo::default()
    };
    let configured_model_providers =
        std::collections::HashMap::from([("custom".to_string(), custom_provider.clone())]);

    let mut expected = built_in_model_providers(/*openai_base_url*/ None);
    expected.insert("custom".to_string(), custom_provider);

    assert_eq!(
        merge_configured_model_providers(
            built_in_model_providers(/*openai_base_url*/ None),
            configured_model_providers,
        ),
        Ok(expected)
    );
}

#[test]
fn test_merge_configured_model_providers_applies_amazon_bedrock_profile_override() {
    let configured_model_providers = std::collections::HashMap::from([(
        AMAZON_BEDROCK_PROVIDER_ID.to_string(),
        ModelProviderInfo {
            aws: Some(ModelProviderAwsAuthInfo {
                profile: Some("agere-bedrock".to_string()),
                region: Some("us-west-2".to_string()),
            }),
            ..ModelProviderInfo::default()
        },
    )]);

    let mut expected = built_in_model_providers(/*openai_base_url*/ None);
    expected
        .get_mut(AMAZON_BEDROCK_PROVIDER_ID)
        .expect("Amazon Bedrock provider should be built in")
        .aws = Some(ModelProviderAwsAuthInfo {
        profile: Some("agere-bedrock".to_string()),
        region: Some("us-west-2".to_string()),
    });

    assert_eq!(
        merge_configured_model_providers(
            built_in_model_providers(/*openai_base_url*/ None),
            configured_model_providers,
        ),
        Ok(expected)
    );
}

#[test]
fn test_merge_configured_model_providers_rejects_amazon_bedrock_non_default_fields() {
    let configured_model_providers = std::collections::HashMap::from([(
        AMAZON_BEDROCK_PROVIDER_ID.to_string(),
        ModelProviderInfo {
            base_url: Some("https://custom.example.com".to_string()),
            aws: Some(ModelProviderAwsAuthInfo {
                profile: Some("agere-bedrock".to_string()),
                region: None,
            }),
            ..ModelProviderInfo::default()
        },
    )]);

    assert_eq!(
        merge_configured_model_providers(
            built_in_model_providers(/*openai_base_url*/ None),
            configured_model_providers,
        ),
        Err(
            "model_providers.amazon-bedrock only supports changing `aws.profile` and `aws.region`; other non-default provider fields are not supported"
                .to_string()
        )
    );
}

#[test]
fn test_merge_configured_model_providers_allows_amazon_bedrock_default_fields() {
    let configured_model_providers = std::collections::HashMap::from([(
        AMAZON_BEDROCK_PROVIDER_ID.to_string(),
        ModelProviderInfo {
            aws: Some(ModelProviderAwsAuthInfo {
                profile: None,
                region: None,
            }),
            wire_api: WireApi::Responses,
            ..ModelProviderInfo::default()
        },
    )]);

    assert_eq!(
        merge_configured_model_providers(
            built_in_model_providers(/*openai_base_url*/ None),
            configured_model_providers,
        ),
        Ok(built_in_model_providers(/*openai_base_url*/ None))
    );
}

#[test]
fn test_validate_provider_aws_rejects_conflicting_auth() {
    let provider = ModelProviderInfo {
        aws: Some(ModelProviderAwsAuthInfo {
            profile: None,
            region: None,
        }),
        env_key: Some("AWS_BEARER_TOKEN_BEDROCK".to_string()),
        supports_websockets: false,
        ..ModelProviderInfo::create_openai_provider(/*base_url*/ None)
    };

    assert_eq!(
        provider.validate(),
        Err("provider aws cannot be combined with env_key, requires_provider_auth".to_string())
    );
}

#[test]
fn test_validate_provider_aws_rejects_websockets() {
    let provider = ModelProviderInfo {
        aws: Some(ModelProviderAwsAuthInfo {
            profile: None,
            region: None,
        }),
        requires_provider_auth: false,
        supports_websockets: true,
        ..ModelProviderInfo::create_openai_provider(/*base_url*/ None)
    };

    assert_eq!(
        provider.validate(),
        Err("provider aws cannot be combined with supports_websockets".to_string())
    );
}

// ─── Thinking signature proxy detection ─────────────────────────────

#[test]
fn thinking_signature_downgrade_detects_openrouter() {
    assert!(requires_thinking_signature_downgrade(
        "https://openrouter.ai/api/v1"
    ));
    assert!(requires_thinking_signature_downgrade(
        "https://openrouter.ai/api/v1/"
    ));
}

#[test]
fn thinking_signature_downgrade_detects_deepseek() {
    assert!(requires_thinking_signature_downgrade(
        "https://api.deepseek.com/anthropic"
    ));
    assert!(requires_thinking_signature_downgrade(
        "https://api.deepseek.com/anthropic/"
    ));
}

#[test]
fn thinking_signature_downgrade_ignores_native_anthropic() {
    assert!(!requires_thinking_signature_downgrade(
        "https://api.anthropic.com"
    ));
    assert!(!requires_thinking_signature_downgrade(
        "https://api.anthropic.com/v1/messages"
    ));
}

#[test]
fn thinking_signature_downgrade_ignores_unrelated_urls() {
    assert!(!requires_thinking_signature_downgrade(
        "https://example.com/v1"
    ));
    assert!(!requires_thinking_signature_downgrade(
        "http://localhost:11434/v1"
    ));
}

#[test]
fn thinking_signature_downgrade_is_case_insensitive() {
    assert!(requires_thinking_signature_downgrade(
        "https://OPENROUTER.AI/API/V1"
    ));
    assert!(requires_thinking_signature_downgrade(
        "https://Api.DeepSeek.Com/Anthropic"
    ));
}

// ─── supports_apply_patch_freeform ───────────────────────────────────

#[test]
fn supports_apply_patch_freeform_openai() {
    let info = ModelProviderInfo {
        base_url: Some("https://api.openai.com/v1".to_string()),
        ..Default::default()
    };
    assert!(info.supports_apply_patch_freeform());
}

#[test]
fn supports_apply_patch_freeform_azure() {
    let info = ModelProviderInfo {
        base_url: Some("https://api.aiservice.ai.azure.com/openai".to_string()),
        ..Default::default()
    };
    assert!(info.supports_apply_patch_freeform());
}

#[test]
fn supports_apply_patch_freeform_openrouter_false() {
    let info = ModelProviderInfo {
        base_url: Some("https://openrouter.ai/api/v1".to_string()),
        ..Default::default()
    };
    assert!(!info.supports_apply_patch_freeform());
}

#[test]
fn supports_apply_patch_freeform_no_base_url() {
    let info = ModelProviderInfo {
        base_url: None,
        wire_api: WireApi::Anthropic,
        ..Default::default()
    };
    assert!(!info.supports_apply_patch_freeform());
}

#[test]
fn supports_apply_patch_freeform_case_insensitive() {
    let info = ModelProviderInfo {
        base_url: Some("https://API.OPENAI.COM/v1".to_string()),
        ..Default::default()
    };
    assert!(info.supports_apply_patch_freeform());
}

#[test]
fn supports_apply_patch_freeform_default_openai_provider() {
    // When base_url is None and wire_api is Responses (default), it should
    // be detected as the default OpenAI provider and return true.
    let info = ModelProviderInfo {
        base_url: None,
        ..Default::default()
    };
    assert!(info.supports_apply_patch_freeform());
}
