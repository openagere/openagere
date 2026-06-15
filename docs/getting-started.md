# Getting started with OpenAgere CLI

This guide walks you through your first session with OpenAgere.

## Installation

```shell
npm i -g openagere
openagere
```

See [`docs/install.md`](install.md) for alternative installation methods (Homebrew, direct download, build from source).

## Launching the TUI

Run `openagere` with no arguments to start the interactive terminal UI:

```shell
openagere
```

The TUI opens in fullscreen mode. You'll see a chat composer at the bottom where you can type prompts.

## Your first prompt

Type a natural-language request and press Enter. OpenAgere will:

1. Read relevant files in your codebase
2. Run shell commands (with your approval, by default)
3. Apply code changes via structured patches
4. Iterate until the task is complete

Example prompts:

```
Explain how this project is structured
Fix all clippy warnings in src/
Add a --verbose flag to the CLI and update the README
```

## Keyboard shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Submit prompt |
| `Ctrl+C` | Interrupt current turn |
| `Ctrl+D` | Exit (graceful shutdown) |
| `?` | Toggle command palette |
| `/` | Slash command prefix |
| `↑` / `↓` | Navigate history |
| `PageUp` / `PageDown` | Scroll chat |

## Slash commands

Type `/` in the composer to access built-in commands:

| Command | Description |
|---------|-------------|
| `/model` | Switch the active model |
| `/compact` | Compress conversation context |
| `/init` | Generate or update AGENTS.md |
| `/resume` | Resume a previous session |
| `/fork` | Fork the current session |
| `/keymap` | View or rebind keys |
| `/apps` | List available connectors |
| `/help` | Show help |

## Configuration

OpenAgere is configured via `~/.openagere/config.toml`. See [`docs/config.md`](config.md) for the full reference.

Minimal example:

```toml
# ~/.openagere/config.toml
access_mode = "workspace-write"
```

## Non-interactive mode

Use `openagere exec` for scripts and CI:

```shell
openagere exec "Fix the failing tests in src/parser.rs"
```

See [`docs/exec.md`](exec.md) for details.

## Next steps

- [`docs/config.md`](config.md) — Full configuration reference
- [`docs/skills.md`](skills.md) — Extend OpenAgere with custom skills
- [`docs/slash_commands.md`](slash_commands.md) — Complete slash command reference
