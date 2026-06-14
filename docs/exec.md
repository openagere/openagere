# Non-interactive mode (`openagere exec`)

The `exec` subcommand runs OpenAgere headlessly — no TUI, no interactive prompts. It's designed for scripts, CI/CD pipelines, and automation.

## Basic usage

```shell
openagere exec "Your prompt here"
```

OpenAgere processes the prompt, runs any needed tools, and exits when the task is complete.

## Piping stdin

You can pipe content into `exec`. OpenAgere appends it as a `<stdin>` block after your prompt:

```shell
echo "build output with errors" | openagere exec "Summarize these errors concisely"
git diff HEAD~1 | openagere exec "Write a commit message for this diff"
```

## Ephemeral mode

Use `--ephemeral` to skip persisting session rollout files to disk:

```shell
openagere exec --ephemeral "Quick one-off task"
```

## Output format

By default, OpenAgere prints the agent's response directly to stdout. Use `RUST_LOG` to enable logging:

```shell
RUST_LOG=agere_core=debug openagere exec "Debug this issue"
```

## Exit codes

- `0` — Task completed successfully
- `1` — Task failed or was interrupted

## Relationship to other modes

| Mode | Use case |
|------|----------|
| `openagere` (TUI) | Interactive development — full terminal UI with approvals |
| `openagere exec` | Automation — scripts, CI, one-shot tasks |
| `openagere exec-server` | Remote/IDE — persistent agent server accessible over WebSocket |

## Security in exec mode

The same access modes apply:

```shell
openagere exec --access-mode workspace-write "Refactor this module"
```

Without a TUI, approval prompts that would normally require user interaction will be auto-denied. Configure `approval_mode = "approve"` for tools you trust in automated contexts.
