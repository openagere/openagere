use sqlx::Row;

/// A single record of token usage for a provider on a given date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageRecord {
    pub provider_id: String,
    pub date: String,
    pub total_tokens: i64,
    pub input_tokens: i64,
    pub cached_input_tokens: i64,
    pub output_tokens: i64,
    pub reasoning_output_tokens: i64,
    /// Wall-clock seconds of the longest turn recorded for this (provider, date).
    pub max_turn_duration_sec: i64,
}

impl super::StateRuntime {
    /// Record token usage for a given provider on today's date.
    /// Uses an upsert: if a record for (provider_id, date) already exists,
    /// the token counts are added to the existing values.
    pub async fn record_usage(&self, record: &UsageRecord) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO usage_records (provider_id, date, total_tokens, input_tokens, cached_input_tokens, output_tokens, reasoning_output_tokens, max_turn_duration_sec)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(provider_id, date) DO UPDATE SET
                total_tokens = total_tokens + excluded.total_tokens,
                input_tokens = input_tokens + excluded.input_tokens,
                cached_input_tokens = cached_input_tokens + excluded.cached_input_tokens,
                output_tokens = output_tokens + excluded.output_tokens,
                reasoning_output_tokens = reasoning_output_tokens + excluded.reasoning_output_tokens,
                max_turn_duration_sec = MAX(max_turn_duration_sec, excluded.max_turn_duration_sec)
            "#,
        )
        .bind(&record.provider_id)
        .bind(&record.date)
        .bind(record.total_tokens)
        .bind(record.input_tokens)
        .bind(record.cached_input_tokens)
        .bind(record.output_tokens)
        .bind(record.reasoning_output_tokens)
        .bind(record.max_turn_duration_sec)
        .execute(self.pool.as_ref())
        .await?;
        Ok(())
    }

    /// Query all usage records, returning them ordered by date.
    pub async fn query_all_usage_records(&self) -> anyhow::Result<Vec<UsageRecord>> {
        let rows = sqlx::query(
            "SELECT provider_id, date, total_tokens, input_tokens, cached_input_tokens, output_tokens, reasoning_output_tokens, max_turn_duration_sec
             FROM usage_records ORDER BY date ASC",
        )
        .map(|row: sqlx::sqlite::SqliteRow| UsageRecord {
            provider_id: row.get("provider_id"),
            date: row.get("date"),
            total_tokens: row.get("total_tokens"),
            input_tokens: row.get("input_tokens"),
            cached_input_tokens: row.get("cached_input_tokens"),
            output_tokens: row.get("output_tokens"),
            reasoning_output_tokens: row.get("reasoning_output_tokens"),
            max_turn_duration_sec: row.get("max_turn_duration_sec"),
        })
        .fetch_all(self.pool.as_ref())
        .await?;
        Ok(rows)
    }
}
