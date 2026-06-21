//! Coordinates asynchronous usage card rendering in the chat widget.
//!
//! The slash command builds a composite history cell immediately, but the widget
//! keeps that cell transient while the provider-usage request runs. The transient
//! card is rendered above the composer through the pending-output accessor so
//! loading never requires clearing or rewriting transcript history. When the
//! matching response arrives, [`TokenActivityHandle`] updates the shared card
//! state and the widget moves the cell into a completed slot.
//!
//! Pure chart rendering and date bucketing live in [`super::tokens_chart`]. This
//! module owns request correlation, transient/completed card state, and the
//! [`UsageData`] adapter that wraps the provider-based response.

use std::sync::Arc;
use std::sync::RwLock;

use agere_app_server_protocol::GetProviderUsageResponse;
use chrono::NaiveDate;
use chrono::Utc;
use ratatui::style::Stylize;
use ratatui::text::Line;

use crate::app_event::AppEvent;
use crate::chatwidget::ChatWidget;
use crate::history_cell::CompositeHistoryCell;
use crate::history_cell::HistoryCell;
use crate::history_cell::PlainHistoryCell;

pub(crate) use super::tokens_chart::TokenActivityView;

#[cfg(test)]
#[path = "tokens_tests.rs"]
mod tests;

/// Adapter around [`GetProviderUsageResponse`] that exposes chart-friendly
/// accessors for daily totals and provider identifiers.
#[derive(Debug)]
pub(crate) struct UsageData {
    pub(crate) response: GetProviderUsageResponse,
}

impl UsageData {
    /// Get total daily bucket values across all providers for chart display.
    pub(crate) fn daily_totals(&self) -> Vec<(String, i64)> {
        self.response
            .total
            .daily_buckets
            .iter()
            .map(|b| (b.date.clone(), b.total_tokens))
            .collect()
    }

    /// Get all provider IDs from the response.
    pub(crate) fn provider_ids(&self) -> Vec<&str> {
        self.response
            .providers
            .iter()
            .map(|p| p.provider_id.as_str())
            .collect()
    }

    /// Get the longest single turn duration in seconds, if any turns have been recorded.
    pub(crate) fn longest_running_turn_sec(&self) -> Option<i64> {
        self.response.total.longest_running_turn_sec
    }
}

/// Tracks the renderable lifecycle of one token activity history cell.
#[derive(Debug)]
enum TokenActivityState {
    Loading,
    Loaded { data: UsageData, today: NaiveDate },
    Error,
}

/// Completes an asynchronously rendered token activity history cell.
///
/// Clones share the same card state, allowing the background request path to
/// update a cell still owned by the widget's transient-output state.
#[derive(Clone, Debug)]
pub(crate) struct TokenActivityHandle {
    state: Arc<RwLock<TokenActivityState>>,
}

impl TokenActivityHandle {
    /// Replaces the loading state with either fetched activity or an error state.
    ///
    /// This method intentionally discards the error string because the TUI exposes
    /// one stable unavailable message. Calling it more than once replaces the prior
    /// terminal state, so request-ID matching should happen before completion.
    pub(crate) fn finish(&self, result: Result<GetProviderUsageResponse, String>) {
        self.finish_with_today(result, Utc::now().date_naive());
    }

    fn finish_with_today(
        &self,
        result: Result<GetProviderUsageResponse, String>,
        today: NaiveDate,
    ) {
        let state = match result {
            Ok(response) => TokenActivityState::Loaded {
                data: UsageData { response },
                today,
            },
            Err(_) => TokenActivityState::Error,
        };
        #[expect(clippy::expect_used)]
        let mut current = self.state.write().expect("token activity state poisoned");
        *current = state;
    }
}

/// Renders one usage card from shared asynchronous state.
#[derive(Debug)]
struct TokenActivityHistoryCell {
    view: TokenActivityView,
    state: Arc<RwLock<TokenActivityState>>,
}

/// Creates the card contents and completion handle for one `/usage` invocation.
///
/// The composite cell includes the echoed slash command and a loading card from
/// the start. Callers must retain the returned handle and complete it when the
/// matching background response arrives; otherwise the transient card stays
/// loading.
pub(crate) fn new_token_activity_output(
    view: TokenActivityView,
) -> (CompositeHistoryCell, TokenActivityHandle) {
    let command = PlainHistoryCell::new(vec![
        format!("/usage {}", view.label().to_lowercase())
            .magenta()
            .into(),
    ]);
    let state = Arc::new(RwLock::new(TokenActivityState::Loading));
    let handle = TokenActivityHandle {
        state: Arc::clone(&state),
    };
    let card = TokenActivityHistoryCell { view, state };
    (
        CompositeHistoryCell::new(vec![Box::new(command), Box::new(card)]),
        handle,
    )
}

impl HistoryCell for TokenActivityHistoryCell {
    fn display_lines(&self, width: u16) -> Vec<Line<'static>> {
        #[expect(clippy::expect_used)]
        let state = self.state.read().expect("token activity state poisoned");
        match &*state {
            TokenActivityState::Loading => {
                vec![
                    " Token activity".bold().into(),
                    "   Loading usage data...".dim().into(),
                ]
            }
            TokenActivityState::Error => vec![
                " Token activity".bold().into(),
                "   Token activity unavailable".dim().into(),
            ],
            TokenActivityState::Loaded { data, today } => {
                super::tokens_chart::loaded_lines(self.view, data, *today, width)
            }
        }
    }

    fn transcript_lines(&self, width: u16) -> Vec<Line<'static>> {
        self.display_lines(width)
    }
}

/// Pending token activity output awaiting a background response.
pub(crate) struct PendingTokenActivityOutput {
    request_id: u64,
    cell: CompositeHistoryCell,
    handle: TokenActivityHandle,
}

impl ChatWidget {
    /// Start an async token activity card and dispatch the background fetch.
    pub(crate) fn add_token_activity_output(&mut self, view: TokenActivityView) {
        let request_id = self.next_token_activity_request_id;
        self.next_token_activity_request_id = self.next_token_activity_request_id.wrapping_add(1);
        let (cell, handle) = new_token_activity_output(view);
        self.pending_token_activity = Some(PendingTokenActivityOutput {
            request_id,
            cell,
            handle,
        });
        self.completed_token_activity = None;
        self.bump_active_cell_revision();
        self.request_redraw();
        self.app_event_tx
            .send(AppEvent::RefreshTokenActivity { request_id });
    }

    /// Returns the transient token activity card that should render above the composer.
    pub(crate) fn pending_token_activity_output(&self) -> Option<&dyn HistoryCell> {
        self.pending_token_activity
            .as_ref()
            .map(|output| &output.cell as &dyn HistoryCell)
            .or_else(|| {
                self.completed_token_activity
                    .as_ref()
                    .map(|output| &output.cell as &dyn HistoryCell)
            })
    }

    /// Complete the pending token activity refresh with a fetched result.
    pub(crate) fn finish_token_activity_refresh(
        &mut self,
        request_id: u64,
        result: Result<GetProviderUsageResponse, String>,
    ) -> bool {
        let Some(output) = self.pending_token_activity.take() else {
            return false;
        };
        if output.request_id != request_id {
            self.pending_token_activity = Some(output);
            return false;
        }
        output.handle.finish(result);
        self.completed_token_activity = Some(output);
        self.bump_active_cell_revision();
        self.request_redraw();
        true
    }

    /// Whether a running task prevents inserting the usage card now.
    pub(crate) fn usage_history_insertion_blocked(&self) -> bool {
        self.stream_controller.is_some()
            || self.plan_stream_controller.is_some()
            || self.pending_stream_consolidations > 0
            || self.active_cell.is_some()
            || self.active_hook_cell.is_some()
    }

    /// Records a stream consolidation barrier that delays token card insertion.
    ///
    /// Each queued consolidation should eventually call
    /// [`ChatWidget::note_stream_consolidation_completed`].
    pub(crate) fn note_stream_consolidation_queued(&mut self) {
        self.pending_stream_consolidations = self.pending_stream_consolidations.saturating_add(1);
    }

    /// Releases one queued stream consolidation barrier.
    ///
    /// The counter saturates at zero so an unmatched completion does not underflow,
    /// but paired queue/completion calls are still the intended contract.
    pub(crate) fn note_stream_consolidation_completed(&mut self) {
        self.pending_stream_consolidations = self.pending_stream_consolidations.saturating_sub(1);
    }

    /// Requests another insertion attempt when completed usage output is waiting.
    pub(crate) fn request_pending_usage_output_insertion(&self) {
        if self.completed_token_activity.is_some() {
            self.app_event_tx.send(AppEvent::CommitPendingUsageOutput);
        }
    }

    pub(crate) fn request_pending_usage_output_insertion_after_stream_shutdown(&self) {
        if self.completed_token_activity.is_some() {
            self.app_event_tx
                .send(AppEvent::CommitPendingUsageOutputAfterStreamShutdown);
        }
    }

    /// Take the completed token activity output for insertion into history.
    pub(crate) fn take_completed_token_activity_output(&mut self) -> Option<CompositeHistoryCell> {
        let output = self.completed_token_activity.take()?;
        self.bump_active_cell_revision();
        Some(output.cell)
    }

    /// Drops transient and completed token cards that must no longer update.
    pub(crate) fn clear_pending_token_activity_refreshes(&mut self) {
        let cleared_refresh = self.pending_token_activity.take().is_some();
        let cleared_completed = self.completed_token_activity.take().is_some();
        if cleared_refresh || cleared_completed {
            self.bump_active_cell_revision();
            self.request_redraw();
        }
    }
}
