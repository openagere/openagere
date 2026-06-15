use super::App;
use crate::app_event::AppEvent;
use crate::app_event::ThreadGoalSetMode;
use crate::app_server_session::AppServerSession;
use crate::bottom_pane::SelectionAction;
use crate::bottom_pane::SelectionItem;
use crate::bottom_pane::SelectionViewParams;
use crate::bottom_pane::popup_consts::standard_popup_hint_line;
use crate::goal_display::GOAL_USAGE;
use crate::goal_display::goal_status_label;
use crate::goal_display::goal_usage_summary;
use agere_app_server_protocol::ThreadGoal;
use agere_app_server_protocol::ThreadGoalStatus;
use agere_protocol::ThreadId;

impl App {
    pub(super) async fn open_thread_goal_menu(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) {
        let result = app_server.thread_goal_get(thread_id).await;
        if self.current_displayed_thread_id() != Some(thread_id) {
            return;
        }

        let response = match result {
            Ok(response) => response,
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to read thread goal: {err}"));
                return;
            }
        };

        let Some(goal) = response.goal else {
            self.chat_widget.add_info_message(
                GOAL_USAGE.to_string(),
                Some("No goal is currently set.".to_string()),
            );
            return;
        };

        self.chat_widget.show_goal_summary(goal);
    }

    pub(super) async fn open_thread_goal_editor(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: Option<ThreadId>,
    ) {
        let Some(thread_id) = thread_id else {
            self.show_no_thread_goal_to_edit();
            return;
        };

        let result = app_server.thread_goal_get(thread_id).await;
        if self.current_displayed_thread_id() != Some(thread_id) {
            return;
        }

        let response = match result {
            Ok(response) => response,
            Err(err) => {
                self.chat_widget
                    .add_error_message(format!("Failed to read thread goal: {err}"));
                return;
            }
        };

        let Some(goal) = response.goal else {
            self.show_no_thread_goal_to_edit();
            return;
        };

        self.chat_widget.show_goal_edit_prompt(thread_id, goal);
    }

    pub(super) async fn set_thread_goal_objective(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        objective: String,
        mode: ThreadGoalSetMode,
    ) {
        let mode = if mode == ThreadGoalSetMode::ConfirmIfExists {
            let result = app_server.thread_goal_get(thread_id).await;
            if self.current_displayed_thread_id() != Some(thread_id) {
                return;
            }

            match result {
                Ok(response) => match confirmed_goal_set_mode(response.goal.as_ref(), mode) {
                    ConfirmedGoalSetMode::Set(mode) => mode,
                    ConfirmedGoalSetMode::ConfirmReplace => {
                        self.show_replace_thread_goal_confirmation(thread_id, objective);
                        return;
                    }
                },
                Err(err) => {
                    self.chat_widget
                        .add_error_message(format!("Failed to read thread goal: {err}"));
                    return;
                }
            }
        } else {
            mode
        };

        let (status, token_budget) = match mode {
            ThreadGoalSetMode::ConfirmIfExists | ThreadGoalSetMode::ReplaceExisting => {
                (ThreadGoalStatus::Active, None)
            }
            ThreadGoalSetMode::UpdateExisting {
                status,
                token_budget,
            } => (status, Some(token_budget)),
        };

        let result = if mode == ThreadGoalSetMode::ReplaceExisting {
            app_server.thread_goal_replace(thread_id, objective).await
        } else {
            app_server
                .thread_goal_set(thread_id, Some(objective), Some(status), token_budget)
                .await
        };
        if self.current_displayed_thread_id() != Some(thread_id) {
            return;
        }

        match result {
            Ok(response) => self.chat_widget.add_info_message(
                format!("Goal {}", goal_status_label(response.goal.status)),
                Some(goal_usage_summary(&response.goal)),
            ),
            Err(err) => {
                let replacing_goal = mode == ThreadGoalSetMode::ReplaceExisting;
                let action = if replacing_goal { "replace" } else { "set" };
                self.chat_widget
                    .add_error_message(format!("Failed to {action} thread goal: {err}"));
            }
        }
    }

    pub(super) async fn set_thread_goal_status(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
        status: ThreadGoalStatus,
    ) {
        let result = app_server
            .thread_goal_set(
                thread_id,
                /*objective*/ None,
                Some(status),
                /*token_budget*/ None,
            )
            .await;
        if self.current_displayed_thread_id() != Some(thread_id) {
            return;
        }

        match result {
            Ok(response) => self.chat_widget.add_info_message(
                format!("Goal {}", goal_status_label(response.goal.status)),
                Some(goal_usage_summary(&response.goal)),
            ),
            Err(err) => self
                .chat_widget
                .add_error_message(format!("Failed to update thread goal: {err}")),
        }
    }

    pub(super) async fn clear_thread_goal(
        &mut self,
        app_server: &mut AppServerSession,
        thread_id: ThreadId,
    ) {
        let result = app_server.thread_goal_clear(thread_id).await;
        if self.current_displayed_thread_id() != Some(thread_id) {
            return;
        }

        match result {
            Ok(response) => {
                if response.cleared {
                    self.chat_widget
                        .add_info_message("Goal cleared".to_string(), /*hint*/ None);
                } else {
                    self.chat_widget.add_info_message(
                        "No goal to clear".to_string(),
                        Some("This thread does not currently have a goal.".to_string()),
                    );
                }
            }
            Err(err) => self
                .chat_widget
                .add_error_message(format!("Failed to clear thread goal: {err}")),
        }
    }

    fn show_replace_thread_goal_confirmation(&mut self, thread_id: ThreadId, objective: String) {
        let replace_objective = objective.clone();
        let replace_actions: Vec<SelectionAction> = vec![Box::new(move |tx| {
            tx.send(AppEvent::SetThreadGoalObjective {
                thread_id,
                objective: replace_objective.clone(),
                mode: ThreadGoalSetMode::ReplaceExisting,
            });
        })];
        let items = vec![
            SelectionItem {
                name: "Replace current goal".to_string(),
                description: Some("Set the new objective and start it now".to_string()),
                actions: replace_actions,
                dismiss_on_select: true,
                ..Default::default()
            },
            SelectionItem {
                name: "Cancel".to_string(),
                description: Some("Keep the current goal".to_string()),
                dismiss_on_select: true,
                ..Default::default()
            },
        ];
        self.chat_widget.show_selection_view(SelectionViewParams {
            title: Some("Replace goal?".to_string()),
            subtitle: Some(format!("New objective: {objective}")),
            footer_hint: Some(standard_popup_hint_line()),
            items,
            ..Default::default()
        });
    }

    fn show_no_thread_goal_to_edit(&mut self) {
        self.chat_widget
            .add_error_message("No goal is currently set.".to_string());
        self.chat_widget.add_info_message(
            GOAL_USAGE.to_string(),
            Some("Create a goal before editing it.".to_string()),
        );
    }
}

fn should_confirm_before_replacing_goal(goal: &ThreadGoal) -> bool {
    match goal.status {
        ThreadGoalStatus::Complete => false,
        ThreadGoalStatus::Active
        | ThreadGoalStatus::Paused
        | ThreadGoalStatus::Blocked
        | ThreadGoalStatus::UsageLimited
        | ThreadGoalStatus::BudgetLimited => true,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ConfirmedGoalSetMode {
    Set(ThreadGoalSetMode),
    ConfirmReplace,
}

fn confirmed_goal_set_mode(
    goal: Option<&ThreadGoal>,
    requested_mode: ThreadGoalSetMode,
) -> ConfirmedGoalSetMode {
    match goal {
        Some(goal) if should_confirm_before_replacing_goal(goal) => {
            ConfirmedGoalSetMode::ConfirmReplace
        }
        Some(_) | None => ConfirmedGoalSetMode::Set(requested_mode),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_goal_does_not_require_replace_confirmation() {
        assert!(!should_confirm_before_replacing_goal(&test_goal(
            ThreadGoalStatus::Complete
        )));
    }

    #[test]
    fn completed_goal_set_uses_backend_replacement_without_clear() {
        assert_eq!(
            confirmed_goal_set_mode(
                Some(&test_goal(ThreadGoalStatus::Complete)),
                ThreadGoalSetMode::ConfirmIfExists,
            ),
            ConfirmedGoalSetMode::Set(ThreadGoalSetMode::ConfirmIfExists)
        );
    }

    #[test]
    fn unfinished_goal_set_prompts_before_replacement() {
        assert_eq!(
            confirmed_goal_set_mode(
                Some(&test_goal(ThreadGoalStatus::Active)),
                ThreadGoalSetMode::ConfirmIfExists,
            ),
            ConfirmedGoalSetMode::ConfirmReplace
        );
    }

    #[test]
    fn unfinished_goals_require_replace_confirmation() {
        for status in [
            ThreadGoalStatus::Active,
            ThreadGoalStatus::Paused,
            ThreadGoalStatus::Blocked,
            ThreadGoalStatus::UsageLimited,
            ThreadGoalStatus::BudgetLimited,
        ] {
            assert!(should_confirm_before_replacing_goal(&test_goal(status)));
        }
    }

    fn test_goal(status: ThreadGoalStatus) -> ThreadGoal {
        ThreadGoal {
            thread_id: ThreadId::new().to_string(),
            objective: "Finish the thing.".to_string(),
            status,
            token_budget: None,
            tokens_used: 0,
            time_used_seconds: 0,
            created_at: 1_776_272_400,
            updated_at: 1_776_272_460,
        }
    }
}
