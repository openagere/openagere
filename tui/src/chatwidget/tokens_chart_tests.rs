use super::*;
use agere_app_server_protocol::GetProviderUsageResponse;
use agere_app_server_protocol::ProviderUsageDailyBucket;
use agere_app_server_protocol::ProviderUsageSummary;
use agere_app_server_protocol::ProviderUsageTotal;
use insta::assert_snapshot;
use pretty_assertions::assert_eq;

fn make_daily_bucket(date: &str, total_tokens: i64) -> ProviderUsageDailyBucket {
    ProviderUsageDailyBucket {
        date: date.to_string(),
        total_tokens,
        input_tokens: 0,
        cached_input_tokens: 0,
        output_tokens: 0,
        reasoning_output_tokens: 0,
    }
}

fn make_response(
    total_tokens: i64,
    peak_daily_tokens: i64,
    daily_buckets: Vec<ProviderUsageDailyBucket>,
    providers: Vec<ProviderUsageSummary>,
) -> GetProviderUsageResponse {
    GetProviderUsageResponse {
        total: ProviderUsageTotal {
            total_tokens,
            input_tokens: 0,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            peak_daily_tokens,
            longest_running_turn_sec: None,
            daily_buckets,
        },
        providers,
    }
}

#[test]
fn duplicate_dates_sum_and_negative_values_clamp() {
    let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("valid date");
    let buckets = vec![
        make_daily_bucket("2026-05-29", 10),
        make_daily_bucket("2026-05-29", 5),
        make_daily_bucket("2026-05-28", -4),
    ];
    let data = UsageData {
        response: make_response(15, 10, buckets, vec![]),
    };

    let values = daily_values(&data.daily_totals(), today);

    assert_eq!(values.iter().sum::<i64>(), 15);
}

#[test]
fn bar_levels_fill_from_bottom() {
    let levels = bar_levels(&[0, 10]);

    assert_eq!(&levels[..DAY_COUNT], &[0; DAY_COUNT]);
    assert_eq!(&levels[DAY_COUNT..], &[4; DAY_COUNT]);
}

#[test]
fn token_activity_view_aliases_parse() {
    assert_eq!(TokenActivityView::parse(""), Some(TokenActivityView::Daily));
    assert_eq!(
        TokenActivityView::parse("day"),
        Some(TokenActivityView::Daily)
    );
    assert_eq!(
        TokenActivityView::parse("week"),
        Some(TokenActivityView::Weekly)
    );
    assert_eq!(
        TokenActivityView::parse("cumulative"),
        Some(TokenActivityView::Cumulative)
    );
    assert_eq!(TokenActivityView::parse("year"), None);
}

#[test]
fn daily_graph_snapshot_uses_distinct_empty_and_active_cells() {
    let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("valid date");
    let buckets = vec![
        make_daily_bucket("2026-05-25", 1),
        make_daily_bucket("2026-05-29", 4),
    ];
    let data = UsageData {
        response: make_response(5, 4, buckets, vec![]),
    };

    let rendered = chart_lines(TokenActivityView::Daily, &data.daily_totals(), today, 22)
        .into_iter()
        .map(|line| line.to_string().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert_snapshot!(rendered, @"
         Apr     May
    Su · · · · · · · · ·
    Mo · · · · · · · · ░
    Tu · · · · · · · · ·
    We · · · · · · · · ·
    Th · · · · · · · · ·
    Fr · · · · · · · · █
    Sa · · · · · · · ·

     Less · ░ ▒ ▓ █ More
     [daily] · weekly · cumulative
    ");
}

#[test]
fn daily_graph_snapshot_stays_left_aligned_in_wide_terminal() {
    assert_eq!(graph_width(160), 107);
    assert_eq!(graph_width(u16::MAX), u16::MAX);

    let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("valid date");
    let lines = chart_lines(TokenActivityView::Daily, &[], today, 160);
    let rendered = [&lines[0], &lines[1], lines.last().expect("legend line")]
        .into_iter()
        .map(|line| line.to_string().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert_snapshot!(rendered, @"
       Jun       Jul     Aug       Sep     Oct     Nov       Dec     Jan     Feb     Mar       Apr     May
    Su · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · ·
     [daily] · weekly · cumulative
    ");
}

#[test]
fn weekly_graph_snapshot_renders_bar_chart_and_caption() {
    let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("valid date");
    let buckets = vec![
        make_daily_bucket("2026-05-11", 3),
        make_daily_bucket("2026-05-18", 6),
        make_daily_bucket("2026-05-25", 9),
    ];
    let data = UsageData {
        response: make_response(18, 9, buckets, vec![]),
    };

    let rendered = chart_lines(TokenActivityView::Weekly, &data.daily_totals(), today, 22)
        .into_iter()
        .map(|line| line.to_string().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert_snapshot!(rendered, @"
          Apr     May
    max                 █
                        █
                      █ █
                      █ █
                    █ █ █
                    █ █ █
      0             █ █ █

       Each column = 1 week · tallest 9
      daily · [weekly] · cumulative
    ");
}

#[test]
fn cumulative_graph_snapshot_renders_running_total_bar_chart_and_caption() {
    let today = NaiveDate::from_ymd_opt(2026, 5, 29).expect("valid date");
    let buckets = vec![
        make_daily_bucket("2026-05-11", 3),
        make_daily_bucket("2026-05-18", 6),
        make_daily_bucket("2026-05-25", 9),
    ];
    let data = UsageData {
        response: make_response(18, 9, buckets, vec![]),
    };

    let rendered = chart_lines(
        TokenActivityView::Cumulative,
        &data.daily_totals(),
        today,
        22,
    )
    .into_iter()
    .map(|line| line.to_string().trim_end().to_string())
    .collect::<Vec<_>>()
    .join("\n");

    assert_snapshot!(rendered, @"
          Apr     May
    max                 █
                        █
                        █
                      █ █
                      █ █
                    █ █ █
      0             █ █ █

       Running total · top 18
      daily · weekly · [cumulative]
    ");
}

#[test]
fn summary_snapshot_left_aligns_and_splits_when_needed() {
    let provider = ProviderUsageSummary {
        provider_id: "openai".to_string(),
        total_tokens: 100_000,
        input_tokens: 0,
        cached_input_tokens: 0,
        output_tokens: 0,
        reasoning_output_tokens: 0,
        peak_daily_tokens: 50_000,
        daily_buckets: vec![],
    };
    let data = UsageData {
        response: make_response(200_000, 80_000, vec![], vec![provider]),
    };
    let rendered = |width| {
        summary_lines(&data, graph_width(width))
            .into_iter()
            .map(|line| line.to_string().trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    };

    assert_snapshot!(
        format!(
            "wide:\n{}\n\nnarrow:\n{}",
            rendered(120),
            rendered(40)
        ),
        @"
    wide:
      Total 200K   ·   Peak 80K   ·   Streak -   ·   Longest task -

     Providers
      openai  ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓  100.0K  100.0%

    narrow:
      Total         200K
      Peak          80K
      Streak        -
      Longest task  -

     Providers
      openai  ▓▓▓▓▓▓▓▓▓▓▓▓▓  100.0K  100.0%
    "
    );
}

#[test]
fn provider_summary_lines_do_not_exceed_available_width() {
    let provider = ProviderUsageSummary {
        provider_id: "very-long-provider-name".to_string(),
        total_tokens: 100_000,
        input_tokens: 0,
        cached_input_tokens: 0,
        output_tokens: 0,
        reasoning_output_tokens: 0,
        peak_daily_tokens: 50_000,
        daily_buckets: vec![],
    };
    let data = UsageData {
        response: make_response(200_000, 80_000, vec![], vec![provider]),
    };
    let width = graph_width(40);

    let lines = summary_lines(&data, width);

    for line in lines {
        assert!(
            line.width() <= usize::from(width),
            "line exceeds width {width}: {:?}",
            line.to_string()
        );
    }
}

#[test]
fn provider_summary_left_aligns_mixed_width_names() {
    let providers = vec![
        ProviderUsageSummary {
            provider_id: "Alibaba Coding Plan 阿里云".to_string(),
            total_tokens: 228_100_000,
            input_tokens: 0,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            peak_daily_tokens: 122_000_000,
            daily_buckets: vec![],
        },
        ProviderUsageSummary {
            provider_id: "火山coding plan".to_string(),
            total_tokens: 9_300_000,
            input_tokens: 0,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            peak_daily_tokens: 9_300_000,
            daily_buckets: vec![],
        },
    ];
    let data = UsageData {
        response: make_response(237_400_000, 122_000_000, vec![], providers),
    };

    let rendered = summary_lines(&data, graph_width(120))
        .into_iter()
        .map(|line| line.to_string().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert_snapshot!(rendered, @"
     Total 237M   ·   Peak 122M   ·   Streak -   ·   Longest task -

    Providers
     Alibaba Coding Plan 阿里云  ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░  228.1M  96.1%
     火山coding plan             ▓░░░░░░░░░░░░░░░░░░░    9.3M  3.9%
    ");
}

#[test]
fn loaded_lines_separate_title_summary_and_providers() {
    let today = NaiveDate::from_ymd_opt(2026, 6, 23).expect("valid date");
    let provider = ProviderUsageSummary {
        provider_id: "Alibaba Coding Plan 阿里云".to_string(),
        total_tokens: 237_000_000,
        input_tokens: 0,
        cached_input_tokens: 0,
        output_tokens: 0,
        reasoning_output_tokens: 0,
        peak_daily_tokens: 122_000_000,
        daily_buckets: vec![],
    };
    let buckets = vec![make_daily_bucket("2026-06-23", 237_000_000)];
    let mut response = make_response(237_000_000, 122_000_000, buckets, vec![provider]);
    response.total.longest_running_turn_sec = Some(3920);
    let data = UsageData { response };

    let rendered = loaded_lines(TokenActivityView::Daily, &data, today, 120)
        .into_iter()
        .take(14)
        .map(|line| line.to_string().trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n");

    assert_snapshot!(rendered, @"
    Token activity   last 12 months

     Total 237M   ·   Peak 122M   ·   Streak 1d   ·   Longest task 1h 5m 20s

    Providers
     Alibaba Coding Plan 阿里云  ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓  237.0M  100.0%

         Jul     Aug       Sep     Oct     Nov       Dec     Jan     Feb     Mar       Apr     May       Jun
    Su · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · ·
    Mo · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · ·
    Tu · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · █
    We · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · ·
    Th · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · ·
    Fr · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · · ·
    ");
}

#[test]
fn metric_summary_lines_do_not_exceed_available_width() {
    let today = chrono::Utc::now().date_naive();
    let old_run_start = today - Duration::days(500);
    let mut daily_buckets = (0..365)
        .map(|offset| {
            let date = old_run_start + Duration::days(offset);
            make_daily_bucket(&date.format("%Y-%m-%d").to_string(), 1)
        })
        .collect::<Vec<_>>();
    daily_buckets.extend((0..12).map(|offset| {
        make_daily_bucket(
            &(today - Duration::days(11 - offset))
                .format("%Y-%m-%d")
                .to_string(),
            1,
        )
    }));
    let mut response = make_response(1_000_000, 800_000, daily_buckets, vec![]);
    response.total.longest_running_turn_sec = Some(3661);
    let data = UsageData { response };
    let width = graph_width(52);

    let lines = summary_lines(&data, width);

    for line in lines {
        assert!(
            line.width() <= usize::from(width),
            "line exceeds width {width}: {:?}",
            line.to_string()
        );
    }
}

#[test]
fn compute_streaks_counts_consecutive_active_days() {
    let today = chrono::Utc::now().date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();
    let yesterday_str = (today - Duration::days(1)).format("%Y-%m-%d").to_string();
    let two_days_ago_str = (today - Duration::days(2)).format("%Y-%m-%d").to_string();

    let daily_totals = vec![
        (two_days_ago_str, 100),
        (yesterday_str, 200),
        (today_str, 300),
    ];
    let streaks = compute_streaks(&daily_totals);
    assert_eq!(streaks.current, 3);
    assert_eq!(streaks.longest, 3);
}

#[test]
fn compute_streaks_resets_on_gap() {
    let today = chrono::Utc::now().date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();
    let five_days_ago_str = (today - Duration::days(5)).format("%Y-%m-%d").to_string();
    let six_days_ago_str = (today - Duration::days(6)).format("%Y-%m-%d").to_string();
    let seven_days_ago_str = (today - Duration::days(7)).format("%Y-%m-%d").to_string();

    let daily_totals = vec![
        (seven_days_ago_str, 100),
        (six_days_ago_str, 200),
        (five_days_ago_str, 300),
        (today_str, 50),
    ];
    let streaks = compute_streaks(&daily_totals);
    assert_eq!(streaks.current, 1);
    assert_eq!(streaks.longest, 3);
}

#[test]
fn format_optional_duration_formats_human_readable() {
    assert_eq!(format_optional_duration(None), "-");
    assert_eq!(format_optional_duration(Some(0)), "-");
    assert_eq!(format_optional_duration(Some(45)), "45s");
    assert_eq!(format_optional_duration(Some(720)), "12m");
    assert_eq!(format_optional_duration(Some(13920)), "3h 52m");
    assert_eq!(format_optional_duration(Some(3661)), "1h 1m 1s");
}

#[test]
fn format_streak_matches_codex_behavior() {
    assert_eq!(format_streak(0, 0), "-");
    assert_eq!(format_streak(54, 54), "54d");
    assert_eq!(format_streak(12, 54), "12d (best 54d)");
}
