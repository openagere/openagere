# Execution policy

OpenAgere execution policy system controls which commands and operations the agent is allowed to perform, and under what conditions.

## Access modes

The top-level access mode sets the coarse security boundary:

| Mode | Filesystem | Network |
|------|-----------|---------|
| `read-only` | Read only | Allowed |
| `workspace-write` | Write within workspace | Blocked |
| `danger-full-access` | Full access | Allowed |

Set via CLI:

```shell
openagere --access-mode workspace-write
```

Or in `~/.openagere/config.toml`:

```toml
access_mode = "workspace-write"
```

## Permission profiles

Permission profiles provide finer-grained control. They define which directories are readable, writable, and which tools are available.

```toml
[permissions]
default_profile = "standard"
```

## Tool approvals

Each tool can have a per-operation approval mode:

| Mode | Behavior |
|------|----------|
| `prompt` | Ask the user before executing (default) |
| `approve` | Execute without prompting |
| `deny` | Block execution entirely |

### Configuring approvals

```toml
# Default for all tools
default_tools_approval_mode = "prompt"

# Per-tool overrides
[tools.shell]
approval_mode = "approve"

[tools.apply_patch]
approval_mode = "approve"
```

### MCP server approvals

```toml
[mcp_servers.docs]
command = "docs-server"
default_tools_approval_mode = "approve"

[mcp_servers.docs.tools.search]
approval_mode = "prompt"
```

## OS-level sandboxing

OpenAgere leverages platform-native sandboxing:

- **macOS:** Seatbelt profiles enforce filesystem and network restrictions
- **Linux:** Landlock and bubblewrap for namespace isolation
- **Windows:** Elevated/unelevated backends with path-based restrictions

## Hook system

Pre- and post-tool-use hooks allow custom logic before and after tool execution. Hooks are written in Starlark and configured in `config.toml`.
