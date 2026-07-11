# Exit and shutdown flow (tui)

This document describes how exit, shutdown, and interruption work in the Rust TUI (`tui`).
It is intended for OpenAgere developers and OpenAgere itself when reasoning about future exit/shutdown
changes.

This doc replaces earlier separate history and design notes. High-level history is summarized
below; full details are captured in PR #8936.

## Terms

- **Exit**: end the UI event loop and terminate the process.
- **Shutdown**: request a graceful agent/core shutdown (`Op::Shutdown`) and wait for
  `ShutdownComplete` so cleanup can run.
- **Interrupt**: cancel a running operation (`Op::Interrupt`).

## Event model (AppEvent)

Exit is coordinated via a single event with explicit modes:

- `AppEvent::Exit(ExitMode::ShutdownFirst)`
  - Prefer this for user-initiated quits so cleanup runs.
- `AppEvent::Exit(ExitMode::Immediate)`
  - Escape hatch for immediate exit. This bypasses shutdown and can drop
    in-flight work (e.g., tasks, rollout flush, child process cleanup).

`App` is the coordinator: it submits `Op::Shutdown` and it exits the UI loop only when
`ExitMode::Immediate` arrives (typically after `ShutdownComplete`).

## User-triggered quit flows

### Ctrl+C

Priority order in the UI layer (`DOUBLE_PRESS_QUIT_SHORTCUT_ENABLED` is currently `false`):

1. Active modal/view gets the first chance to consume (`BottomPane::on_ctrl_c`).
   - If the modal handles it, the quit flow stops (for example clearing a draft or dismissing a
     prompt). Cleared drafts are also appended to cross-session message history when a thread id
     is available.
   - When an active view's key would interrupt the agent turn (for example
     `request_user_input` interrupt bindings), the active goal is paused.
2. If cancellable work is active (streaming/tools/review), `ChatWidget` submits
   `Op::Interrupt` with restore-prompt-if-no-output behavior, and pauses any active goal.
3. If idle with no cancellable work, Ctrl+C requests shutdown-first quit immediately
   (single press; the experimental double-press quit shortcut is disabled).

When the double-press quit experiment is re-enabled:

1. Modal handling still comes first; arming/clearing the quit shortcut follows the same rules.
2. A second Ctrl+C within ~1s requests shutdown-first quit.
3. The first press arms the quit hint (`ctrl + c again to quit`) and may also interrupt.

### Ctrl+D

- Only participates in quit when the composer is empty **and** no modal is active.
- With double-press disabled: requests shutdown-first quit on a single press when idle and empty.
- With any modal/popup open, key events are routed to the view and Ctrl+D does not attempt to
  quit.

### Slash commands

- `/quit`, `/exit`, `/logout` request shutdown-first quit **without** a prompt,
  because slash commands are harder to trigger accidentally and imply clear intent to quit.

### /new

- Uses shutdown without exit (suppresses `ShutdownComplete`) so the app can
  start a fresh session without terminating.

## Shutdown completion and suppression

`ShutdownComplete` is the signal that core cleanup has finished. The UI treats it as the boundary
for exit:

- `ChatWidget` requests `Exit(Immediate)` on `ShutdownComplete`.
- `App` can suppress a single `ShutdownComplete` when shutdown is used as a
  cleanup step (e.g., `/new`).

## Edge cases and invariants

- **Review mode** counts as cancellable work. Ctrl+C should interrupt review, not
  quit.
- **Modal open** means Ctrl+C/Ctrl+D should not quit unless the modal explicitly
  declines to handle Ctrl+C.
- **Immediate exit** is not a normal user path; it is a fallback for shutdown
  completion or an emergency exit. Use it sparingly because it skips cleanup.

## Testing expectations

At a minimum, we want coverage for:

- Ctrl+C while working interrupts, does not quit (and may restore the cancelled prompt).
- Ctrl+C while idle requests shutdown-first quit (single press while double-press is disabled).
- Ctrl+D with modal open does not quit.
- `/quit` / `/exit` / `/logout` quit without prompt, but still shutdown-first.
- Ctrl+D while idle and empty requests shutdown-first quit (single press while double-press is
  disabled).

## History (high level)

OpenAgere has historically mixed "exit immediately" and "shutdown-first" across quit gestures, largely
due to incremental changes and regressions in state tracking. This doc reflects the current
unified, shutdown-first approach. See PR #8936 for the detailed history and rationale.
