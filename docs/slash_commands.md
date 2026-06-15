# Slash commands

Slash commands provide quick access to OpenAgere features from the TUI composer. Type `/` to see the available commands, or type a command name to filter.

## Built-in commands

### Session management

| Command | Description |
|---------|-------------|
| `/resume` | Resume a previous session from history |
| `/fork` | Fork the current session at a previous turn |
| `/compact` | Compress conversation context to save tokens |
| `/clear` | Clear the current conversation |

### Model and configuration

| Command | Description |
|---------|-------------|
| `/model` | Switch the active model |
| `/model list` | List available models with capabilities |
| `/keymap` | View or rebind keyboard shortcuts |
| `/config` | View or edit current configuration |

### Repository context

| Command | Description |
|---------|-------------|
| `/init` | Generate or update AGENTS.md for the repository |
| `/agents` | View the current AGENTS.md instructions |
| `/git` | Show git status, diff, or log |

### Apps and connectors

| Command | Description |
|---------|-------------|
| `/apps` | List available and installed connectors |
| `/plugins` | Manage plugins |

### Help and diagnostics

| Command | Description |
|---------|-------------|
| `/help` | Show help and available commands |
| `/doctor` | Diagnose configuration issues |
| `/feedback` | Submit feedback |

## Using slash commands

1. In the TUI composer, type `/`
2. A popup appears showing available commands
3. Type to filter, use arrow keys to navigate, Enter to select
4. Some commands open sub-menus or prompt for additional input

## Tips

- Press `?` (without `/`) to open the full command palette with search
- Most commands can be abbreviated
- Slash commands are only available in the interactive TUI, not in `exec` mode
