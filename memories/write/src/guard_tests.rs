use super::*;

#[tokio::test]
async fn rate_limits_ok_always_returns_true() {
    // The simplified guard always allows startup.
    // Note: Config no longer implements Default, so we build a minimal one.
    let auth_manager =
        AuthManager::from_auth_for_testing(agere_login::AgereAuth::from_api_key("test-key"));
    let config = build_minimal_test_config().await;
    assert!(rate_limits_ok(&auth_manager, &config).await);
}

async fn build_minimal_test_config() -> Config {
    use agere_config::CloudRequirementsLoader;
    use agere_config::LoaderOverrides;
    use agere_core::config::ConfigBuilder;
    ConfigBuilder::default()
        .loader_overrides(LoaderOverrides::default())
        .cloud_requirements(CloudRequirementsLoader::default())
        .build()
        .await
        .expect("minimal config should build")
}
