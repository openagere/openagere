use super::*;
use pretty_assertions::assert_eq;

#[test]
fn deserialize_skill_config_with_name_selector() {
    let cfg: SkillConfig = toml::from_str(
        r#"
            name = "github:yeet"
            enabled = false
        "#,
    )
    .expect("should deserialize skill config with name selector");

    assert_eq!(cfg.name.as_deref(), Some("github:yeet"));
    assert_eq!(cfg.path, None);
    assert!(!cfg.enabled);
}

#[test]
fn deserialize_skill_config_with_path_selector() {
    let tempdir = tempfile::tempdir().expect("tempdir");
    let skill_path = tempdir.path().join("skills").join("demo").join("SKILL.md");
    let cfg: SkillConfig = toml::from_str(&format!(
        r#"
            path = {path:?}
            enabled = false
        "#,
        path = skill_path.display().to_string(),
    ))
    .expect("should deserialize skill config with path selector");

    assert_eq!(
        cfg,
        SkillConfig {
            path: Some(
                AbsolutePathBuf::from_absolute_path(&skill_path)
                    .expect("skill path should be absolute"),
            ),
            name: None,
            enabled: false,
        }
    );
}

#[test]
fn memories_config_clamps_count_limits_to_nonzero_values() {
    let config = MemoriesConfig::from(MemoriesToml {
        max_raw_memories_for_consolidation: Some(0),
        max_rollouts_per_startup: Some(0),
        ..Default::default()
    });

    assert_eq!(
        config,
        MemoriesConfig {
            max_raw_memories_for_consolidation: 1,
            max_rollouts_per_startup: 1,
            ..MemoriesConfig::default()
        }
    );
}

#[test]
fn memories_config_clamps_rate_limit_remaining_threshold() {
    let config = MemoriesConfig::from(MemoriesToml {
        min_rate_limit_remaining_percent: Some(101),
        ..Default::default()
    });
    assert_eq!(
        config,
        MemoriesConfig {
            min_rate_limit_remaining_percent: 100,
            ..MemoriesConfig::default()
        }
    );

    let config = MemoriesConfig::from(MemoriesToml {
        min_rate_limit_remaining_percent: Some(-1),
        ..Default::default()
    });
    assert_eq!(
        config,
        MemoriesConfig {
            min_rate_limit_remaining_percent: 0,
            ..MemoriesConfig::default()
        }
    );
}
#[test]
fn rate_limit_retry_defaults_use_five_hour_slow_retry_ceiling() {
    assert_eq!(
        RateLimitRetryConfig::default(),
        RateLimitRetryConfig {
            enabled: true,
            max_attempts: 32,
            delays_secs: vec![60, 120, 300, 600],
            respect_resets_at: false,
            cap_secs: 600,
            trigger_after_consecutive: 1,
        }
    );
}

#[test]
fn rate_limit_retry_default_max_attempts_stays_under_five_hours() {
    let cfg = RateLimitRetryConfig::default();
    let total_wait_secs = (0..cfg.max_attempts)
        .map(|attempt| cfg.delay_secs_for_attempt(attempt))
        .sum::<u64>();

    assert_eq!(total_wait_secs, 17_880);
    assert!(total_wait_secs < 5 * 60 * 60);
    assert!(total_wait_secs + cfg.delay_secs_for_attempt(cfg.max_attempts) > 5 * 60 * 60);
}

#[test]
fn rate_limit_retry_schedule_reuses_last_delay() {
    let cfg = RateLimitRetryConfig::default();

    assert_eq!(cfg.delay_secs_for_attempt(0), 60);
    assert_eq!(cfg.delay_secs_for_attempt(1), 120);
    assert_eq!(cfg.delay_secs_for_attempt(2), 300);
    assert_eq!(cfg.delay_secs_for_attempt(3), 600);
    assert_eq!(cfg.delay_secs_for_attempt(99), 600);
}

#[test]
fn rate_limit_retry_toml_clamps_custom_delays_to_cap() {
    let cfg = RateLimitRetryConfig::from(RateLimitRetryToml {
        cap_secs: Some(600),
        delays_secs: Some(vec![30, 900]),
        ..RateLimitRetryToml::default()
    });

    assert_eq!(cfg.delay_secs_for_attempt(0), 30);
    assert_eq!(cfg.delay_secs_for_attempt(1), 600);
    assert_eq!(cfg.delay_secs_for_attempt(10), 600);
}
