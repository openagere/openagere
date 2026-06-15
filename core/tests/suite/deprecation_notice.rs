#![cfg(not(target_os = "windows"))]

use agere_app_server_protocol::ConfigLayerSource;
use agere_config::ConfigLayerEntry;
use agere_config::ConfigLayerStack;
use agere_config::ConfigRequirements;
use agere_config::ConfigRequirementsToml;
use agere_features::Feature;
use agere_protocol::protocol::DeprecationNoticeEvent;
use agere_protocol::protocol::EventMsg;
use anyhow::Ok;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_absolute_path;
use core_test_support::test_agere::TestAgere;
use core_test_support::test_agere::test_agere;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use toml::Value as TomlValue;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_deprecation_notice_for_legacy_feature_flag() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_agere().with_config(|config| {
        let mut features = config.features.get().clone();
        features.enable(Feature::UnifiedExec);
        features
            .record_legacy_usage_force("use_experimental_unified_exec_tool", Feature::UnifiedExec);
        config
            .features
            .set(features)
            .expect("test config should allow managed feature metadata updates");
        config.use_experimental_unified_exec_tool = true;
    });

    let TestAgere { agere, .. } = builder.build(&server).await?;

    let notice = wait_for_event_match(&agere, |event| match event {
        EventMsg::DeprecationNotice(ev) => Some(ev.clone()),
        _ => None,
    })
    .await;

    let DeprecationNoticeEvent { summary, details } = notice;
    assert_eq!(
        summary,
        "`[features].use_experimental_unified_exec_tool` is deprecated. Use `[features].unified_exec` instead.".to_string(),
    );
    assert_eq!(
        details.as_deref(),
        Some(
            "Enable it with `--enable unified_exec` or `[features].unified_exec` in config.toml. See https://developers.openagere.com/agere/config-basic#feature-flags for details."
        ),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_deprecation_notice_for_experimental_instructions_file() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_agere().with_config(|config| {
        let mut table = toml::map::Map::new();
        table.insert(
            "experimental_instructions_file".to_string(),
            TomlValue::String("legacy.md".to_string()),
        );
        let config_layer = ConfigLayerEntry::new(
            ConfigLayerSource::User {
                file: test_absolute_path("/tmp/config.toml"),
            },
            TomlValue::Table(table),
        );
        let config_layer_stack = ConfigLayerStack::new(
            vec![config_layer],
            ConfigRequirements::default(),
            ConfigRequirementsToml::default(),
        )
        .expect("build config layer stack");
        config.config_layer_stack = config_layer_stack;
    });

    let TestAgere { agere, .. } = builder.build(&server).await?;

    let notice = wait_for_event_match(&agere, |event| match event {
        EventMsg::DeprecationNotice(ev)
            if ev.summary.contains("experimental_instructions_file") =>
        {
            Some(ev.clone())
        }
        _ => None,
    })
    .await;

    let DeprecationNoticeEvent { summary, details } = notice;
    assert_eq!(
        summary,
        "`experimental_instructions_file` is deprecated and ignored. Use `model_instructions_file` instead."
            .to_string(),
    );
    assert_eq!(
        details.as_deref(),
        Some(
            "Move the setting to `model_instructions_file` in config.toml (or under a profile) to load instructions from a file."
        ),
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_deprecation_notice_for_web_search_feature_flag_values() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    for enabled in [true, false] {
        let server = start_mock_server().await;

        let mut builder = test_agere().with_config(move |config| {
            let mut entries = BTreeMap::new();
            entries.insert("web_search_request".to_string(), enabled);
            let mut features = config.features.get().clone();
            features.apply_map(&entries);
            config
                .features
                .set(features)
                .expect("test config should allow managed feature map updates");
        });

        let TestAgere { agere, .. } = builder.build(&server).await?;

        let notice = wait_for_event_match(&agere, |event| match event {
            EventMsg::DeprecationNotice(ev)
                if ev.summary.contains("[features].web_search_request") =>
            {
                Some(ev.clone())
            }
            _ => None,
        })
        .await;

        let DeprecationNoticeEvent { summary, details } = notice;
        assert_eq!(
            summary,
            "`[features].web_search_request` is deprecated because web search is enabled by default."
                .to_string(),
        );
        assert_eq!(
            details.as_deref(),
            Some(
                "Set `web_search` to `\"live\"`, `\"cached\"`, or `\"disabled\"` at the top level (or under a profile) in config.toml if you want to override it."
            ),
        );
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn emits_deprecation_notice_for_use_legacy_landlock() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_agere().with_config(|config| {
        let mut entries = BTreeMap::new();
        entries.insert("use_legacy_landlock".to_string(), true);
        let mut features = config.features.get().clone();
        features.apply_map(&entries);
        config
            .features
            .set(features)
            .expect("test config should allow managed feature map updates");
    });

    let TestAgere { agere, .. } = builder.build(&server).await?;

    let notice = wait_for_event_match(&agere, |event| match event {
        EventMsg::DeprecationNotice(ev)
            if ev.summary.contains("[features].use_legacy_landlock") =>
        {
            Some(ev.clone())
        }
        _ => None,
    })
    .await;

    let DeprecationNoticeEvent { summary, details } = notice;
    assert_eq!(
        summary,
        "`[features].use_legacy_landlock` is deprecated and will be removed soon.".to_string(),
    );
    assert_eq!(
        details.as_deref(),
        Some("Remove this setting to stop opting into the legacy Linux Landlock isolation path."),
    );

    Ok(())
}
