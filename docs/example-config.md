# Sample configuration

A minimal `~/.openagere/config.toml` with common settings:

```toml
# Access mode: read-only, workspace-write, or danger-full-access
access_mode = "workspace-write"

# Default model provider and model
[model_providers.openai]
api_key = "sk-..."

# MCP server example
[mcp_servers.docs]
command = "docs-server"
default_tools_approval_mode = "approve"

[mcp_servers.docs.tools.search]
approval_mode = "prompt"

# Tool approval defaults
default_tools_approval_mode = "prompt"

[tools.shell]
approval_mode = "approve"

# Notifications (macOS example)
[notify]
command = "terminal-notifier -title 'OpenAgere' -message 'Turn completed'"

# Permissions
[permissions]
default_profile = "standard"

# Disabled tool suggestions
[tool_suggest]
disabled_tools = [
  { type = "plugin", id = "slack@openagere-curated" },
]
```

For the full configuration reference, see [`docs/config.md`](config.md).
