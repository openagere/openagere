# OpenAgere: A Fast-Start Terminal AI Coding Agent

<p align="center">
  <img src="../1.gif" alt="OpenAgere interactive TUI demo" width="720">
</p>

OpenAgere is an open-source, Rust-powered terminal AI coding agent. It is built from a fork of OpenAI's Codex CLI Rust implementation, with substantial follow-up work around model providers, plugins, local developer workflows, TUI interaction, and real-world engineering use cases.

It is not just a command-line chatbot. OpenAgere is designed for repository-aware development: it can inspect codebases, follow project instructions, edit files, run commands, review diffs, resume previous sessions, and connect to external tools through MCP, Skills, and Plugins.

## Quick Start: Bring Your API Key

OpenAgere is designed to be easy to start.

Install it, run:

```bash
openagere
```

Then select a Provider, enter your `api_key`, and start coding with the agent.

Inside a repository, you can simply run:

```bash
openagere
```

Then ask something like:

```text
Map this repository, explain the main modules, and point me to the best entry files.
```

OpenAgere will use your repository context, project instructions, and available tools to produce a practical engineering-oriented answer.

## Provider: Model Setup Without the Friction

<p align="center">
  <img src="../2.gif" alt="OpenAgere provider setup demo" width="720">
</p>

OpenAgere includes built-in Provider management. You can open it from the TUI with `/provider`, or run:

```bash
openagere provider
```

Provider support is built around three wire protocols:

- **OpenAI Responses API**: `/v1/responses`
- **Anthropic Messages API**: `/v1/messages`
- **OpenAI Chat Completions API**: `/v1/chat/completions`

It supports:

- OpenAI
- Anthropic
- OpenAI-compatible APIs
- Custom Providers
- Remote Provider templates
- Aggregator and gateway providers such as OpenRouter

In many cases, you do not need to manually write complex configuration. Choose a Provider, paste your `api_key`, and start using the agent immediately.

Because OpenAgere supports the mainstream Responses, Anthropic Messages, and Chat Completions protocols, it can quickly connect to providers such as OpenRouter and other model gateways that expose OpenAI-compatible or Anthropic-compatible endpoints.

## Installation

Using npm:

```bash
npm i -g openagere
openagere
```

Using Bun:

```bash
bun add -g openagere
openagere
```

The package installs platform-specific native binaries through optional dependencies. The launcher requires Node.js 18+.

## Plugins: Bring Your Team Tools Into the Agent

<p align="center">
  <img src="../plugins.png" alt="OpenAgere plugin management" width="720">
</p>

OpenAgere supports Plugins and Skills, allowing teams to package reusable workflows, instructions, references, scripts, and integrations.

OpenAgere is also compatible with Codex-style plugins: plugin manifests under `.codex-plugin/plugin.json` are recognized alongside OpenAgere's `.agere-plugin/plugin.json`, so existing Codex plugin layouts can be reused with minimal migration.

You can extend OpenAgere with:

- Internal engineering workflows
- Code review rules
- Project scaffolding
- API documentation lookup
- Database or business-system operations
- MCP tool integrations
- Team-specific agent workflows

This makes OpenAgere more than a general-purpose coding assistant. It can become a terminal-native agent platform tailored to your team.

## Why OpenAgere

OpenAgere is built for developers and teams that want fast onboarding without giving up extensibility.

Highlights:

- **Fast to start**: choose a Provider, enter an `api_key`, and use it immediately.
- **Terminal-native**: works where developers already spend their time.
- **Rich TUI**: streaming output, approvals, diffs, slash commands, and resumable sessions.
- **Flexible model access**: OpenAI Responses, Anthropic Messages, Chat Completions, OpenAI-compatible APIs, and custom Providers.
- **Fast provider onboarding**: quick setup for OpenRouter, custom gateways, and other compatible model services.
- **Plugin extensibility**: extend through Plugins, Skills, MCP, and Codex-compatible plugin layouts.
- **Engineering-ready**: code edits, shell commands, diff review, non-interactive execution, and app-server integration.
- **Built on Codex**: forked from Codex CLI's Rust implementation and extended for practical engineering workflows.

## In One Sentence

OpenAgere is an open-source terminal AI coding agent that is easy to start, Provider-friendly, plugin-ready, and suitable for modern development environments.

Bring an API key, choose a Provider, and start collaborating with an AI agent inside your codebase.
