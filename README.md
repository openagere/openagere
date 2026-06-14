<div align="center">

# OpenAgere CLI

**An open-source, terminal-native AI coding agent powered by Rust.**

[English](./README.md) | [中文](./README_zh.md)

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://rust-lang.org)

</div>

---

OpenAgere CLI is an agentic coding assistant that runs where developers already work: the terminal. It can inspect a repository, explain code, edit files through structured patches, run commands, review diffs, resume prior sessions, and connect to external tools through MCP, skills, plugins, and app-server integrations.

It is designed for real engineering work rather than single-turn chat: OpenAgere keeps project instructions in context, exposes a rich interactive TUI, supports non-interactive automation, and wraps filesystem and shell access in configurable approval and sandbox policies.

<p align="center">
  <img src="1.gif" alt="OpenAgere interactive TUI demo" width="720">
</p>

## Source provenance

OpenAgere CLI is developed from a fork of [OpenAI's Codex CLI](https://github.com/openai/codex) (Rust implementation). This tree contains substantial follow-on changes and project-specific behavior; licensing and third-party notices are summarized in [`LICENSE`](./LICENSE) and [`NOTICE`](./NOTICE).

---

## Table of Contents

- [Why OpenAgere](#why-openagere)
- [Highlights](#highlights)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Common Workflows](#common-workflows)
- [Interactive TUI](#interactive-tui)
- [CLI Commands](#cli-commands)
- [Configuration](#configuration)
- [Approvals and Sandboxing](#approvals-and-sandboxing)
- [Models and Providers](#models-and-providers)
- [MCP, Skills, and Plugins](#mcp-skills-and-plugins)
- [Automation and App Server](#automation-and-app-server)
- [Repository Layout](#repository-layout)
- [Development](#development)
- [Documentation](#documentation)
- [Security, Contributing, and License](#security-contributing-and-license)

---

## Why OpenAgere

Modern coding agents need more than a prompt box. They need to understand a repository, perform safe changes, coordinate tools, remember sessions, and run in environments ranging from a developer laptop to an IDE or automation service.

OpenAgere focuses on five practical goals:

1. **Terminal-first productivity** — a fast Rust binary with an interactive TUI and scriptable command-line modes.
2. **Controlled execution** — shell commands, patches, network access, and filesystem writes are mediated by access modes and approval rules.
3. **Repository awareness** — project instructions, file search, git context, structured patching, and resumable sessions are first-class.
4. **Extensibility** — MCP servers, skills, plugins, connectors, and app-server APIs allow teams to add their own tools and workflows.
5. **Integration-ready architecture** — the same agent engine can run as a TUI, a one-shot CLI, an MCP server, or an experimental JSON-RPC app server.

## Highlights

- **Interactive terminal UI** — fullscreen Ratatui interface with streaming responses, command palette, slash commands, approvals, diffs, and session navigation.
- **Non-interactive execution** — `openagere exec` runs one-shot tasks for scripts, terminals, and CI-style workflows.
- **Structured code editing** — file changes are applied through `apply_patch`, making edits explicit, reviewable, and safer than ad-hoc shell redirection.
- **Built-in engineering tools** — repository search, file reads/writes, shell execution, git helpers, image input, code review mode, and diagnostics.
- **Configurable trust model** — read-only, workspace-write, and more permissive workflows can be paired with approval policies for commands and patches.
- **Multiple model backends** — supports OpenAI, Anthropic, and OpenAI-compatible providers through model/provider configuration.
- **MCP client support** — connect OpenAgere to local or remote Model Context Protocol servers and make their tools available to the agent.
- **Experimental MCP server** — expose OpenAgere itself to MCP clients with `openagere mcp-server`.
- **Skills and plugins** — package reusable instructions, references, scripts, assets, and integrations for team-specific workflows.
- **Session persistence** — resume or fork previous interactive sessions and keep rollout/history state under the OpenAgere home directory.
- **App-server protocol** — experimental server mode for IDEs, remote frontends, and custom clients using a versioned JSON-RPC API.
- **Rust workspace** — modular crates for the TUI, core agent engine, protocol, config, MCP, execution policy, app server, plugins, state, and more.

---

## Installation

### npm

The npm package installs the native binary for your platform through optional platform packages.

```bash
npm i -g openagere
openagere
```

### Bun

Bun can install the same npm package and native optional dependency packages.

```bash
bun add -g openagere
openagere
```

### GitHub Releases

Download a platform-specific binary from [GitHub Releases](https://github.com/openagere/openagere/releases). Releases may also include a [DotSlash](https://dotslash-cli.com/) file named `openagere`, which lets teams pin the exact CLI version in source control.

### Build from source

```bash
git clone https://github.com/openagere/openagere.git
cd openagere

# Install Rust if needed.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup component add rustfmt clippy

# Workspace helper.
cargo install just

# Optional faster test runner.
cargo install --locked cargo-nextest

# Build and run.
cargo build
cargo run --bin openagere
```

### System requirements

| Requirement | Details |
| --- | --- |
| Operating systems | macOS 12+, Ubuntu 20.04+/Debian 10+, or Windows 11 via WSL2 |
| Git | Recommended for repository-aware workflows and diff review |
| RAM | 4 GB minimum, 8 GB recommended |

---

## Quick Start

### 1. Launch the TUI

Run OpenAgere in a repository:

```bash
openagere
```

Try prompts such as:

```text
Explain how this repository is organized.
Find the code path that handles login and summarize it.
Add tests for the parser error case and run the focused test.
Refactor this module to reduce duplication without changing behavior.
```

### 2. Ask OpenAgere to inspect before editing

Good coding-agent prompts usually include the goal, constraints, and verification expectation:

```text
Find why `cargo test -p agere-tui wrapping` fails, patch the root cause, run the focused test, and summarize the change.
```

### 3. Run one-shot tasks

Use `exec` when you want a non-interactive result:

```bash
openagere exec "Explain the public API exposed by the config crate"
openagere exec "Update the README examples for the new CLI flag"
git diff | openagere exec "Review this diff and list risks"
```

### 4. Resume previous work

```bash
openagere resume
openagere resume --last "Continue from where we left off and run the focused test"
openagere fork --last "Try a smaller alternative implementation"
```

---

## Common Workflows

### Understand a codebase

```bash
openagere "Map the main crates, explain data flow, and point me to the best entry files"
```

OpenAgere can combine repository search, file reading, and project instructions to produce a practical onboarding summary.

### Implement a focused change

```text
Add a `--json` option to the command output. Keep the existing text output unchanged, update docs, and run the focused tests.
```

The agent will typically inspect command definitions, patch code, update docs when applicable, and ask for approvals before risky commands according to your policy.

### Review code

```bash
openagere review --uncommitted
openagere review --base main
openagere review --commit <sha>
```

Review mode is useful for finding correctness, security, compatibility, and maintainability risks before a pull request.

### Automate from shell scripts

```bash
openagere exec --json "Run the focused test for the changed crate and report failures"
```

The JSON output mode is intended for tooling that wants machine-readable events.

---

## Interactive TUI

The default `openagere` command opens a terminal UI optimized for iterative development.

Useful interactions:

| Action | How |
| --- | --- |
| Submit prompt | `Enter` |
| Interrupt current turn | `Ctrl+C` |
| Exit gracefully | `Ctrl+D` |
| Open command palette | `?` |
| Start slash command | `/` |
| Scroll conversation | `PageUp` / `PageDown` |
| Navigate prompt history | `↑` / `↓` |

Common slash commands:

| Command | Purpose |
| --- | --- |
| `/help` | Show help and available commands |
| `/model` | Switch the active model |
| `/compact` | Summarize/compress long context |
| `/resume` | Resume a previous session |
| `/fork` | Fork a previous session into a new line of work |
| `/init` | Generate or update repository instructions in `AGENTS.md` |
| `/agents` | View active project instructions |
| `/config` | View or edit configuration |
| `/apps` | Work with available connectors |
| `/plugins` | Manage plugins |

See [`docs/slash_commands.md`](./docs/slash_commands.md) for the full slash command reference.

---

## CLI Commands

The binary exposes multiple modes through one command:

| Command | Description |
| --- | --- |
| `openagere` | Start the interactive TUI |
| `openagere <PROMPT>` | Start the TUI with an initial prompt |
| `openagere exec <PROMPT>` | Run a non-interactive task |
| `openagere review` | Run a non-interactive code review |
| `openagere login` | Manage authentication |
| `openagere logout` | Remove stored credentials |
| `openagere resume` | Resume a previous session |
| `openagere fork` | Fork a previous session |
| `openagere mcp` | Manage configured MCP servers |
| `openagere mcp-server` | Run OpenAgere as an MCP server |
| `openagere plugin marketplace` | Manage plugin marketplaces |
| `openagere provider` | Open provider management UI |
| `openagere app-server` | Run experimental app server tooling |
| `openagere exec-server` | Run the standalone exec-server service |
| `openagere completion <shell>` | Generate shell completions |
| `openagere update` | Update a release installation |
| `openagere features list` | Inspect feature flags |

Most commands accept configuration overrides with `-c key=value`. For example:

```bash
openagere -c model="gpt-5" -c access_mode="workspace-write"
openagere exec -c approval_policy="on-request" "Fix the failing test"
```

---

## Configuration

OpenAgere reads configuration from `~/.openagere/config.toml` by default. You can also override values per command with `-c key=value`.

Minimal example:

```toml
# ~/.openagere/config.toml
model = "gpt-5"
access_mode = "workspace-write"
approval_policy = "on-request"
```

Typical configuration areas:

- **Model selection** — choose the default model and reasoning effort.
- **Providers** — configure OpenAI, Anthropic, or OpenAI-compatible endpoints.
- **Approvals** — decide when commands and patch applications require confirmation.
- **Access mode** — control filesystem and network capabilities.
- **MCP servers** — register local stdio servers or remote HTTP servers.
- **Logging** — set log directory and Rust `RUST_LOG` behavior.
- **UI preferences** — configure interaction style and feature flags.

See [`docs/config.md`](./docs/config.md) and [`docs/example-config.md`](./docs/example-config.md) for the full reference.

---

## Approvals and Sandboxing

OpenAgere is built around explicit control of side effects.

- **Filesystem access modes** determine whether the agent can only read, write inside the workspace, or use broader access.
- **Approval policies** determine when the user must confirm shell commands, patches, or privileged actions.
- **Structured patches** make file edits visible and auditable before or as they are applied.
- **Execution policy tooling** helps validate whether commands are allowed under configured policy.
- **Platform restrictions** integrate with OS-level mechanisms where available.

This makes OpenAgere useful both for exploratory read-only analysis and for higher-trust coding sessions where it can edit and test autonomously.

Related docs:

- [`docs/execpolicy.md`](./docs/execpolicy.md)
- [`execpolicy/README.md`](./execpolicy/README.md)
- [`shell-escalation/README.md`](./shell-escalation/README.md)

---

## Models and Providers

OpenAgere can work with multiple model providers. The repository includes provider/client crates for OpenAI-style chat APIs and Anthropic, plus model/provider management components.

Use `/provider` inside the TUI, or run `openagere provider`, to add API keys, pick from remote provider templates, configure a custom OpenAI-compatible endpoint, and switch the active provider.

<p align="center">
  <img src="2.gif" alt="OpenAgere provider setup demo" width="720">
</p>

Common setup paths:

```bash
openagere login
openagere provider
openagere -c model="<model-id>"
```

If you use an OpenAI-compatible endpoint, configure the provider in `config.toml` and select the model with `/model` or `-c model=...`.

For authentication details, see [`docs/authentication.md`](./docs/authentication.md).

---

## MCP, Skills, and Plugins

### MCP client

Register tools from an MCP server:

```bash
openagere mcp add docs -- npx -y @modelcontextprotocol/server-filesystem .
openagere mcp list
openagere mcp remove docs
```

OpenAgere supports stdio-based MCP servers and remote streamable HTTP servers. See [`docs/codex_mcp_interface.md`](./docs/codex_mcp_interface.md) for protocol and integration details.

### MCP server

Run OpenAgere itself as an MCP server:

```bash
openagere mcp-server | your_mcp_client
npx @modelcontextprotocol/inspector openagere mcp-server
```

### Skills

Skills are local instruction packages stored under `$AGERE_HOME/skills/`. A skill can include:

- `SKILL.md` instructions and trigger descriptions
- `scripts/` for reusable helper commands
- `references/` for domain-specific docs
- `assets/` for templates or examples

Install and use skills by placing them under `$AGERE_HOME/skills/` or by asking the built-in `skill-installer` skill to install a curated or GitHub-hosted skill:

```text
Use the skill-installer skill to install <skill-name>.
```

See [`docs/skills.md`](./docs/skills.md).

### Plugins

Plugins package reusable integrations and can be distributed through marketplaces:

```bash
openagere plugin marketplace add owner/repo
openagere plugin marketplace upgrade
openagere plugin marketplace remove <name>
```

---

## Automation and App Server

OpenAgere is not limited to a terminal UI.

- **`openagere exec`** is the simplest automation interface for scripts and CI-like tasks.
- **`openagere exec --json`** emits structured events for tooling.
- **`openagere app-server`** exposes an experimental JSON-RPC app-server API for IDEs or remote frontends.
- **`openagere exec-server`** runs the standalone execution server service.
- **`app-server-protocol`** defines versioned Rust and TypeScript protocol shapes.

Useful references:

- [`docs/exec.md`](./docs/exec.md)
- [`app-server/README.md`](./app-server/README.md)
- [`app-server-client/README.md`](./app-server-client/README.md)
- [`protocol/README.md`](./protocol/README.md)

---

## Repository Layout

This repository is a Rust Cargo workspace. Important crates and directories include:

| Path | Purpose |
| --- | --- |
| `cli/` | Top-level `openagere` binary and subcommand dispatch |
| `tui/` | Interactive terminal UI |
| `core/` | Core agent runtime and orchestration |
| `exec/` | Non-interactive execution mode |
| `app-server/` | Experimental app-server implementation |
| `app-server-protocol/` | Versioned app-server API types and schema generation |
| `agere-mcp/`, `rmcp-client/` | MCP integration |
| `execpolicy/`, `shell-escalation/` | Execution policy and approval behavior |
| `config/` | Configuration loading and schema support |
| `skills/`, `core-skills/` | Skill system and built-in skills |
| `plugin/`, `core-plugins/` | Plugin system |
| `model-provider/`, `models-manager/` | Provider and model catalog support |
| `thread-store/`, `state/`, `rollout-trace/` | Session, state, and trace persistence |
| `docs/` | User and developer documentation |

---

## Development

Common development commands:

```bash
# Format Rust code.
just fmt

# Fix lints for a specific crate.
just fix -p agere-tui

# Run focused tests.
cargo test -p agere-tui

# Run the full suite with nextest, if installed.
just test

# Generate config schema after config API changes.
just write-config-schema

# Generate app-server schema after protocol changes.
just write-app-server-schema
```

Development notes:

- Follow [`AGENTS.md`](./AGENTS.md) for repository-specific coding rules.
- Prefer focused crate tests before workspace-wide tests.
- UI changes should include or update relevant `insta` snapshots.
- Avoid routine `--all-features` test runs unless you specifically need full feature coverage.

---

## Documentation

- [`docs/getting-started.md`](./docs/getting-started.md) — first session walkthrough
- [`docs/install.md`](./docs/install.md) — installation and build details
- [`docs/config.md`](./docs/config.md) — configuration reference
- [`docs/example-config.md`](./docs/example-config.md) — example configuration
- [`docs/exec.md`](./docs/exec.md) — non-interactive execution
- [`docs/slash_commands.md`](./docs/slash_commands.md) — TUI slash commands
- [`docs/skills.md`](./docs/skills.md) — custom skills
- [`docs/agents_md.md`](./docs/agents_md.md) — project instructions with `AGENTS.md`
- [`docs/contributing.md`](./docs/contributing.md) — contribution guide
- [`docs/license.md`](./docs/license.md) — licensing notes

---

## Security, Contributing, and License

- Report security issues using [`SECURITY.md`](./SECURITY.md).
- Contributions are welcome; start with [`docs/contributing.md`](./docs/contributing.md).
- OpenAgere is licensed under Apache-2.0; see [`LICENSE`](./LICENSE) and [`NOTICE`](./NOTICE).
