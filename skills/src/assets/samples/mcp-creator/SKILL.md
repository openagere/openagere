---
name: mcp-creator
description: Configure OpenAgere MCP servers in global config.toml or plugin .mcp.json files. Use when adding, editing, validating, or explaining MCP server configuration; choosing stdio or streamable HTTP transports; setting OAuth, bearer token environment variables, headers, tool filters, approval modes, timeouts, required servers, or parallel tool-call settings; or generating plugin-bundled MCP server definitions.
---

# MCP Creator

Use this skill to configure MCP servers for OpenAgere. Keep edits scoped to the requested MCP server and preserve unrelated config.

## Workflow

1. Identify the target:
   - Global config: `$AGERE_HOME/config.toml` under `[mcp_servers.<name>]`.
   - Plugin config: `.mcp.json` under top-level `mcpServers`.
2. Identify the transport:
   - stdio: local process launched with `command`, optional `args`, `env`, `env_vars`, and `cwd`.
   - streamable HTTP: remote or local HTTP endpoint configured with `url`, optional `bearer_token_env_var`, `http_headers`, and `env_http_headers`.
3. Ask only for missing values that cannot be inferred safely, such as server name, command, URL, required env var names, or whether tools mutate external state.
4. Use the smallest valid config that satisfies the request.
5. Never inline secrets. Do not write `bearer_token`. Prefer `bearer_token_env_var`, `env_vars`, or `env_http_headers`.
6. Narrow broad servers with `enabled_tools` or `disabled_tools` when the user only needs a subset.
7. Use approval modes conservatively:
   - Use `prompt` for tools that mutate files, secrets, network resources, external services, or user data.
   - Use `auto` only for trusted low-risk tools.
   - Use per-tool overrides when one server mixes low-risk and risky tools.
8. Set `supports_parallel_tool_calls = true` only when the server's tools are known to be safe under concurrent calls.
9. Suggest verification:
   - Global config: `agere mcp list`, `agere mcp get <name>`, and `agere mcp login <name>` for OAuth-capable HTTP servers.
   - App-server sessions: reload MCP config when the running app-server needs to pick up disk edits.
   - Plugin config: inspect or install the plugin through the existing plugin workflow.

## References

Read `references/mcp-config.md` when writing concrete MCP configuration, validating field compatibility, or choosing a template.

When creating a plugin that also needs a manifest or marketplace entry, use the `plugin-creator` skill for plugin scaffolding and use this skill only for the MCP server definition.
