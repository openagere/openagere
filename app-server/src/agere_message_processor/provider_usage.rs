use agere_app_server_protocol::GetProviderUsageResponse;
use agere_app_server_protocol::ProviderUsageDailyBucket;
use agere_app_server_protocol::ProviderUsageSummary;
use agere_app_server_protocol::ProviderUsageTotal;
use std::collections::BTreeMap;

pub(super) fn build_provider_usage_response(
    records: &[agere_state::UsageRecord],
) -> GetProviderUsageResponse {
    let mut by_provider: BTreeMap<String, Vec<&agere_state::UsageRecord>> = BTreeMap::new();
    for record in records {
        by_provider
            .entry(record.provider_id.clone())
            .or_default()
            .push(record);
    }

    let mut providers = Vec::new();
    let mut global_daily: BTreeMap<String, ProviderUsageDailyBucket> = BTreeMap::new();
    let mut grand_total_tokens: i64 = 0;
    let mut grand_input_tokens: i64 = 0;
    let mut grand_cached_input_tokens: i64 = 0;
    let mut grand_output_tokens: i64 = 0;
    let mut grand_reasoning_output_tokens: i64 = 0;

    for (provider_id, provider_records) in by_provider {
        let mut total_tokens: i64 = 0;
        let mut input_tokens: i64 = 0;
        let mut cached_input_tokens: i64 = 0;
        let mut output_tokens: i64 = 0;
        let mut reasoning_output_tokens: i64 = 0;
        let mut peak_daily: i64 = 0;
        let mut daily_buckets = Vec::new();

        for record in provider_records {
            total_tokens += record.total_tokens;
            input_tokens += record.input_tokens;
            cached_input_tokens += record.cached_input_tokens;
            output_tokens += record.output_tokens;
            reasoning_output_tokens += record.reasoning_output_tokens;
            if record.total_tokens > peak_daily {
                peak_daily = record.total_tokens;
            }
            daily_buckets.push(ProviderUsageDailyBucket {
                date: record.date.clone(),
                total_tokens: record.total_tokens,
                input_tokens: record.input_tokens,
                cached_input_tokens: record.cached_input_tokens,
                output_tokens: record.output_tokens,
                reasoning_output_tokens: record.reasoning_output_tokens,
            });

            let entry = global_daily.entry(record.date.clone()).or_insert_with(|| {
                ProviderUsageDailyBucket {
                    date: record.date.clone(),
                    total_tokens: 0,
                    input_tokens: 0,
                    cached_input_tokens: 0,
                    output_tokens: 0,
                    reasoning_output_tokens: 0,
                }
            });
            entry.total_tokens += record.total_tokens;
            entry.input_tokens += record.input_tokens;
            entry.cached_input_tokens += record.cached_input_tokens;
            entry.output_tokens += record.output_tokens;
            entry.reasoning_output_tokens += record.reasoning_output_tokens;
        }

        grand_total_tokens += total_tokens;
        grand_input_tokens += input_tokens;
        grand_cached_input_tokens += cached_input_tokens;
        grand_output_tokens += output_tokens;
        grand_reasoning_output_tokens += reasoning_output_tokens;

        providers.push(ProviderUsageSummary {
            provider_id,
            total_tokens,
            input_tokens,
            cached_input_tokens,
            output_tokens,
            reasoning_output_tokens,
            peak_daily_tokens: peak_daily,
            daily_buckets,
        });
    }

    let global_daily_buckets: Vec<ProviderUsageDailyBucket> = global_daily.into_values().collect();
    let grand_peak_daily = global_daily_buckets
        .iter()
        .map(|bucket| bucket.total_tokens)
        .max()
        .unwrap_or(0);
    let longest_turn = records
        .iter()
        .map(|record| record.max_turn_duration_sec)
        .filter(|duration| *duration > 0)
        .max();

    GetProviderUsageResponse {
        providers,
        total: ProviderUsageTotal {
            total_tokens: grand_total_tokens,
            input_tokens: grand_input_tokens,
            cached_input_tokens: grand_cached_input_tokens,
            output_tokens: grand_output_tokens,
            reasoning_output_tokens: grand_reasoning_output_tokens,
            peak_daily_tokens: grand_peak_daily,
            longest_running_turn_sec: longest_turn,
            daily_buckets: global_daily_buckets,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn build_provider_usage_response_sorts_provider_summaries_by_id() {
        let records = vec![
            usage_record("openai", "2026-06-20", 20),
            usage_record("anthropic", "2026-06-20", 10),
        ];

        let response = build_provider_usage_response(&records);

        let provider_ids: Vec<_> = response
            .providers
            .iter()
            .map(|provider| provider.provider_id.as_str())
            .collect();
        assert_eq!(provider_ids, vec!["anthropic", "openai"]);
    }

    fn usage_record(provider_id: &str, date: &str, total_tokens: i64) -> agere_state::UsageRecord {
        agere_state::UsageRecord {
            provider_id: provider_id.to_string(),
            date: date.to_string(),
            total_tokens,
            input_tokens: total_tokens,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            max_turn_duration_sec: 0,
        }
    }
}
