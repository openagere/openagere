# MCP Configuration Reference

Use this reference when producing OpenAgere MCP server configuration. Prefer minimal, explicit config over broad defaults.

## Target Formats

Global config uses TOML in `$AGERE_HOME/config.toml`:

```toml
[mcp_servers.docs]
command = "docs-mcp"
args = ["--stdio"]
```

Plugin config uses JSON in `.mcp.json`:

```json
{
  "mcpServers": {
    "docs": {
      "command": "docs-mcp",
      "args": ["--stdio"]
    }
  }
}
```

Use the same server fields in both targets unless the surrounding file format requires TOML or JSON syntax.

## Server Names

Use short ASCII names made from letters, numbers, `-`, or `_`. Choose names that identify the service rather than the transport, such as `github`, `linear`, `docs`, or `warehouse`.

## Stdio Transport

Use stdio for MCP servers launched as a local process.

Global TOML:

```toml
[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env_vars = ["GITHUB_PERSONAL_ACCESS_TOKEN"]
startup_timeout_sec = 20
tool_timeout_sec = 60
default_tools_approval_mode = "prompt"
```

Plugin JSON:

```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env_vars": ["GITHUB_PERSONAL_ACCESS_TOKEN"],
      "startup_timeout_sec": 20,
      "tool_timeout_sec": 60,
      "default_tools_approval_mode": "prompt"
    }
  }
}
```

Allowed stdio fields:

- `command`: executable name or path.
- `args`: command arguments.
- `env`: literal non-secret environment values.
- `env_vars`: environment variable names that OpenAgere forwards.
- `cwd`: working directory for the MCP process.
- Shared fields listed below.

Do not use `url`, `bearer_token_env_var`, `http_headers`, `env_http_headers`, or `oauth_resource` with stdio servers.

Use sourced `env_vars` when placement matters:

```toml
[mcp_servers.remote-docs]
command = "docs-mcp"
env_vars = [
  { name = "DOCS_TOKEN", source = "remote" },
  { name = "LOCAL_CACHE_DIR", source = "local" },
]
```

Supported `source` values are `local` and `remote`.

## Streamable HTTP Transport

Use streamable HTTP for MCP servers exposed over an HTTP endpoint.

OAuth-capable server:

```toml
[mcp_servers.docs]
url = "https://docs.example.com/mcp"
scopes = ["search", "read"]
oauth_resource = "https://docs.example.com"
default_tools_approval_mode = "auto"
```

Bearer-token server:

```toml
[mcp_servers.internal]
url = "https://internal.example.com/mcp"
bearer_token_env_var = "INTERNAL_MCP_TOKEN"
tool_timeout_sec = 30
```

Header sourced from an environment variable:

```toml
[mcp_servers.gateway]
url = "https://gateway.example.com/mcp"
env_http_headers = { "X-API-Key" = "GATEWAY_API_KEY" }
```

Plugin JSON:

```json
{
  "mcpServers": {
    "docs": {
      "url": "https://docs.example.com/mcp",
      "scopes": ["search", "read"],
      "oauth_resource": "https://docs.example.com",
      "default_tools_approval_mode": "auto"
    }
  }
}
```

Allowed streamable HTTP fields:

- `url`: MCP endpoint.
- `bearer_token_env_var`: environment variable containing the bearer token.
- `http_headers`: literal non-secret headers.
- `env_http_headers`: map of header name to environment variable name.
- `scopes`: OAuth scopes to request.
- `oauth_resource`: OAuth resource parameter.
- Shared fields listed below.

Do not use `command`, `args`, `env`, `env_vars`, or `cwd` with streamable HTTP servers.

Never write `bearer_token`; OpenAgere rejects inline bearer tokens. Use `bearer_token_env_var`.

## Shared Fields

These fields can be used with either transport:

- `enabled`: set to `false` to keep the server configured but inactive.
- `required`: set to `true` when execution should fail if the server cannot start.
- `startup_timeout_sec`: startup and initial tool-list timeout in seconds.
- `tool_timeout_sec`: per-tool-call timeout in seconds.
- `default_tools_approval_mode`: `auto`, `prompt`, or `approve`.
- `enabled_tools`: allow-list of tool names.
- `disabled_tools`: deny-list of tool names.
- `supports_parallel_tool_calls`: advertise all tools from the server as parallel-call safe.
- `tools`: per-tool settings keyed by tool name.

## Tool Exposure

Prefer allow-lists for broad servers:

```toml
[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env_vars = ["GITHUB_PERSONAL_ACCESS_TOKEN"]
enabled_tools = ["search_repositories", "get_file_contents"]
```

Use deny-lists when a server is mostly useful but has a few risky tools:

```toml
[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env_vars = ["GITHUB_PERSONAL_ACCESS_TOKEN"]
disabled_tools = ["delete_file", "create_branch"]
```

## Approval Settings

Use server-level defaults for simple cases:

```toml
[mcp_servers.docs]
url = "https://docs.example.com/mcp"
default_tools_approval_mode = "auto"
```

Use per-tool overrides when risk varies by tool:

```toml
[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env_vars = ["GITHUB_PERSONAL_ACCESS_TOKEN"]
default_tools_approval_mode = "prompt"

[mcp_servers.github.tools.search_repositories]
approval_mode = "auto"

[mcp_servers.github.tools.create_pull_request]
approval_mode = "prompt"
```

Approval values:

- `auto`: low-risk trusted tools can run without an extra prompt.
- `prompt`: ask before running the tool.
- `approve`: execute without prompting.

Use `prompt` unless the server and tool behavior are clearly low-risk. Use `approve` only for explicitly trusted tools or automated contexts where bypassing prompts is intentional.

## Timeouts and Required Servers

Use longer startup timeouts for package managers or slow local servers:

```toml
[mcp_servers.slow-local]
command = "npx"
args = ["-y", "slow-mcp-server"]
startup_timeout_sec = 45
tool_timeout_sec = 120
required = true
```

Set `required = true` only when the requested workflow cannot proceed without the server.

## Parallel Tool Calls

Only set `supports_parallel_tool_calls = true` when every enabled tool is safe to run concurrently. Avoid it for tools that mutate shared files, remote records, databases, issue trackers, calendars, or secrets.

```toml
[mcp_servers.readonly-docs]
url = "https://docs.example.com/mcp"
enabled_tools = ["search", "read"]
supports_parallel_tool_calls = true
```

## Common Templates

NPM stdio server:

```toml
[mcp_servers.example]
command = "npx"
args = ["-y", "@example/mcp-server"]
env_vars = ["EXAMPLE_TOKEN"]
default_tools_approval_mode = "prompt"
```

Python stdio server:

```toml
[mcp_servers.example]
command = "python"
args = ["-m", "example_mcp"]
env_vars = ["EXAMPLE_TOKEN"]
cwd = "/path/to/project"
```

Local binary stdio server:

```toml
[mcp_servers.example]
command = "/usr/local/bin/example-mcp"
args = ["serve", "--stdio"]
```

HTTP OAuth server:

```toml
[mcp_servers.example]
url = "https://example.com/mcp"
scopes = ["read"]
oauth_resource = "https://example.com"
```

HTTP bearer-token server:

```toml
[mcp_servers.example]
url = "https://example.com/mcp"
bearer_token_env_var = "EXAMPLE_MCP_TOKEN"
```

## Verification

After global config changes:

```shell
agere mcp list
agere mcp get <name>
agere mcp login <name>
```

Use `agere mcp login <name>` for streamable HTTP servers that support OAuth. For a running app-server, reload MCP config before expecting loaded threads to see disk changes.

After plugin `.mcp.json` changes, inspect the plugin through the plugin workflow or install/read the plugin in the app UI to confirm the MCP server names appear.
