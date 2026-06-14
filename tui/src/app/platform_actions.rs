//! Platform-specific app actions and small global shortcuts.
//!
//! This module owns platform state used by `App`, the side-conversation return shortcut predicate,
//! and Windows execution-restriction helper actions that are compiled only on Windows.

use super::*;

#[derive(Default)]
pub(super) struct WindowsRestrictionState {
    pub(super) setup_started_at: Option<Instant>,
    // One-shot suppression of the next world-writable scan after user confirmation.
    pub(super) skip_world_writable_scan_once: bool,
}

impl App {
    #[cfg(target_os = "windows")]
    pub(super) fn spawn_world_writable_scan(
        _cwd: AbsolutePathBuf,
        _env_map: std::collections::HashMap<String, String>,
        _logs_base_dir: AbsolutePathBuf,
        _permission_profile: PermissionProfile,
        _tx: AppEventSender,
    ) {
        // Windows restriction integration removed; no-op.
    }
}

#[cfg(target_os = "windows")]
fn send_world_writable_scan_failed(tx: &AppEventSender) {
    tx.send(AppEvent::OpenWorldWritableWarningConfirmation {
        preset: None,
        sample_paths: Vec::new(),
        extra_count: 0usize,
        failed_scan: true,
    });
}

pub(super) fn side_return_shortcut_matches(key_event: KeyEvent) -> bool {
    match key_event {
        KeyEvent {
            code: KeyCode::Esc,
            kind: KeyEventKind::Press | KeyEventKind::Repeat,
            ..
        } => true,
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers,
            kind: KeyEventKind::Press,
            ..
        } if modifiers.contains(KeyModifiers::CONTROL) && c.eq_ignore_ascii_case(&'c') => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_return_shortcuts_match_esc_and_ctrl_c() {
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        )));
        assert!(side_return_shortcut_matches(KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        )));
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
        )));
        assert!(side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('C'),
            KeyModifiers::CONTROL,
        )));
        assert!(!side_return_shortcut_matches(KeyEvent::new(
            KeyCode::Char('d'),
            KeyModifiers::CONTROL,
        )));
        assert!(!side_return_shortcut_matches(KeyEvent::new_with_kind(
            KeyCode::Esc,
            KeyModifiers::NONE,
            KeyEventKind::Release,
        )));
    }
}
