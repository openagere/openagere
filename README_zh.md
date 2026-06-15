<div align="center">

# OpenAgere CLI

**开源、终端原生、由 Rust 驱动的 AI 编程 Agent。**

[English](./README.md) | [中文](./README_zh.md)

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://rust-lang.org)

</div>

---

OpenAgere 是一个基于 Rust 构建的开源终端 AI 编程 Agent，基于 OpenAI Codex CLI 的 Rust 实现二次改造而来，并在模型 Provider、插件系统、国内使用体验、TUI 交互和工程化能力上做了大量增强。

它可以理解代码仓库、解释实现、修改文件、运行命令、审查 diff、恢复历史会话，并通过 MCP、Skills、Plugins 和 App Server 接入团队自己的工具链。

它不是简单的一问一答聊天工具，而是面向真实工程工作的 Agent：能读取项目级指令，能在 TUI 中持续协作，能用非交互模式自动完成任务，也能通过审批和沙箱机制控制文件、命令和网络等副作用。

<p align="center">
  <img src="1.gif" alt="OpenAgere 交互式 TUI 演示" width="720">
</p>

## Source provenance

OpenAgere CLI is developed from a fork of [OpenAI's Codex CLI](https://github.com/openai/codex) (Rust implementation). This tree contains substantial follow-on changes and project-specific behavior; licensing and third-party notices are summarized in [`LICENSE`](./LICENSE) and [`NOTICE`](./NOTICE).

---

## 目录

- [项目定位](#项目定位)
- [核心亮点](#核心亮点)
- [安装](#安装)
- [快速开始](#快速开始)
- [常见使用场景](#常见使用场景)
- [交互式 TUI](#交互式-tui)
- [CLI 命令](#cli-命令)
- [配置](#配置)
- [审批与沙箱](#审批与沙箱)
- [模型与 Provider](#模型与-provider)
- [MCP、Skills 与 Plugins](#mcpskills-与-plugins)
- [自动化与 App Server](#自动化与-app-server)
- [仓库结构](#仓库结构)
- [开发](#开发)
- [更多文档](#更多文档)
- [安全、贡献与许可证](#安全贡献与许可证)

---

## 项目定位

现代编程 Agent 不只需要“能聊天”，还需要能理解仓库、能安全地改代码、能调用工具、能保留上下文、能被 IDE 或自动化系统集成。

OpenAgere 重点解决五类问题：

1. **终端优先的开发体验**：单个 Rust 二进制，既有交互式 TUI，也有可脚本化的命令模式。
2. **可控的执行能力**：文件写入、命令执行、网络访问、补丁应用都可以被 access mode 与 approval policy 管控。
3. **仓库级理解能力**：支持 `AGENTS.md` 项目指令、代码搜索、文件读取、git 上下文、结构化 patch、会话恢复。
4. **可扩展工具生态**：通过 MCP、Skills、Plugins、Connectors 和 App Server 接入团队内部系统和专业工作流。
5. **适合工程集成的架构**：同一套 Agent 能以 TUI、一次性 CLI、MCP Server、实验性 JSON-RPC App Server 等方式运行。

## 核心亮点

- **交互式终端 UI**：基于 Ratatui 的全屏界面，支持流式输出、命令面板、Slash Commands、审批、diff 展示和会话导航。
- **非交互执行模式**：`openagere exec` 可用于脚本、批处理和 CI 风格任务。
- **结构化代码修改**：通过 `apply_patch` 应用文件修改，变更可审阅、可追踪，比任意 shell 重定向更安全。
- **内置工程工具**：代码搜索、文件读写、shell 执行、git 辅助、图片输入、代码审查、诊断工具等。
- **可配置信任模型**：从只读分析到工作区写入，再到更高权限流程，都可以搭配不同审批策略。
- **多模型后端**：支持 OpenAI、Anthropic 以及 OpenAI-compatible API，可通过配置或 `/model` 切换。
- **MCP Client**：连接本地或远程 Model Context Protocol Server，让 Agent 使用外部工具。
- **实验性 MCP Server**：用 `openagere mcp-server` 将 OpenAgere 暴露给其他 MCP Client。
- **Skills 与 Plugins**：沉淀团队专属指令、参考资料、脚本、模板和工具集成。
- **会话持久化**：支持恢复、fork 历史会话，并在 OpenAgere home 下保存状态和 rollout trace。
- **App Server 协议**：实验性 JSON-RPC 服务，适合 IDE、远程前端和自定义客户端集成。
- **Rust 模块化工作区**：TUI、核心运行时、协议、配置、MCP、执行策略、插件、状态等能力拆分为多个 crate。

---

## 安装

### npm 安装

npm 包会通过 optional dependencies 自动安装当前平台对应的原生二进制。

```bash
npm i -g openagere
openagere
```

### Bun 安装

Bun 可以安装同一个 npm 包，并自动安装对应平台的原生 optional dependency。

```bash
bun add -g openagere
openagere
```

### GitHub Releases

也可以从 [GitHub Releases](https://github.com/openagere/openagere/releases) 下载平台对应的二进制。Release 中还可能包含名为 `openagere` 的 [DotSlash](https://dotslash-cli.com/) 文件，方便团队把固定版本的 CLI 放进源码仓库统一使用。

### 从源码构建

```bash
git clone https://github.com/openagere/openagere.git
cd openagere

# 如未安装 Rust，先安装 Rust 工具链。
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup component add rustfmt clippy

# 安装 workspace 辅助命令。
cargo install just

# 可选：更快的测试运行器。
cargo install --locked cargo-nextest

# 构建并运行。
cargo build
cargo run --bin openagere
```

### 系统要求

| 要求 | 说明 |
| --- | --- |
| 操作系统 | macOS 12+、Ubuntu 20.04+/Debian 10+，或 Windows 11 via WSL2 |
| Git | 推荐安装，用于仓库上下文、diff 与代码审查能力 |
| 内存 | 最低 4 GB，推荐 8 GB |

---

## 快速开始

### 1. 启动 TUI

在代码仓库目录运行：

```bash
openagere
```

可以尝试这些 prompt：

```text
解释这个仓库的整体结构。
找到登录逻辑的代码路径并总结实现。
为 parser 的错误分支补测试，并运行相关测试。
重构这个模块，减少重复逻辑，但不要改变行为。
```

### 2. 让 Agent 先理解再修改

好的编程 Agent prompt 通常包含目标、约束和验证方式：

```text
定位 `cargo test -p agere-tui wrapping` 失败的根因，修复它，运行聚焦测试，并总结改动。
```

### 3. 一次性执行任务

如果不需要交互式界面，可以使用 `exec`：

```bash
openagere exec "解释 config crate 对外暴露的 public API"
openagere exec "为新的 CLI 参数更新 README 示例"
git diff | openagere exec "审查这个 diff，列出风险点"
```

### 4. 恢复或 fork 会话

```bash
openagere resume
openagere resume --last "从上次中断处继续，并运行聚焦测试"
openagere fork --last "尝试一个更小的替代实现"
```

---

## 常见使用场景

### 理解陌生代码仓库

```bash
openagere "梳理主要 crate、解释数据流，并指出最适合开始阅读的入口文件"
```

OpenAgere 会结合代码搜索、文件读取和项目指令，生成面向工程实践的上手说明。

### 完成聚焦代码变更

```text
为命令输出增加 `--json` 选项。保持现有文本输出不变，更新文档，并运行相关测试。
```

Agent 通常会先阅读命令定义，再修改代码、补文档，并根据你的审批策略请求运行命令或应用补丁。

### 代码审查

```bash
openagere review --uncommitted
openagere review --base main
openagere review --commit <sha>
```

Review 模式适合在提交 PR 前发现正确性、安全性、兼容性和可维护性风险。

### Shell 自动化

```bash
openagere exec --json "运行变更 crate 的聚焦测试，并以结构化方式报告失败"
```

JSON 输出模式适合被工具或脚本消费。

---

## 交互式 TUI

默认的 `openagere` 命令会打开适合迭代开发的终端 UI。

常用操作：

| 操作 | 按键 |
| --- | --- |
| 提交 prompt | `Enter` |
| 中断当前 turn | `Ctrl+C` |
| 优雅退出 | `Ctrl+D` |
| 打开命令面板 | `?` |
| 输入 Slash Command | `/` |
| 滚动会话 | `PageUp` / `PageDown` |
| 浏览历史输入 | `↑` / `↓` |

常见 Slash Commands：

| 命令 | 用途 |
| --- | --- |
| `/help` | 查看帮助和可用命令 |
| `/model` | 切换当前模型 |
| `/compact` | 压缩长上下文 |
| `/resume` | 恢复历史会话 |
| `/fork` | 从历史会话 fork 出新分支 |
| `/init` | 生成或更新仓库级 `AGENTS.md` 指令 |
| `/agents` | 查看当前生效的项目指令 |
| `/config` | 查看或编辑配置 |
| `/apps` | 查看或使用 connectors |
| `/plugins` | 管理插件 |

完整说明见 [`docs/slash_commands.md`](./docs/slash_commands.md)。

---

## CLI 命令

OpenAgere 用一个二进制提供多种运行模式：

| 命令 | 说明 |
| --- | --- |
| `openagere` | 启动交互式 TUI |
| `openagere <PROMPT>` | 带初始 prompt 启动 TUI |
| `openagere exec <PROMPT>` | 非交互执行任务 |
| `openagere review` | 非交互代码审查 |
| `openagere login` | 管理认证 |
| `openagere logout` | 删除本地凭据 |
| `openagere resume` | 恢复历史会话 |
| `openagere fork` | fork 历史会话 |
| `openagere mcp` | 管理 MCP Server 配置 |
| `openagere mcp-server` | 以 MCP Server 方式运行 OpenAgere |
| `openagere plugin marketplace` | 管理插件市场 |
| `openagere provider` | 打开 Provider 管理 UI |
| `openagere app-server` | 运行实验性 App Server 相关功能 |
| `openagere exec-server` | 运行独立 exec-server 服务 |
| `openagere completion <shell>` | 生成 shell completion |
| `openagere update` | 更新 release 安装版本 |
| `openagere features list` | 查看 feature flags |

多数命令支持用 `-c key=value` 做临时配置覆盖：

```bash
openagere -c model="gpt-5" -c access_mode="workspace-write"
openagere exec -c approval_policy="on-request" "修复失败测试"
```

---

## 配置

OpenAgere 默认读取 `~/.openagere/config.toml`。也可以通过命令行 `-c key=value` 临时覆盖配置。

最小示例：

```toml
# ~/.openagere/config.toml
model = "gpt-5"
access_mode = "workspace-write"
approval_policy = "on-request"
```

常见配置方向：

- **模型选择**：设置默认模型和 reasoning effort。
- **Provider**：配置 OpenAI、Anthropic 或 OpenAI-compatible endpoint。
- **审批策略**：控制命令执行和 patch 应用何时需要确认。
- **访问模式**：限制文件系统和网络能力。
- **MCP Server**：注册本地 stdio 或远程 HTTP MCP Server。
- **日志**：设置日志目录和 Rust `RUST_LOG` 行为。
- **UI 偏好**：配置交互体验和 feature flags。

完整参考见 [`docs/config.md`](./docs/config.md) 和 [`docs/example-config.md`](./docs/example-config.md)。

---

## 审批与沙箱

OpenAgere 的核心设计之一是显式控制副作用。

- **Filesystem access mode** 决定 Agent 只能读、能写工作区，还是能使用更宽权限。
- **Approval policy** 决定何时需要用户确认 shell 命令、patch 或高权限动作。
- **结构化 patch** 让代码变更在应用前后都更容易审阅。
- **Execution policy 工具** 可以检查命令是否符合当前策略。
- **平台限制能力** 会在可用系统上结合 OS 级机制使用。

因此，OpenAgere 既可以用于低风险的只读分析，也可以在更高信任场景中自动编辑、运行测试和迭代修复。

相关文档：

- [`docs/execpolicy.md`](./docs/execpolicy.md)
- [`execpolicy/README.md`](./execpolicy/README.md)
- [`shell-escalation/README.md`](./shell-escalation/README.md)

---

## 模型与 Provider

OpenAgere 支持多种模型 Provider。仓库中包含 OpenAI 风格 Chat API、Anthropic、模型目录和 Provider 管理相关组件。

在 TUI 中输入 `/provider`，或运行 `openagere provider`，可以添加 API Key、从远程 Provider 模板中选择、配置自定义 OpenAI-compatible endpoint，并切换当前使用的 Provider。

<p align="center">
  <img src="2.gif" alt="OpenAgere Provider 配置演示" width="720">
</p>

常见入口：

```bash
openagere login
openagere provider
openagere -c model="<model-id>"
```

如果使用 OpenAI-compatible endpoint，可以在 `config.toml` 中配置 Provider，再通过 `/model` 或 `-c model=...` 选择模型。

认证细节见 [`docs/authentication.md`](./docs/authentication.md)。

---

## MCP、Skills 与 Plugins

### MCP Client

注册 MCP Server 提供的工具：

```bash
openagere mcp add docs -- npx -y @modelcontextprotocol/server-filesystem .
openagere mcp list
openagere mcp remove docs
```

OpenAgere 支持 stdio MCP Server 和远程 streamable HTTP MCP Server。协议与集成细节见 [`docs/codex_mcp_interface.md`](./docs/codex_mcp_interface.md)。

### MCP Server

把 OpenAgere 作为 MCP Server 运行：

```bash
openagere mcp-server | your_mcp_client
npx @modelcontextprotocol/inspector openagere mcp-server
```

### Skills

Skills 是存放在 `$AGERE_HOME/skills/` 下的本地能力包，可以包含：

- `SKILL.md`：技能说明、触发条件和操作指令
- `scripts/`：可复用脚本
- `references/`：领域参考资料
- `assets/`：模板、图片或示例文件

可以把 Skill 放到 `$AGERE_HOME/skills/` 下，也可以让内置 `skill-installer` Skill 安装 curated skill 或 GitHub 上的 skill：

```text
使用 skill-installer skill 安装 <skill-name>。
```

更多说明见 [`docs/skills.md`](./docs/skills.md)。

### Plugins

Plugins 用于打包可复用集成，也可以通过 marketplace 分发：

```bash
openagere plugin marketplace add owner/repo
openagere plugin marketplace upgrade
openagere plugin marketplace remove <name>
```

---

## 自动化与 App Server

OpenAgere 不只是一套终端 UI。

- **`openagere exec`** 是最简单的脚本化和 CI 风格入口。
- **`openagere exec --json`** 输出结构化事件，方便工具消费。
- **`openagere app-server`** 提供实验性 JSON-RPC App Server API，适合 IDE 或远程前端。
- **`openagere exec-server`** 运行独立执行服务。
- **`app-server-protocol`** 定义 Rust 与 TypeScript 的版本化协议结构。

参考文档：

- [`docs/exec.md`](./docs/exec.md)
- [`app-server/README.md`](./app-server/README.md)
- [`app-server-client/README.md`](./app-server-client/README.md)
- [`protocol/README.md`](./protocol/README.md)

---

## 仓库结构

本仓库是一个 Rust Cargo workspace，主要目录包括：

| 路径 | 作用 |
| --- | --- |
| `cli/` | 顶层 `openagere` 二进制与子命令分发 |
| `tui/` | 交互式终端 UI |
| `core/` | Agent 核心运行时与编排逻辑 |
| `exec/` | 非交互执行模式 |
| `app-server/` | 实验性 App Server 实现 |
| `app-server-protocol/` | App Server API 类型与 schema 生成 |
| `agere-mcp/`, `rmcp-client/` | MCP 集成 |
| `execpolicy/`, `shell-escalation/` | 执行策略与审批行为 |
| `config/` | 配置加载与 schema 支持 |
| `skills/`, `core-skills/` | Skills 系统和内置 Skills |
| `plugin/`, `core-plugins/` | 插件系统 |
| `model-provider/`, `models-manager/` | Provider 与模型目录支持 |
| `thread-store/`, `state/`, `rollout-trace/` | 会话、状态和 trace 持久化 |
| `docs/` | 用户和开发者文档 |

---

## 开发

常用开发命令：

```bash
# 格式化 Rust 代码。
just fmt

# 修复指定 crate 的 lint。
just fix -p agere-tui

# 运行聚焦测试。
cargo test -p agere-tui

# 如已安装 nextest，运行完整测试套件。
just test

# 修改配置 API 后生成配置 schema。
just write-config-schema

# 修改 App Server 协议后生成 schema。
just write-app-server-schema
```

开发注意事项：

- 遵循 [`AGENTS.md`](./AGENTS.md) 中的仓库级规则。
- 优先运行受影响 crate 的聚焦测试，再考虑 workspace 级测试。
- UI 文案或渲染变化应补充或更新相关 `insta` snapshot。
- 日常本地验证不建议默认使用 `--all-features`，除非确实需要完整 feature 覆盖。

---

## 更多文档

- [`docs/getting-started.md`](./docs/getting-started.md) — 第一次使用指南
- [`docs/install.md`](./docs/install.md) — 安装和构建说明
- [`docs/config.md`](./docs/config.md) — 配置参考
- [`docs/example-config.md`](./docs/example-config.md) — 配置示例
- [`docs/exec.md`](./docs/exec.md) — 非交互执行模式
- [`docs/slash_commands.md`](./docs/slash_commands.md) — TUI Slash Commands
- [`docs/skills.md`](./docs/skills.md) — 自定义 Skills
- [`docs/agents_md.md`](./docs/agents_md.md) — 使用 `AGENTS.md` 管理项目指令
- [`docs/contributing.md`](./docs/contributing.md) — 贡献指南
- [`docs/license.md`](./docs/license.md) — 许可证说明

---

## 安全、贡献与许可证

- 安全问题请参考 [`SECURITY.md`](./SECURITY.md)。
- 欢迎贡献代码和文档，建议先阅读 [`docs/contributing.md`](./docs/contributing.md)。
- OpenAgere 使用 Apache-2.0 许可证，详见 [`LICENSE`](./LICENSE) 和 [`NOTICE`](./NOTICE)。
