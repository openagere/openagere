# OpenAgere 端到端详细流程分析

> 从 CLI 入口到 Agent 交互完成，涵盖所有关键通道消息、Turn 状态机、LLM 调用、工具执行与 TUI 显示

---

## 目录

0. [Rust 与 Agent 概念入门](#0-rust-与-agent-概念入门)
1. [项目架构总览](#1-项目架构总览)
2. [CLI 入口与启动流程](#2-cli-入口与启动流程)
3. [配置加载与解析](#3-配置加载与解析)
4. [TUI 初始化与显示框架](#4-tui-初始化与显示框架)
5. [AppServer 启动与嵌入模式](#5-appserver-启动与嵌入模式)
6. [通道系统总览与消息传递架构](#6-通道系统总览与消息传递架构)
7. [Agere 核心结构与会话创建](#7-agere-核心结构与会话创建)
8. [Submission 提交循环](#8-submission-提交循环)
9. [用户输入处理与 Turn 状态机](#9-用户输入处理与-turn-状态机)
10. [LLM 调用与流式响应处理](#10-llm-调用与流式响应处理)
11. [工具调用与执行流程](#11-工具调用与执行流程)
12. [事件发送与消费链路](#12-事件发送与消费链路)
13. [子代理生成与代理间通信](#13-子代理生成与代理间通信)
14. [TUI 事件处理与渲染管线](#14-tui-事件处理与渲染管线)
15. [流式内容显示与提交动画](#15-流式内容显示与提交动画)
16. [关闭与清理流程](#16-关闭与清理流程)
17. [完整实例追踪：一次真实的 Agent 交互](#17-完整实例追踪一次真实的-agent-交互)
18. [ModelProvider 与 ModelsManager](#18-modelprovider-与-modelsmanager)
19. [Model 切换流程——/model 命令详解](#19-model-切换流程model-命令详解)
20. [Context Compact——上下文压缩](#20-context-compact上下文压缩)
21. [spawn_agent V2——新一代子代理生成](#21-spawn_agent-v2新一代子代理生成)
22. [/ 命令菜单体系](#22--命令菜单体系)
23. [Skills & Plugins 扩展系统](#23-skills--plugins-扩展系统)
24. [Realtime Conversation 实时对话](#24-realtime-conversation-实时对话)
25. [Session 恢复与 Fork](#25-session-恢复与-fork)
26. [远程模式与 AppServer 分离](#26-远程模式与-appserver-分离)
27. [特殊工具实现 —— 内置原生工具](#27-特殊工具实现-内置原生工具)
[附录A：初学者速查表](#附录a初学者速查表)
[附录B：关键文件索引](#附录b关键文件索引)

---

## 0. Rust 与 Agent 概念入门

> 本章为 Rust 初学者和 Agent 领域新手准备。如果你已经熟悉这些概念，可以直接跳到第1章。

### 0.1 Rust 异步并发概念速览

OpenAgere 大量使用 Rust 的异步并发原语。以下是本项目中最常见的几种，理解它们对阅读后续内容至关重要。

#### 通道 (Channel)：组件间的"管道"

通道是 Rust 异步编程中最基础的通信方式，类比于现实中的**管道**——数据从一端流入，从另一端流出。

| 通道类型 | 类比 | 特点 | 本项目中的典型用途 |
|----------|------|------|-------------------|
| `async_channel::bounded(N)` | 有容量限制的管道 | 缓冲区满时发送者被阻塞（背压） | 提交队列（512容量），防止内存爆炸 |
| `async_channel::unbounded()` | 无限容量的管道 | 发送者永不阻塞，但内存可能无限增长 | 事件流，确保事件不丢失 |
| `tokio::mpsc::channel(N)` | 多生产者单消费者管道 | 多个发送端，一个接收端 | AppServer 的消息分发 |
| `tokio::mpsc::unbounded_channel()` | 无界多生产者管道 | 同上但无背压 | TUI 内部 AppEvent 传递 |
| `tokio::broadcast::channel(N)` | 广播喇叭 | 一条消息，多个接收者同时收到 | 帧绘制通知、进程输出、线程创建事件 |
| `tokio::watch::channel(init)` | 状态公告牌 | 保留最新值，新订阅者立即看到当前值 | AgentStatus（Running/Completed...） |
| `tokio::oneshot::channel()` | 一次性传话筒 | 只发一次，只收一次 | 请求-响应配对（"请审批这个命令"→"同意"） |

**关键理解：**
```
// bounded: 管道有容量，满了就等
let (tx, rx) = async_channel::bounded(512);
tx.send(msg).await;  // 如果管道满了512条，这里会阻塞等待

// unbounded: 管道无限，永远不阻塞发送者
let (tx, rx) = async_channel::unbounded();
tx.send(msg).await;  // 永远不会阻塞

// broadcast: 一人广播，多人收听
let (tx, _rx) = broadcast::channel(128);
let rx1 = tx.subscribe();  // 订阅者1
let rx2 = tx.subscribe();  // 订阅者2
tx.send(msg);              // rx1 和 rx2 都能收到
```

#### Arc：共享所有权

```rust
// Arc = Atomic Reference Counted（原子引用计数）
// 类比：多人合租一套房子，每个人都有钥匙
let session = Arc::new(Session::new(...));
let clone1 = Arc::clone(&session);  // 引用计数+1，指向同一个Session
let clone2 = Arc::clone(&session);  // 引用计数+1，指向同一个Session
// 只有当所有人都归还钥匙（引用计数归零），Session才会被释放
```

在本项目中，`Session` 被 `Arc` 包裹后在多个地方共享：
- `submission_loop` 持有 `Arc<Session>`（处理提交）
- `Agere` 持有 `Arc<Session>`（外部访问）
- 子代理持有父 `Arc<Session>` 的克隆（审批冒泡）

#### tokio::spawn：启动异步任务

```rust
// 类比：雇人去做一件事，自己做自己的，互不阻塞
tokio::spawn(async move {
    // 这个代码块在后台运行
    submission_loop(session, config, rx_sub).await;
});
// 主线程继续执行，不会等待上面的任务完成
```

#### tokio::select!：同时等待多件事

```rust
// 类比：同时等三个快递，哪个先到就先处理哪个
loop {
    select! {
        msg = channel_a.recv() => { /* 处理A */ }
        msg = channel_b.recv() => { /* 处理B */ }
        _ = signal.cancelled() => { break; /* 退出 */ }
    }
}
```

TUI 主循环正是用 `select!` 同时等待四种事件：键盘输入、内部消息、线程事件、服务器通知。

### 0.2 Agent 领域核心概念

#### 什么是 Agent？

一个 Agent 是一个**能自主决策和行动**的 AI 程序。它不同于传统的"一问一答"聊天机器人：

```
传统 ChatBot:
  用户: "1+1等于多少？"
  AI:   "等于2"                    ← 一次回答就结束

Agent:
  用户: "帮我写一个Python函数，读取CSV文件并计算平均值"
  AI:   "让我先看看项目中有没有CSV文件..."     ← 自主决定调用工具
  → 调用 search_file 工具                   ← 工具执行
  → 调用 read_file 工具                     ← 工具执行
  AI:   "我看到了你的数据结构，这是处理函数..."  ← 基于工具结果回答
```

#### 核心概念：Turn（回合）

一个 **Turn** 是一次完整的"用户输入 → AI处理 → 最终回复"的周期。一个Turn内部可能包含多次LLM调用和工具执行：

```
Turn 开始
  ├── 第1次 LLM 调用 → 模型决定调用 read_file 工具
  │   └── 工具执行 → read_file("data.csv")
  ├── 第2次 LLM 调用 → 模型决定调用 grep 工具
  │   └── 工具执行 → grep("import pandas")
  └── 第3次 LLM 调用 → 模型有了足够信息，生成最终回复
Turn 结束
```

#### 核心概念：Submission（提交）

**Submission** 是外部（TUI/CLI/远程客户端）向 Agent 发送操作的方式。每种操作都是 `Op` 枚举的一个变体：

```
Op::UserInput       → "用户输入了一段文本"
Op::Interrupt       → "用户按了Ctrl+C，中断当前操作"
Op::Shutdown        → "关闭会话"
Op::ExecApproval    → "用户同意/拒绝了那个命令的执行"
Op::Compact         → "历史太长了，压缩一下"
...
```

Submission 通过**有界通道**（`tx_sub`）发送，确保不会无限排队。

#### 核心概念：Event（事件）

**Event** 是 Agent 向外部（TUI/CLI/远程客户端）报告状态变化的方式。每种事件都是 `EventMsg` 枚举的一个变体：

```
EventMsg::TurnStarted      → "一个Turn开始了"
EventMsg::AgentMessageDelta → "AI正在打字，这是最新的一小段文字"
EventMsg::TurnComplete      → "Turn完成了，最后的回复是..."
EventMsg::ExecCommandBegin  → "开始执行这个shell命令了"
EventMsg::Error             → "出错了！"
...
```

Event 通过**无界通道**（`tx_event`）发送，确保任何事件都不会丢失。

#### 核心概念：Session（会话）

**Session** 是 Agent 的全部运行时状态：对话历史、配置、可用工具、MCP连接、hook管理器等。它 =
 是一个 `Arc<Session>`，在多个地方共享引用。

#### 核心概念：AppServer（应用服务器）

**AppServer** 是 Agent 和外部世界之间的**桥梁**。它负责：
1. 接收客户端连接（WebSocket/stdio/进程内）
2. 将客户端的 JSON-RPC 请求转换为核心引擎的 Op/Submission
3. 将核心引擎的 Event 转换为 JSON-RPC 通知发送给客户端

```
TUI/CLI客户端 ←→ AppServer（JSON-RPC） ←→ Core（Op/Event 通道）
```

### 0.3 一张图理解整体架构

```
┌─────────────────────────────────────────────────────────┐
│                      用户（键盘输入）                      │
└─────────────────────┬───────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────┐
│  TUI 层 (agere-tui)                                     │
│  • 渲染终端界面 (ratatui + crossterm)                     │
│  • 处理键盘事件                                          │
│  • 接收服务器通知并更新显示                               │
│  • 内嵌模式: 通过 mpsc 通道与 AppServer 通信              │
│  • 远程模式: 通过 WebSocket 与远端 AppServer 通信         │
└─────────────────────┬───────────────────────────────────┘
                      │  JSON-RPC over mpsc/WebSocket
                      ▼
┌─────────────────────────────────────────────────────────┐
│  AppServer 层 (agere-app-server)                        │
│  • 管理客户端连接 (接受/断开/认证)                        │
│  • JSON-RPC 消息路由 (method → Op)                      │
│  • Event → JSON-RPC 通知转换                            │
│  • 进程内/远程 两种运行模式                               │
└─────────────────────┬───────────────────────────────────┘
                      │  tx_sub (提交 Op) / rx_event (接收 Event)
                      ▼
┌─────────────────────────────────────────────────────────┐
│  Core 引擎 (agere-core)                                  │
│  ┌─────────────────────────────────────────────────┐    │
│  │  submission_loop (事件循环)                       │    │
│  │  • 接收 Submission, 分派到处理器                  │    │
│  │  • user_input_or_turn → run_turn                 │    │
│  └─────────────────────┬───────────────────────────┘    │
│                        │                                 │
│  ┌─────────────────────▼───────────────────────────┐    │
│  │  run_turn (Turn状态机)                            │    │
│  │  • 构建 prompt (对话历史+指令+工具列表)             │    │
│  │  • 调用 LLM (Anthropic/Responses API)             │    │
│  │  • 解析响应 (文本增量 / 工具调用)                   │    │
│  │  • 执行工具 (并行/串行)                            │    │
│  │  • 将工具结果反馈给 LLM (循环)                     │    │
│  └─────────────────────────────────────────────────┘    │
│                                                         │
│  Session: 持有对话历史、配置、工具注册表、MCP管理器等     │
└─────────────────────────────────────────────────────────┘
```


> **扩展阅读**：本书文档后续章节深入讲解的功能模块，建议先理解以下核心概念：
> - **Tool / Tool Handler**：Agent 的工具系统 — 从注册到执行的完整 6 阶段生命周期（详见 Ch21）
> - **Turn 状态机**：一次用户交互的完整周期，包含多层循环（详见 Ch9）
> - **Compact 上下文压缩**：对话太长时自动摘要压缩（详见 Ch27）
> - **Hook 系统**：Agent 执行生命周期中的外部介入点（详见 Ch23）
> - **MCP (Model Context Protocol)**：让 Agent 连接外部服务的开放协议（详见 Ch22）
> - **Agent 创建**：`Agere::spawn` 的 7 层调用链（详见 Ch25）
> - **多代理协作**：子代理生成、Mailbox 通信、审批冒泡（详见 Ch26）

---

## 1. 项目架构总览

### 1.1 工作区结构

OpenAgere 是一个包含 75+ 子crate 的 Cargo 工作区项目。工作区根位于 `Cargo.toml`，使用 resolver "2" 和 Rust edition 2024。

> **初学者提示**：Cargo 工作区（workspace）允许一个仓库包含多个相互依赖的 Rust 包（crate），共享同一个 `target/` 编译目录和 `Cargo.lock`。这比多个独立仓库更易于重构和同步版本。

### 1.2 核心crate层次

架构遵循一个清晰的分层设计，从底层的工具库向上构建到面向用户的 CLI。这体现了**依赖倒置原则**——底层不依赖上层，上层通过抽象与底层通信：

```
utils/*（底层工具库：纯函数，无副作用）
  └── protocol（核心协议定义，不含core依赖）
        │  定义所有"语言"——Event、Submission、AgentStatus
        │  为什么在core之外？因为app-server-client需要知道这些类型
        │  但不能依赖core（会造成循环依赖）
        │
        └── app-server-protocol（应用服务端协议扩展）
              └── config（配置层，不含core依赖）
                    │  为什么独立？配置在core初始化之前就需要加载
                    │
                    └── core（中心引擎：一切的核心）
                          ├── app-server（WS/stdio托管进程运行时）
                          │     └── app-server-client（客户端SDK）
                          │           ├── tui（终端UI层）
                          │           └── exec（单次执行模式）
                          └── cli（CLI入口，总调度器）
```

关键设计原则（初学者版）：
- **`protocol`** crate 是最底层的基础crate，不依赖 `core`，定义了所有核心类型（Event、EventMsg、AgentStatus 等）。这就好比字典定义了所有词汇，但字典不需要知道谁在使用这些词汇。
- **`config`** crate 也不依赖 `core`，独立处理配置层堆叠。配置必须在引擎启动前就绪。
- **`core`** 是中心引擎，几乎依赖所有子crate。它是"大脑"。
- **`tui`** 不直接依赖 `core`，而是通过 `app-server-client` 与系统通信。这个设计使得 TUI 可以连接本地或远程的 Agent——实现了**位置透明性**。

### 1.3 关键crate角色定位

| Crate | 角色 | 关键文件 |
|-------|------|---------|
| `agere-cli` | CLI入口，子命令调度 | `cli/src/main.rs` (1020行) |
| `agere-tui` | 终端UI，ratatui渲染 | `tui/src/lib.rs` (2338行) |
| `agere-core` | 核心引擎，会话管理 | `core/src/session/mod.rs` |
| `agere-app-server` | 应用服务器，JSON-RPC传输 | `app-server/src/lib.rs` |
| `agere-app-server-client` | 服务端客户端SDK | `app-server-client/src/lib.rs` |
| `agere-protocol` | 核心协议类型定义 | `protocol/src/protocol.rs` |
| `agere-config` | 配置系统 | `config/src/loader/mod.rs` |

### 1.4 端到端流程一句话概览

```
CLI解析 → 配置加载 → TUI初始化 → AppServer启动 → 用户输入 → Agere::spawn创建会话
→ submission_loop接收Submission → user_input_or_turn启动RegularTask
→ run_turn进入[LLM调用⇄工具执行]循环 → Session发送Event → app-server转为JSON-RPC通知
→ TUI接收通知 → ChatWidget渲染流式内容 → 帧调度器驱动重绘
```

---

## 2. CLI 入口与启动流程

### 2.1 启动伪代码

```
main():
    // 检查 argv[0] 身份（多二进制复用）
    match arg0_name:
        "agere-execve-wrapper"  → 单线程runtime, shell提权
        "apply_patch"           → 直接调用 patch 逻辑, exit
        "agere" (default)       → 加载 .env, 设置PATH别名, 继续

    // 构建多线程tokio运行时
    runtime = new tokio::Runtime(multi_thread, stack=16MB, enable_all)

    // 同步阻塞执行异步主函数
    runtime.block_on(cli_main())
```

### 2.2 main() 函数与Arg0调度

**文件：** `cli/src/main.rs`，第633行

```rust
fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(cli_main)
}
```

`main()` 实际不直接执行逻辑，而是交出控制权给 `arg0` crate。`arg0_dispatch_or_else` 位于 `arg0/src/lib.rs` 第162-180行，执行三个关键步骤：

**步骤1：Arg0检查**（`arg0_dispatch()`，第54行）
检查 `argv[0]` 名称以支持多种二进制身份：
- 若 arg0 == `agere-execve-wrapper`：创建单线程 tokio 运行时，运行 shell 提权包装器
- 若 arg0 == `apply_patch`：调用 `agere_apply_patch::main()` 并退出
- 若 argv[1] == `AGERE_FS_HELPER_ARG1`：运行文件系统助手
- 若 argv[1] == `AGERE_CORE_APPLY_PATCH_ARG1`：运行补丁应用
- 否则：加载 `~/.openagere/.env`（过滤 `AGERE_` 前缀的环境变量），设置 PATH 别名

**步骤2：Tokio运行时构建**（`build_runtime()`，第206行）
```rust
fn build_runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    builder.thread_stack_size(TOKIO_WORKER_STACK_SIZE_BYTES); // 16 MB
    Ok(builder.build()?)
}
```
使用多线程运行时，所有I/O特性全开，线程栈大小设为16MB（比Rust默认2MB大得多，以支持深度递归和大型future）。

**步骤3：阻塞运行**（第175行）
`runtime.block_on(run_main_with_arg0_guard(...))` 同步阻塞直到主异步函数 `cli_main()` 完成。

### 2.3 cli_main() 调度伪代码

```
cli_main():
    (config_overrides, feature_toggles, remote, interactive, subcommand) = MultitoolCli::parse()

    // 功能开关转为 -c features.X=true/false
    feature_toggles.to_overrides() → prepend to config_overrides

    match subcommand:
        None:
            // ★ 默认交互式TUI ★
            prepend_config_flags()
            merge_interactive_cli_flags()
            return run_interactive_tui(cli, config_overrides, remote, arg0_paths)

        Exec(exec):
            reject_remote_mode!("exec")           // --remote 仅交互模式可用
            return agere_exec::run_main(exec, override_config, loader_overrides, arg0_paths)

        Resume(cmd):
            finalize_resume_interactive(&mut interactive, cmd)  // 设置 resume_picker/session_id
            return run_interactive_tui(...)

        Fork(cmd):
            finalize_fork_interactive(&mut interactive, cmd)
            return run_interactive_tui(...)

        Mcp(mcp)   → mcp_cli.run()
        Plugin(p)  → MarketplaceCli::run()
        Logout     → run_logout()
        AppServer  → run_app_server() / dispatch sub-subcommand
        Completion → print_completion()
        Update     → run_update_command()
        Debug(d)   → dispatch_debug_subcommand(d)
        ...其他子命令...
```

### 2.4 CLI参数解析

**文件：** `cli/src/main.rs`，第63-145行

顶层结构体 `MultitoolCli` 使用 `clap::Parser` derive：

```rust
#[derive(Debug, Parser)]
struct MultitoolCli {
    config_overrides: CliConfigOverrides,   // -c key=value 重载
    feature_toggles: FeatureToggles,        // --enable/--disable 功能开关
    remote: InteractiveRemoteOptions,       // --remote ws://... 远程连接
    interactive: TuiCli,                    // 所有交互式TUI参数
    subcommand: Option<Subcommand>,         // 子命令枚举
}
```

子命令枚举包含19个变体：
```rust
enum Subcommand {
    Exec(ExecCli),            // agere exec：单次非交互式执行
    Review(ReviewArgs),       // agere review：代码审查
    Logout(LogoutCommand),    // agere logout：注销
    Mcp(McpCli),              // agere mcp：管理MCP服务器
    Plugin(PluginCli),        // agere plugin：插件管理
    AppServer(...),           // agere app-server：应用服务器
    App(...),                 // agere app：macOS/Windows应用
    Completion(...),          // shell补全
    Update,                   // 自更新
    Debug(...),              // 调试工具
    Execpolicy(...),         // 执行策略检查（隐藏）
    Resume(ResumeCommand),    // openagere resume：恢复会话
    Fork(ForkCommand),        // agere fork：分支会话
    StdioToUds(...),          // stdio到UDS桥接（隐藏）
    ExecServer(...),          // exec服务器
    Features(FeaturesCli),    // agere features
}
```

### 2.5 交互式TUI模式的启动

**文件：** `cli/src/main.rs`，第1291行

```rust
async fn run_interactive_tui(
    mut cli: TuiCli,
    config_overrides: CliConfigOverrides,
    remote: InteractiveRemoteOptions,
    arg0_paths: Arg0DispatchPaths,
) -> std::io::Result<AppExitInfo> {
    // 解析 -c 重载
    let config_overrides = config_overrides.parse_overrides().unwrap_or_else(|e| {
        eprintln!("Error parsing -c overrides: {e}");
        std::process::exit(1);
    });

    agere_tui::run_main(cli, arg0_paths, LoaderOverrides::default(), remote_opt, remote_auth_token).await
}
```

最终调用 `agere_tui::run_main()` 进入TUI主循环。

---

## 3. 配置加载与解析

### 3.1 ConfigBuilder管道

**文件：** `core/src/config/mod.rs`，第793行

`ConfigBuilder::build()` 执行一个多层配置堆叠过程：

```
基线默认值
  └── 用户配置层 (~/.openagere/config.toml)
        └── 项目配置层 (.claude/settings.json)
              └── CLI重载层 (-c key=value)
                    └── 云端需求层 (CloudRequirementsLoader)
                          └── ConfigOverrides (模型、审批策略、CWD等)
                                └── 序列化为 Config 结构体
```

### 3.2 配置加载伪代码

```
load_config_or_exit():
    config_overrides = parse_overrides("-c key=value" 数组)  // "model=opus"→(model, opus)
    agere_home = find_agere_home()                          // ~/.openagere/ 或 $AGERE_HOME

    // 预加载：检测项目和迁移状态
    config_toml = load_config_as_toml_with_cli_overrides(agere_home, cli_overrides)

    // 正式构建：六层堆叠
    config = ConfigBuilder::default()
        .cli_overrides(cli_overrides)            // 第3层: -c key=value
        .harness_overrides(ConfigOverrides {      // 第5层: 模型/审批/CWD等
            model, approval_policy, cwd,
            ephemeral, ...
        })
        .loader_overrides(LoaderOverrides)        // 控制哪些层跳过
        .cloud_requirements(cloud_reqs)           // 第4层: 云端需求
        .build()

    return config
```

### 3.3 关键配置类型

**ConfigOverrides**（`core/src/config/mod.rs`，第500行附近）
```rust
pub struct ConfigOverrides {
    pub model: Option<ModelProviderOverride>,
    pub approval_policy: Option<String>,
    pub access_mode: Option<String>,
    pub cwd: Option<PathBuf>,
    pub config_profile: Option<String>,
    pub agere_self_exe: Option<PathBuf>,
    pub ephemeral: Option<bool>,
    pub additional_writable_roots: Vec<PathBuf>,
    // ... 更多字段
}
```

**CliConfigOverrides**（`utils/cli/src/config_override.rs`，第18-37行）
捕获 `-c key=value` 原始字符串：
```rust
pub struct CliConfigOverrides {
    #[arg(short = 'c', value_name = "KEY=VALUE", action = clap::ArgAction::Append)]
    pub overrides: Vec<String>,
}
```
提供 `parse_overrides()` 方法（第42行）将原始字符串解析为 `Vec<(String, TomlValue)>`，以及 `apply_on_value()` 方法（第82行）将重载合并入 toml 值。

**LoaderOverrides**（`config/src/loader/mod.rs`）
```rust
pub struct LoaderOverrides {
    pub ignore_user_config: bool,
    pub ignore_user_and_project_exec_policy_rules: bool,
}
```
控制哪些配置层被加载——例如，在 exec 模式中可能忽略用户配置和规则。

---

## 4. TUI 初始化与显示框架

> **初学者理解**：TUI（Terminal User Interface）= 终端用户界面。不同于 GUI（图形界面），TUI 在纯文本终端中运行，通过字符绘制按钮、面板、输入框等。本项目使用 `ratatui` 框架（`tui-rs` 的社区维护分支）+ `crossterm` 后端（终端控制库）。

### 4.1 run_main() 伪代码

```
run_main(cli, arg0_paths, loader_overrides, remote, remote_auth_token):
    ── 阶段1: 配置 ──
    config_overrides = cli.config_overrides.parse_overrides()
        // 将 "-c model=opus -c approval=never" 解析为 [(model, opus), (approval, never)]
    agere_home = find_agere_home()
        // 查找 ~/.openagere/ 或 $AGERE_HOME 环境变量
    config_toml = load_config_as_toml_with_cli_overrides(agere_home, overrides)
        // 预加载配置：检测项目状态、运行迁移
    config = load_config_or_exit(...)       // 六层堆叠 → Config
        // 如果配置有严重错误，直接退出并打印错误

    ── 阶段2: 基础设施 ──
    init_otel()                              // OpenTelemetry：分布式追踪+日志
    env_manager = EnvironmentManager::new(config)
    app_server_session = AppServerSession::new(config, remote)
        // 嵌入模式: 同进程启动 AppServer
        //   优点: 零网络延迟, 不需要网络配置
        //   缺点: TUI崩溃 = Agent崩溃
        // 远程模式: WebSocket 连接到远端
        //   优点: TUI可以独立重启, 多客户端共享一个Agent
        //   缺点: 网络延迟, 需要额外配置

    ── 阶段3: TUI启动 ──
    return run_ratatui_app(config, app_server_session)
```

> **设计决策：为什么有两种运行模式？** 嵌入模式适合个人开发者——启动快、配置少、零网络开销。远程模式适合团队或服务器部署——多个开发者可以各自打开TUI窗口连接到同一个远程Agent，共享上下文。

### 4.2 TUI主循环伪代码

```
run_ratatui_app(config, app_server_session):
    terminal = CustomTerminal::new()         // 自定义ratatui, 支持inline viewport + diff渲染
    tui = Tui::new(terminal)
        // 创建: draw_tx (broadcast), event_broker, frame_requester (120fps调度器)
    app = App::new(tui, app_server_session, config)
        // 创建: app_event_tx (mpsc unbounded), chat_widget, keymap

    return app.run()   // ★ 阻塞直到用户退出

// ─── App::run() 主循环 ───
App::run():
    loop {
        event = select! {
            Some(event) = app_event_rx.recv()         // ① 内部AppEvent (widget间)
            Some(event) = active_thread_rx.recv()     // ② 当前线程事件 (app-server通知)
            event = tui_events.next()                 // ③ TUI事件 (键盘/resize/draw)
            app_server_event = app_server.next_event() // ④ 全局app-server通知
        }

        control = match event:
            AppEvent::Exit          → return Exit
            TuiEvent::Key(key)      → handle_key_event(key)
            TuiEvent::Draw          → render_frame()
            TuiEvent::Resize(w,h)   → reflow_and_render()
            TuiEvent::Paste(text)   → chat_widget.handle_paste(text)
            ThreadBufferedEvent(n)  → dispatch_to_chat_widget(n)
            ServerNotification(n)   → dispatch_app_server_event(n)

        if control == Exit: break
    }

    // 清理
    terminal.restore()          // 退出raw模式, 恢复alt screen
    return AppExitInfo
```

### 4.3 核心TUI结构体

**Tui结构体**（`tui/src/tui.rs`，第313行）
```rust
pub struct Tui {
    draw_tx: broadcast::Sender<()>,      // 帧绘制通知
    event_broker: Arc<EventBroker>,       // 共享crossterm事件源
    terminal: Terminal,                   // 自定义ratatui终端（含inline viewport）
    pending_history_lines: Vec<Line<'static>>, // 缓冲的滚动历史
    // ...
}
```

**App结构体**（`tui/src/app.rs`，第511行）
```rust
pub struct App {
    app_event_tx: AppEventSender,                          // 发送AppEvent
    chat_widget: ChatWidget,                                // 主聊天UI
    config: Config,                                         // 应用配置
    transcript_cells: Vec<Arc<dyn HistoryCell>>,            // 已提交的对话单元
    overlay: Option<Overlay>,                               // 分页器覆盖层
    keymap: RuntimeKeymap,                                  // 键位绑定
    active_thread_id: Option<ThreadId>,                     // 当前活动子代理线程
    thread_event_channels: HashMap<ThreadId, ThreadEventChannel>, // 每线程事件存储
    // ... 更多字段
}
```

### 4.4 FrameRequester帧调度器

**文件：** `tui/src/tui/frame_requester.rs`，第31-41行

一个微型的actor模式实现，用于合并和对齐帧请求：
```rust
pub struct FrameRequester {
    frame_schedule_tx: mpsc::UnboundedSender<Instant>,
}

pub fn new(draw_tx: broadcast::Sender<()>) -> Self {
    let (tx, rx) = mpsc::unbounded_channel();
    let scheduler = FrameScheduler::new(rx, draw_tx);
    tokio::spawn(scheduler.run());
    // ...
}
```

**帧调度器伪代码：**
```
FrameScheduler::run():
    loop {
        deadline = rx.recv()           // 接收重绘请求 (带Instant时间戳)
        // 合并: 在deadline之前收集所有额外请求
        while let Ok(later) = rx.try_recv():
            deadline = max(deadline, later)

        // 限速: 距上次draw不足8.33ms则等待
        rate_limiter.throttle()

        draw_tx.send(())              // ★ 广播绘制信号
    }
```

工作流程：
1. 任何widget调用 `frame_requester.schedule_frame()` 请求重绘
2. `FrameScheduler` actor通过 `mpsc::UnboundedReceiver<Instant>` 接收请求
3. 合并多个请求为一个draw通知
4. 通过 `broadcast::Sender<()>` 发送到TUI事件循环
5. 帧率限制在120 FPS（`MIN_FRAME_INTERVAL = 8.33ms`）

---

## 5. AppServer 启动与嵌入模式

### 5.1 两种运行模式

TUI可以通过两种方式连接到核心引擎：

**嵌入模式（Embedded）**：在同一进程中启动 `agere_app_server`
- TUI直接通过 `InProcessAppServerClient` 与app-server通信
- 使用 `mpsc` 通道替代WebSocket进行进程内通信

**远程模式（Remote）**：连接到远程app-server
- TUI通过 `RemoteAppServerClient` 使用WebSocket连接
- 适用于客户端-服务器部署

### 5.2 AppServer启动伪代码

```
AppServer::start(config, transport):
    // ── 创建三组核心通道 ──
    (transport_event_tx, transport_event_rx) = mpsc::channel::<TransportEvent>(128)
    (outgoing_tx, outgoing_rx) = mpsc::channel::<OutgoingEnvelope>(128)
    (outbound_control_tx, outbound_control_rx) = mpsc::channel::<OutboundControlEvent>(128)

    // ── 创建 MessageProcessor (核心消息引擎) ──
    message_processor = MessageProcessor::new(config, outgoing_tx, outbound_control_tx)

    // ── 启动传输监听 ──
    transport.listen(transport_event_tx)  // stdio/WS/UDS → TransportEvent

    // ── 启动出站路由器 ──
    tokio::spawn(outbound_router(outgoing_rx, outbound_control_rx))
        // 循环: 从outgoing_rx取OutgoingEnvelope → 按connection_id路由 → 写入transport

    // ── 主循环: 处理入站 ──
    loop {
        event = transport_event_rx.recv()
        match event:
            ConnectionOpened(id, writer)  → register(id, writer)
            ConnectionClosed(id)          → unregister(id)
            IncomingMessage(id, jsonrpc)  → message_processor.process(id, jsonrpc)
                // ★ 核心: JSON-RPC method → Op → Agere::tx_sub
    }
```

### 5.3 消息路由伪代码

```
MessageProcessor::process(connection_id, message):
    match message.method:
        "session/create"         → ThreadManager::spawn_thread()  → Agere::spawn()
        "session/input"          → agere.submit(Op::UserInput{...})
        "session/interrupt"      → agere.submit(Op::Interrupt)
        "session/shutdown"       → agere.submit(Op::Shutdown)
        "session/resume"         → ThreadManager::resume_thread()
        "session/fork"           → ThreadManager::fork_thread()
        "session/list_mcp_tools" → agere.submit(Op::ListMcpTools)
        ...

// ─── 出站路由 ───
OutgoingMessageSender::send_notification(event):
    envelope = OutgoingEnvelope::Broadcast(notification)
    outgoing_tx.send(envelope)         // → outbound_router → 所有已连接客户端

OutgoingMessageSender::send_request(connection_id, request):
    (response_tx, response_rx) = oneshot::channel()
    envelope = OutgoingEnvelope::ToConnection(connection_id, request, response_tx)
    callback_map.insert(request.id, response_tx)
    outgoing_tx.send(envelope)
    return response_rx                  // 等待客户端响应
```

### 5.4 AppServer核心通道

**文件：** `app-server/src/lib.rs`，第426-430行

```rust
let (transport_event_tx, mut transport_event_rx) =
    mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);        // 容量 128
let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<OutgoingEnvelope>(CHANNEL_CAPACITY);
let (outbound_control_tx, mut outbound_control_rx) =
    mpsc::channel::<OutboundControlEvent>(CHANNEL_CAPACITY);
```

这三个通道构成了从传输层到消息处理器的双向管线：

```
外部客户端 (WebSocket/stdio/UDS)
    │
    ▼ (入站)
TransportEvent (mpsc通道)
    │
    ▼
MessageProcessor 主循环
    │
    ▼ (出站)
OutgoingEnvelope (mpsc通道)
    │
    ▼
出站路由器任务
    │
    ▼ (返回外部客户端)
WebSocket/stdio/UDS
```

### 5.5 TransportEvent 定义

**文件：** `app-server/src/transport/mod.rs`，第158行

```rust
pub(crate) enum TransportEvent {
    ConnectionOpened {
        connection_id: ConnectionId,
        origin: String,
        writer: Arc<dyn MessageWriter>,
        disconnect_sender: watch::Sender<bool>,
    },
    ConnectionClosed {
        connection_id: ConnectionId,
    },
    IncomingMessage {
        connection_id: ConnectionId,
        message: JSONRPCMessage,
    },
}
```

### 5.6 OutgoingEnvelope 定义

**文件：** `app-server/src/outgoing_message.rs`，第86行

```rust
pub(crate) enum OutgoingEnvelope {
    ToConnection {
        connection_id: ConnectionId,
        message: OutgoingMessage,
        write_complete_tx: Option<oneshot::Sender<()>>,  // 异步写完成确认
    },
    Broadcast {
        message: OutgoingMessage,
    },
}
```

### 5.7 OutgoingMessageSender——请求/响应追踪

**文件：** `app-server/src/outgoing_message.rs`，第113行

```rust
pub(crate) struct OutgoingMessageSender {
    next_server_request_id: AtomicI64,
    sender: mpsc::Sender<OutgoingEnvelope>,
    request_id_to_callback: Mutex<HashMap<RequestId, PendingCallbackEntry>>,
    request_contexts: Mutex<HashMap<ConnectionRequestId, RequestContext>>,
}
```
对于每个发出的请求，它会用 `oneshot::Sender<ClientRequestResult>` 注册一个回调，以便在响应到达时进行匹配。

### 5.8 进程内通道

**InProcessServerEvent**（`app-server/src/in_process.rs`，第143行）
```rust
pub enum InProcessServerEvent {
    ServerRequest(ServerRequest),
    ServerNotification(ServerNotification),
    Lagged { skipped: usize },
}
```

**InProcessClientMessage**（`app-server/src/in_process.rs`，第157行）
```rust
enum InProcessClientMessage {
    Request {
        request: Box<ClientRequest>,
        response_tx: oneshot::Sender<Result<ServerRequestResult, ClientRequestError>>,
    },
    Notification { notification: ClientNotification },
    ServerRequestResponse { request_id: RequestId, result: ServerRequestResult },
    ServerRequestError { request_id: RequestId, error: ServerRequestError },
    Shutdown { done_tx: oneshot::Sender<()> },
}
```

**创建位置：**
- `InProcessServerEvent` 通道在 `app-server-client/src/lib.rs` 第526行由客户端创建：
  `mpsc::channel::<InProcessServerEvent>(channel_capacity)`
- `InProcessClientMessage` 通道在 `app-server/src/in_process.rs` 第185行创建

---

## 6. 通道系统总览与消息传递架构

> **初学者理解**：如果 OpenAgere 是一个人，那通道就是他的**神经系统**。不同组件（大脑Core、嘴巴TUI、耳朵AppServer）之间不直接调用对方的函数，而是通过发送消息来通信。这就像发微信而不是面对面说话——发送者和接收者互不知道对方在哪里、在做什么，只知道通过同一个群聊（通道）传递信息。这种设计的核心好处是**解耦**：组件可以独立开发、测试、甚至替换（把本地AppServer换成远程WebSocket），而不影响其他组件。

### 6.1 所有关键通道一览表

OpenAgere使用多个层次的通道系统。下表列出了最关键的通道：

| 位置 | 类型 | 容量 | 消息类型 | 发送者 | 接收者 |
|------|------|------|----------|--------|--------|
| `core/src/session/mod.rs:468` | async_channel 有界 | 512 | `Submission` | 外部（`Agere::tx_sub`） | `submission_loop` |
| `core/src/session/mod.rs:469` | async_channel 无界 | 无限 | `Event`/`EventMsg` | `Session`（`tx_event`） | 消费者（`Agere::rx_event`） |
| `core/src/agent/mailbox.rs:24` | mpsc 无界 | 无限 | `InterAgentCommunication` | `Mailbox` | `MailboxReceiver` |
| `core/src/agent/mailbox.rs:25` | watch | 1 | `u64`（序列号） | `Mailbox` | 订阅者 |
| `core/src/thread_manager.rs:276` | broadcast | 1024 | `ThreadId` | `ThreadManager` | 订阅者 |
| `core/src/skills_watcher.rs:40` | broadcast | 128 | `SkillsWatcherEvent` | `SkillsWatcher` | 订阅者 |
| `app-server/src/lib.rs:426` | mpsc | 128 | `TransportEvent` | 传输层 | `MessageProcessor` 循环 |
| `app-server/src/lib.rs:428` | mpsc | 128 | `OutgoingEnvelope` | `OutgoingMessageSender` | 出站路由器 |
| `app-server/src/lib.rs:430` | mpsc | 128 | `OutboundControlEvent` | 传输/消息处理器 | 出站路由器控制 |
| `app-server/src/in_process.rs:185` | mpsc | 可配置 | `InProcessClientMessage` | `InProcessClientSender` | in-process 运行时 |
| `app-server-client/src/lib.rs:526` | mpsc | 可配置 | `InProcessServerEvent` | in-process 运行时 | `InProcessAppServerClient` |
| `exec-server/src/process.rs:84` | broadcast | 可配置 | `ExecProcessEvent` | `ExecProcessEventLog` | 订阅者 |
| `core/src/unified_exec/process.rs:75` | broadcast | 可配置 | `Vec<u8>` | `UnifiedExecProcess` | 输出消费者 |
| `tui/src/tui.rs:336` | broadcast | 1 | `()`（绘制信号） | `FrameRequester` | TUI 事件循环 |
| `tui/src/tui/frame_requester.rs:40` | mpsc 无界 | 无限 | `Instant` | `FrameRequester` 句柄 | `FrameScheduler` |
| `tui/src/app_event_sender.rs:19` | mpsc 无界 | 无限 | `AppEvent` | `AppEventSender` | TUI 主循环 |
| `core/src/session/session.rs:11` | watch | 1 | `AgentStatus` | `Session` | 订阅者 |
| `core/src/session/session.rs:12` | watch | 1 | `bool`（暂停状态） | `Session` | 订阅者 |

### 6.2 核心双通道设计

整个系统的核心是 `Agere` 结构体维护的两个 `async_channel`：

```
       tx_sub (async_channel 有界)           rx_event (async_channel 无界)
              │                                        │
    外部 -> 发送 Submission               Session 发送 Event -> 外部消费
              │                                        │
              ▼                                        ▼
       submission_loop                        Agere::next_event()
```

**设计理念：**
- **`tx_sub`（有界）**：提供背压机制。当消费端跟不上时，提交者会被阻塞。容量512次提交。
- **`rx_event`（无界）**：确保事件不会丢失。消费者必须跟上，但发送者永远不会被阻塞。

### 6.3 全链路消息流（TUI嵌入模式）

```
┌─ 用户键盘 ──────────────────────────────────────────────────────┐
│  Crossterm EventStream                                          │
│      │                                                          │
│      ▼                                                          │
│  TuiEventStream ──轮询──► TuiEvent::Key ──► handle_key_event()  │
│      │                                                          │
│      ▼                                                          │
│  ChatWidget/BottomPane ──► AppEvent ──► Op::UserInput           │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼  app_event_tx (mpsc unbounded)
┌─ App 主循环 ────────────────────────────────────────────────────┐
│  select! 四路复用:                                              │
│    ① app_event_rx.recv()     ← 内部AppEvent                    │
│    ② active_thread_rx.recv() ← 线程事件                        │
│    ③ tui_events.next()       ← TUI事件                         │
│    ④ app_server.next_event() ← 全局通知 ★                      │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼  InProcessClientMessage (mpsc)
┌─ AppServer (同进程) ────────────────────────────────────────────┐
│  TransportEvent::IncomingMessage                                │
│      │                                                          │
│      ▼                                                          │
│  MessageProcessor::process(connection, jsonrpc)                 │
│      │  解析 JSON-RPC method → Op                              │
│      ▼                                                          │
│  agere.submit(Op::UserInput{...})                               │
│      │                                                          │
│      ▼  tx_sub (async_channel bounded 512)                     │
│  submission_loop                                                │
│      │                                                          │
│      ▼                                                          │
│  user_input_or_turn() → RegularTask → run_turn()                │
│      │                                                          │
│      ▼                                                          │
│  ┌─ [LLM调用循环] ──────────────────────────────┐               │
│  │  stream() → SSE解析 → 流事件                 │               │
│  │      │                                       │               │
│  │      ├── 文本增量 → AgentMessageDelta 事件    │               │
│  │      └── 工具调用 → drain_in_flight()         │               │
│  │           └── 结果记录回对话历史               │               │
│  │                                               │               │
│  │  needs_follow_up?                             │               │
│  │      YES → 继续循环 (工具结果发给LLM)         │               │
│  │      NO  → stop hooks → TurnComplete          │               │
│  └───────────────────────────────────────────────┘               │
│      │                                                          │
│      ▼  tx_event (async_channel unbounded)                      │
│  Session::send_event(Event)                                     │
│      │                                                          │
│      ▼                                                          │
│  apply_bespoke_event_handling()                                 │
│      │  EventMsg → JSON-RPC Notification                       │
│      ▼                                                          │
│  OutgoingMessageSender::send_notification()                     │
│      │                                                          │
│      ▼  OutgoingEnvelope (mpsc)                                 │
│  outbound_router → InProcessServerEvent::ServerNotification     │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼  InProcessServerEvent (mpsc)
┌─ TUI 显示层 ────────────────────────────────────────────────────┐
│  App::handle_app_server_event(notification)                     │
│      │                                                          │
│      ▼                                                          │
│  ChatWidget::handle_streaming_delta(text)     ← 文本增量         │
│  ChatWidget::handle_item_started(item)        ← 工具开始         │
│  ChatWidget::handle_item_completed(item)      ← 工具完成         │
│      │                                                          │
│      ▼                                                          │
│  StreamController::push(delta) → 解析markdown → 排队行           │
│      │                                                          │
│      ▼                                                          │
│  FrameRequester::schedule_frame() → draw_tx.send(())             │
│      │                                                          │
│      ▼                                                          │
│  render_frame() → chat_widget.render(area, buffer)               │
│      │  diff_buffers() → 仅写变化部分到终端                      │
└──────────────────────────────────────────────────────────────────┘
```

---

## 7. Agere 核心结构与会话创建

### 7.1 Agere 结构体

**文件：** `core/src/session/mod.rs`，第365-374行

```rust
pub struct Agere {
    pub(crate) tx_sub: Sender<Submission>,          // 提交通道（输入）
    pub(crate) rx_event: Receiver<Event>,           // 事件通道（输出）
    pub(crate) agent_status: watch::Receiver<AgentStatus>,  // 状态观察
    pub(crate) session: Arc<Session>,               // 共享会话状态
    pub(crate) session_loop_termination: SessionLoopTermination,  // 等待循环退出
}
```

这是一个精简的公开接口，包含五个字段。外部代码通过 `tx_sub` 发送操作，通过 `rx_event` 接收事件。

### 7.2 AgereSpawnArgs 参数结构

**文件：** `core/src/session/mod.rs`，第385-411行

```rust
pub(crate) struct AgereSpawnArgs {
    pub(crate) config: Config,
    pub(crate) models_manager: SharedModelsManager,
    pub(crate) environment_manager: Arc<EnvironmentManager>,
    pub(crate) skills_manager: Arc<SkillsManager>,
    pub(crate) plugins_manager: Arc<PluginsManager>,
    pub(crate) mcp_manager: Arc<McpManager>,
    pub(crate) skills_watcher: Arc<SkillsWatcher>,
    pub(crate) conversation_history: InitialHistory,
    pub(crate) session_source: SessionSource,         // Session 或 SubAgent
    pub(crate) agent_control: AgentControl,
    pub(crate) dynamic_tools: Vec<DynamicToolSpec>,
    pub(crate) persist_extended_history: bool,
    pub(crate) metrics_service_name: Option<String>,
    pub(crate) inherited_shell_snapshot: Option<Arc<ShellSnapshot>>,
    pub(crate) inherited_exec_policy: Option<Arc<ExecPolicyManager>>,
    pub(crate) parent_rollout_thread_trace: ThreadTraceContext,
    pub(crate) user_shell_override: Option<shell::Shell>,
    pub(crate) parent_trace: Option<W3cTraceContext>,
    pub(crate) environments: Vec<TurnEnvironmentSelection>,
    pub(crate) analytics_events_client: Option<AnalyticsEventsClient>,
    pub(crate) thread_store: Arc<dyn ThreadStore>,
}
```

### 7.3 Agere::spawn 完整伪代码

> **初学者理解**：`spawn` 一词在 Rust 中有双重含义。`tokio::spawn` 是启动一个异步后台任务；`Agere::spawn` 是**创建一个新的 Agent 会话实例**——包括它的通道、状态、后台循环。类比：`Agere::spawn` = 雇佣一个新员工（给他分配工位、电话、任务清单），`tokio::spawn` = 告诉这个员工"开始干活吧"。

```
Agere::spawn(args: AgereSpawnArgs) → AgereSpawnOk:
    ── 解构 args ──
    (config, models_manager, env_manager, skills_manager,
     plugins_manager, mcp_manager, skills_watcher,
     conversation_history, session_source, agent_control, ...) = args

    ── ★ 创建核心双通道 ★ ──
    (tx_sub, rx_sub) = async_channel::bounded(512)      // 提交通道: 有界, 512容量
    (tx_event, rx_event) = async_channel::unbounded()   // 事件通道: 无界

    // 为什么提交是有界的？防止恶意或bug导致内存无限增长。
    // 如果消费端处理不过来，背压会让发送者慢下来。
    // 为什么事件是无界的？事件必须不丢失——漏掉 TurnComplete 可能导致
    // TUI 永远显示 "Agent is thinking..."。

    ── 环境选择与验证 ──
    primary_env = validate_and_select_environment(environments)

    ── 加载技能与插件 ──
    skills = query_skills_for_config(skills_manager, plugins_manager)

    ── 深度限制检查 ──
    if agent_depth > MAX_DEPTH:
        disable(SpawnCsv | Collab)

    ── 解析指令与策略 ──
    user_directives = resolve_user_directives(config)
    exec_policy = inherited_exec_policy.or(build_exec_policy(config))

    ── 查询模型信息 ──
    model_info = models_manager.resolve(config.model)

    ── 动态工具解析 ──
    dynamic_tools = load_from_db().or(load_from_rollout_file())

    ── 构建 SessionConfiguration ──
    session_config = merge(
        config,
        model_info,
        skills,
        dynamic_tools,
        exec_policy,
        user_directives,
        ...
    )

    ── ★ 创建 Session ★ ──
    (agent_status_tx, agent_status_rx) = watch::channel(AgentStatus::PendingInit)

    session = Session::new(
        session_config,
        config,
        models_manager,
        exec_policy,
        tx_event,              // ← 事件发送端 传入 Session
        agent_status_tx,       // ← 状态发送端 传入 Session
        conversation_history,
        session_source,
        skills_manager, plugins_manager, mcp_manager,
        skills_watcher, agent_control, env_manager,
        analytics_events_client, thread_store, ...
    )  // 内部: 4路并发 join! + 同步构建

    ── ★ 启动提交循环 (后台任务) ★ ──
    session_loop_handle = tokio::spawn(async {
        submission_loop(Arc::clone(&session), config, rx_sub)
    })

    ── 组装 Agere 返回 ──
    return AgereSpawnOk {
        agere: Agere {
            tx_sub,              // 外部通过此发送 Submission
            rx_event,            // 外部通过此接收 Event
            agent_status: agent_status_rx,
            session,
            session_loop_termination: from_handle(session_loop_handle),
        },
        thread_id,
    }
```

### 7.4 Session 结构体

**文件：** `core/src/session/session.rs`，第8-30行

```rust
pub(crate) struct Session {
    pub(crate) conversation_id: ThreadId,
    pub(super) tx_event: Sender<Event>,                      // 持有事件发送端
    pub(super) agent_status: watch::Sender<AgentStatus>,      // 持有状态发送端
    pub(super) out_of_band_elicitation_paused: watch::Sender<bool>,
    pub(super) state: Mutex<SessionState>,
    pub(super) managed_network_proxy_refresh_lock: Semaphore,
    pub(super) features: ManagedFeatures,
    pub(super) pending_mcp_server_refresh_config: Mutex<Option<McpServerRefreshConfig>>,
    pub(crate) conversation: Arc<RealtimeConversationManager>,
    pub(crate) active_turn: Mutex<Option<ActiveTurn>>,
    pub(super) mailbox: Mailbox,                              // 代理间邮箱
    pub(super) mailbox_rx: Mutex<MailboxReceiver>,
    pub(super) idle_pending_input: Mutex<Vec<ResponseInputItem>>,
    pub(crate) goal_runtime: GoalRuntimeState,
    pub(crate) guardian_review_session: GuardianReviewSessionManager,
    pub(crate) services: SessionServices,                     // 所有外部服务
    pub(super) next_internal_sub_id: AtomicU64,
}
```

### 7.5 Session::new() 核心初始化伪代码

```
Session::new(config, ..., tx_event, agent_status_tx, ...) → Arc<Session>:
    ── 4路并发初始化 (tokio::join!) ──
    (thread_persistence, state_db, history_meta, mcp_and_auth)
        = join!(
            create_or_resume_live_thread(),       // 持久化历史
            get_state_db_handle(),                // 本地状态DB
            load_history_metadata(),              // 消息历史元数据
            fetch_mcp_servers_and_oauth(),        // MCP服务器+OAuth状态
        )

    ── 同步构建 ──
    trace = build_trace_metadata()
    startup_events = collect_deprecation_warnings()
    setup_telemetry_and_metrics()
    shell_snapshot = configure_default_shell()     // 含 watch channel
    state = SessionState::new()
    hooks = configure_hooks(config)
    services = SessionServices {
        mcp_manager, exec_manager, model_client,
        hooks, telemetry, network_policy, ...
    }
    session = Session { conversation_id, tx_event, agent_status_tx, state, services, ... }

    ── 初始事件 ──
    session.deliver_event_raw(Event::SessionConfigured(...))
    for warn in startup_events:
        session.deliver_event_raw(Event::Warning(warn))

    ── 启动后台任务 ──
    spawn_skills_watcher_listener()
    reinit_mcp_connection_manager()
    schedule_warmup()

    return Arc::new(session)
```

### 7.6 ThreadManager——调用入口

**文件：** `core/src/thread_manager.rs`，第1015-1086行

`ThreadManager::spawn_thread_with_source()` 是调用 `Agere::spawn` 的生产入口点：

```rust
pub(crate) async fn spawn_thread_with_source(
    &self,
    source: ThreadSource,
    // ... 其他参数
) -> AgereResult<Arc<AgereThread>> {
    let spawn_args = AgereSpawnArgs {
        config,
        models_manager,
        environment_manager,
        skills_manager,
        plugins_manager,
        mcp_manager,
        skills_watcher,
        // ... 所有参数
    };
    let AgereSpawnOk { agere, thread_id } = Agere::spawn(spawn_args).await?;
    // 等待初始 SessionConfiguredEvent
    let agere_thread = finalize_thread_spawn(agere, thread_id, ...).await?;
    Ok(agere_thread)
}
```

`finalize_thread_spawn`（第1088行）等待第一个 `SessionConfigured` 事件，将生成的 `Agere` 包装在 `Arc<AgereThread>` 中，并将其注册到活动线程映射表中。


> **扩展阅读**：
> - `SessionServices` 是所有共享服务的容器，子代理通过 `Arc::clone` 继承（详见 Ch25.5）
> - `AgentControl` 是多代理控制平面，管理 Agent 注册、深度限制、Fork（详见 Ch26）
> - `SessionConfiguration` 的模型解析优先级：rollout 持久化 > config.toml > 默认（详见 Ch19.7）

---

## 8. Submission 提交循环

> **初学者理解**：`submission_loop` 是 Agent 的"心脏"。它是一个无限循环，不断从通道接收操作指令，分发处理，然后等待下一个。这就像一个餐厅厨房：服务员不断递来订单（Submission），厨师按顺序处理（分派到 handler），处理完一个再接下一个。

### 8.1 submission_loop 伪代码

```
submission_loop(session, config, rx_sub):
    // rx_sub 是 async_channel::Receiver<Submission>，来自 Agere::tx_sub
    // 容量 512，有背压保护
    // session 是 Arc<Session>，引用计数共享

    loop {
        sub = rx_sub.recv().await?          // ★ 从有界通道接收提交
            // 若通道关闭（所有发送端被丢弃）, recv() 返回 Err，退出循环
            // "?" 操作符让错误自动向上传播

        should_exit = match sub.op:
            // ── 生命周期操作 ──
            Op::Interrupt:
                interrupt(session)
                // 取消当前Turn的 CancellationToken
                // Turn 中的流式操作检测到取消，优雅中止
                false

            Op::Shutdown:
                shutdown(session)            // ★ 唯一返回true的操作
                // ① 中断当前Turn
                // ② 发送 ShutdownComplete 事件
                true

            // ── 用户输入 (最常见的操作) ──
            Op::UserInput { items, updates }
            | Op::UserInputWithTurnContext { items, turn_context, updates }
            | Op::UserTurn { items, updates }:
                user_input_or_turn(session, sub.id, sub.op)
                false

            // ── 审批响应 ──
            Op::ExecApproval { call_id, decision }:
                handle_exec_approval_response(session, sub)
                false
            Op::PatchApproval { call_id, decision }:
                handle_patch_approval_response(session, sub)
                false
            Op::UserInputAnswer { call_id, answer }:
                handle_user_input_answer(session, sub)
                false
            Op::RequestPermissionsResponse { call_id, granted }:
                handle_request_permissions_response(session, sub)
                false

            // ── 代理间通信 ──
            Op::InterAgentCommunication { message }:
                handle_inter_agent_communication(session, sub.op)
                false

            // ── 会话管理 ──
            Op::Compact:
                handle_compact(session, sub.id)
                false
            Op::Undo:
                handle_undo(session, sub.id)
                false
            Op::ThreadRollback { target_id }:
                handle_thread_rollback(session, sub)
                false
            Op::SetThreadName { name }:
                handle_set_thread_name(session, sub)
                false

            // ── 实时对话 ──
            Op::RealtimeConversationStart(params):
                handle_realtime_conversation_start(session, params)
                false
            Op::RealtimeConversationAudio(params):
                handle_realtime_conversation_audio(session, params)
                false
            // ... 更多实时操作

            // ── MCP/技能/工具管理 ──
            Op::ListMcpTools:
                handle_list_mcp_tools(session, sub.id)
                false
            Op::RefreshMcpServers { config }:
                handle_refresh_mcp_servers(session, sub)
                false
            Op::ListSkills { query }:
                handle_list_skills(session, sub)
                false

            // ── 其他 ──
            Op::ReloadUserConfig → handle_reload_user_config(session); false
            Op::RunUserShellCommand{cmd} → handle_run_user_shell_command(session, sub); false
            Op::Review{...} → handle_review(session, sub); false
            Op::AddToHistory{items} → handle_add_to_history(session, sub.op); false
            _ → false

        if should_exit: break
    }

    // ── ★ 清理 (无论如何退出都执行) ★ ──
    cleanup_session(session)
        // 终止所有 UnifiedExecManager 进程
        // 关闭所有 MCP 连接
        // 排空 guardian 审查状态
```

### 8.2 关键设计要点

1. **几乎所有操作返回 `false`**——唯一返回 `true` 的是 `Op::Shutdown`，表示循环应该退出。
2. **通道关闭退出**——如果 `rx_sub` 的对端被丢弃，`recv()` 返回 `Err`，循环也会退出。同样执行清理。
3. **单线程顺序处理**——所有操作在一个循环中顺序处理。没有操作会并发执行，确保了会话状态的线程安全。
4. **清理总是执行**——无论循环如何退出（正常或异常），`cleanup_session()` 都会终止统一执行管理器进程、关闭MCP连接并停用guardian审查会话。

### 8.3 Submission 结构体

```rust
pub struct Submission {
    pub id: String,          // UUID v7 提交ID
    pub op: Op,              // 操作payload
    pub trace: Option<W3cTraceContext>,  // 分布式追踪上下文
}
```

### 8.4 Agere::submit 方法

**文件：** `core/src/session/mod.rs`，第665-696行

```rust
pub async fn submit(&self, op: Op) -> Result<(), async_channel::SendError<Submission>> {
    let submission = Submission {
        id: uuid_v7(),
        op,
        trace: None,
    };
    self.tx_sub.send(submission).await
}
```

`submit_with_trace` 变体会自动附加当前span的W3C追踪上下文。


> **扩展阅读**：Submission 循环还处理以下高级 Op：
> - `Op::Compact` — 手动触发上下文压缩（详见 Ch27）
> - `Op::Undo` / `Op::ThreadRollback` — 回滚对话历史
> - `Op::ListMcpTools` / `Op::RefreshMcpServers` — MCP 服务器管理（详见 Ch22）
> - `Op::RunUserShellCommand` — 执行用户 shell 命令
> - `Op::InterAgentCommunication` — 代理间消息传递（详见 Ch26.4）

---

## 9. 用户输入处理与 Turn 状态机

### 9.1 user_input_or_turn 伪代码

```
user_input_or_turn(session, sub_id, op):
    // 解构 Op
    (items, updates, metadata) = match op:
        Op::UserInput { items, updates, .. }
        | Op::UserInputWithTurnContext { items, updates, .. }
        | Op::UserTurn { items, updates, .. } → (items, updates, metadata)

    // 创建 TurnContext: 应用设置更新, 解析模型, 创建追踪
    turn_context = session.new_turn_with_sub_id(sub_id, updates)

    // ★ 尝试引导输入: 如果已有正在运行的turn, 注入输入
    result = session.steer_input(items, turn_context)
    match result:
        Ok(turn_id):
            // 输入已注入到活动turn中 (多模态交互)
            return
        Err(SteerInputError::NoActiveTurn(items)):
            // 会话空闲 → 启动新的 RegularTask
            session.spawn_task(turn_context, items, RegularTask::new())
        Err(other):
            session.send_event(ErrorEvent(other))

    // 如果实时对话激活, 镜像用户文本到实时通道
    mirror_user_text_to_realtime(items)
```

### 9.2 RegularTask 与 Turn 启动伪代码

```
session.spawn_task(context, items, task):
    session.abort_all_existing_tasks()        // 中止所有正在运行的task

    cancel_token = CancellationToken::new()
    session.active_turn.replace(ActiveTurn { cancel_token, ... })

    tokio::spawn(async {
        task.run(context, cancel_token)
    })

// ─── RegularTask::run ───
RegularTask::run(context, cancel_token):
    session.send_event(TurnStarted)

    // 尝试消耗预热的 WebSocket 会话 (v2快速启动)
    prewarmed = session.take_prewarmed_session()

    // ★ 外层循环: 处理 pending 输入 ★
    loop {
        result = run_turn(session, context, current_input, prewarmed)

        // 检查是否有新的 pending 输入 (在turn期间通过 steer_input 注入)
        pending = session.drain_pending_input()
        if pending.is_empty(): break
        current_input = pending
    }

    session.send_event(TurnComplete { last_agent_message: result })
```

### 9.3 run_turn——核心状态机伪代码

> **初学者理解**：`run_turn` 是整个 Agent 最核心的逻辑。它实现了一个 **ReAct 循环**（Reason + Act）——模型推理 → 执行工具 → 观察结果 → 继续推理，直到模型认为任务完成。这是现代 Agent 系统的标准模式。

```
run_turn(session, context, input, prewarmed_session) → Option<String>:
    ── 设置阶段 ──
    maybe_compact_before_sampling()
        // 如果对话历史超过模型上下文窗口，自动压缩（保留重要信息，丢弃细节）
    record_context_updates()
        // 将 connectors（目录结构）、skills（可用技能）、plugins 注入到系统指令中
    record_user_prompt(input)
        // 用户输入被格式化为 ResponseInputItem::Message 添加到对话历史
    run_hook("user_prompt_submit")
        // 用户自定义钩子：可以在此修改输入、阻止输入、记录日志

    // 创建Turn范围的资源
    client_session = ModelClientSession::new(prewarmed_session)
        // 复用或新建 HTTP/WS 连接。Turn 内复用同一连接以提高效率
    tool_router = ToolRouter::build(session)
        // 枚举所有可用工具：MCP工具、Shell命令、自定义工具
    tool_runtime = ToolCallRuntime::new()
        // 管理工具执行的并行/串行调度

    ── ★ 内层循环: LLM调用 ⇄ 工具执行 (ReAct 循环) ★ ──
    // 这个循环可能执行 1~N 次，取决于模型是否需要调用工具
    loop:
        // ① 处理 pending 输入 (在turn中被 steer_input 注入的)
        //    steer_input 允许在当前Turn中注入新用户输入（多步交互）
        for item in session.drain_pending_input():
            if run_hook("inspect_pending_input", item) == Accept:
                record_conversation_item(item)
                // 被 Hook 拒绝的输入会被静默丢弃

        // ② 构建采样请求 (从对话历史组装)
        prompt = build_prompt_input(conversation_history, system_instructions, tools)
            // conversation_history: Vec<ResponseInputItem>，包含所有之前的消息和工具结果
            // system_instructions: String，包含系统提示词、技能说明、规则等
            // tools: Vec<ToolSpec>，包含所有可用工具的定义（名称、描述、参数schema）

        // ③ ★ 调用LLM ★
        result = run_sampling_request(session, context, client_session,
                                       prompt, tool_router, tool_runtime, cancel_token)
        // result = {
        //   needs_follow_up: bool,      // true = 有工具结果需要发给LLM继续
        //   last_agent_message: Option<String>,  // 最新一条assistant消息
        // }

        // ④ 判断是否需要继续
        if !result.needs_follow_up:
            // ★ 模型说 "end_turn" → 准备结束
            stop_decision = run_hook("stop")
                // Hook 返回: Allow（允许停止）/ Block（强制继续）
                //             / Inject(text)（注入额外指令后继续）
            if stop_decision == Block:
                needs_follow_up = true; continue
            if stop_decision == Inject(injection):
                record_injection(injection); continue

            run_hook("after_agent")
                // 用户自定义后处理：发送通知、格式化输出、评估质量等
            break                                  // ★ 退出循环

        if token_limit_reached:
            auto_compact()
                // 触发自动压缩：将历史总结为摘要，截断旧消息
                // 然后继续循环——压缩后的历史变短，下次调用可以继续
```

> **设计决策：为什么Turn内部是循环而不是单次调用？** 因为 Agent 不知道需要调用几次工具才能完成任务。模型可能第一次调用 `search_file`，看完结果后调用 `read_file`，看完文件内容后调用 `grep`... 每次工具调用的结果都追加到对话历史，然后让模型决定下一步。这是一个"逐步收集信息、逐步接近答案"的过程。

### 9.4 run_sampling_request 伪代码

```
run_sampling_request(session, ctx, client_session, prompt, tool_router, tool_runtime, cancel):
    ── ① 启动LLM流 ──
    stream = client_session.stream(prompt, model_info, ...)

    ── ② 处理流事件 ──
    last_agent_message = None
    needs_follow_up = false

    while let Some(event) = stream.next():
        match event:
            OutputItemAdded(item):
                active_item = item
                if is_text(item):
                    emit(AgentMessageDelta)
                if is_tool_call(item):
                    tool_arg_consumer = ToolArgumentDiffConsumer::new()

            OutputTextDelta(delta):
                emit(AgentMessageContentDelta(delta))       // ★ 逐个token发送
                parse_plan_tags(delta)                      // 检测 <PLAN> 标签

            ToolCallInputDelta(delta):
                tool_arg_consumer.feed(delta)               // 累积工具参数JSON

            OutputItemDone(item):
                result = handle_output_item_done(item)
                    // 如果是工具调用 → 排队执行 → needs_follow_up = true
                    // 如果是文本消息 → 记录 last_agent_message

            Completed { end_turn }:
                needs_follow_up = end_turn == Some(false)   // "tool_use" → 需要继续

    ── ③ ★ 等待所有工具执行完成 ★ ──
    tool_results = drain_in_flight(session, ctx, tool_runtime, cancel)
        // 每个结果 → record_conversation_items()
        // 下次循环迭代时, 这些结果作为 prompt 的一部分发给LLM

    return SamplingRequestResult { needs_follow_up, last_agent_message }
```

---

## 10. LLM 调用与流式响应处理

> **初学者理解**：当你向 ChatGPT 或 Claude 发送消息时，回复是一个字一个字"流"出来的——这就是 SSE (Server-Sent Events)。服务端不断推送小块数据（事件），客户端逐块接收。OpenAgere 正是这样工作的：LLM API 以 SSE 流返回内容，`process_anthropic_sse` 逐条解析事件，`run_sampling_request` 逐条处理——文本增量直接转为 `AgentMessageDelta` 事件发给TUI，工具调用则被提取出来排队执行。

### 10.1 ModelClient::stream 伪代码

```
ModelClientSession::stream(prompt, model_info) → ResponseStream:
    wire_api = model_info.provider.wire_api

    match wire_api:
        WireApi::Responses:
            if websocket_enabled:
                result = stream_via_websocket(prompt)
                if result != FallbackToHttp:
                    return result
                // 永久降级: 禁用WebSocket
                self.disable_websocket()

            return stream_via_http_responses_api(prompt)
                // POST /responses → SSE流 → ResponseStream

        WireApi::Anthropic:
            return stream_anthropic(prompt, model_info)
                // ① 转换: ResponseItems → Anthropic Messages格式
                // ② 转换: 工具规范 → Anthropic Tool定义
                // ③ POST /v1/messages (含 system, messages, tools, thinking)
                // ④ 启动后台任务: process_anthropic_sse()
                // ⑤ 返回 ResponseStream
```

### 10.2 Anthropic SSE流处理伪代码

```
process_anthropic_sse(raw_bytes_stream) → ResponseStream:
    // 在独立的 tokio 任务中运行
    loop {
        line = read_next_sse_line(raw_bytes_stream)

        event = match parse_sse_event(line):
            "message_start"     → record(response_id, server_model)
            "content_block_start" → match block_type:
                "text"       → emit(OutputItemAdded(Message{role:"assistant", content:[text]}))
                "tool_use"   → emit(OutputItemAdded(FunctionCall{name, call_id:id}))
                "thinking"   → emit(OutputItemAdded(Reasoning{...}))
            "content_block_delta" → match delta_type:
                "text_delta"       → emit(OutputTextDelta(delta))        // ★ token增量
                "input_json_delta"  → emit(ToolCallInputDelta(json))     // ★ 工具参数流
                "thinking_delta"    → emit(ReasoningContentDelta(delta))
            "content_block_stop" → match block_type:
                "text"       → emit(OutputItemDone(Message{accumulated_text}))
                "tool_use"   → emit(OutputItemDone(FunctionCall{arguments: full_json}))
            "message_delta" → match stop_reason:
                "tool_use"   → emit(Completed{end_turn: Some(false)})    // ★ 模型要调用工具
                "end_turn"   → emit(Completed{end_turn: Some(true)})     // ★ 模型结束
                "stop_sequence" → emit(Completed{end_turn: Some(true)})
            "error" → emit(Err(mapped_error_code))
    }
```

### 10.3 SSE事件映射表

| SSE事件 | 转换结果 |
|---------|---------|
| `MessageStart` | 记录response_id和server_model |
| `ContentBlockStart { Text { text } }` | `OutputItemAdded(Message { role: "assistant", content: [text] })` |
| `ContentBlockStart { ToolUse { id, name } }` | `OutputItemAdded(FunctionCall { name, call_id: id })` |
| `ContentBlockStart { Thinking }` | `OutputItemAdded(Reasoning { ... })` |
| `ContentBlockDelta { TextDelta }` | `OutputTextDelta` |
| `ContentBlockDelta { InputJsonDelta }` | `ToolCallInputDelta`（流式工具参数JSON） |
| `ContentBlockDelta { ThinkingDelta }` | `ReasoningContentDelta` |
| `ContentBlockStop { Text }` | `OutputItemDone(Message { ... })`（含累积文本） |
| `ContentBlockStop { ToolUse }` | `OutputItemDone(FunctionCall { arguments: String })` |
| `MessageDelta { stop_reason: "tool_use" }` | `Completed { end_turn: Some(false) }`（模型想继续） |
| `MessageDelta { stop_reason: "end_turn" }` | `Completed { end_turn: Some(true) }` |
| `Error` | `Err`（含映射的错误码） |

### 10.4 Responses API的构建

**stream_responses_api**（`core/src/client.rs`，第1105行）：
```rust
async fn stream_responses_api(
    &mut self,
    // ...
) -> AgereResult<ResponseStream<'_>> {
    let request = build_responses_api_request(
        model, instructions, input, tools, reasoning, text_format,
    );
    // POST到 /responses 端点
    // 映射原始API流到 ResponseStream<ResponseEvent>
}
```

**stream_responses_websocket**（第1202行）：
- 通过 `build_responses_request()` 构建请求
- 通过 `websocket_connection()` 管理WS连接（惰性打开，按需重连）
- 支持增量请求（仅发送自上次响应以来新的输入项）
- 如果服务器返回 `UPGRADE_REQUIRED`，回退到HTTP

---

## 11. 工具调用与执行流程

> **初学者理解**：AI Agent 不同于普通聊天机器人的关键就在于**工具调用**。LLM 本身只能生成文本，但 Agent 可以让 LLM 决定"调用工具"——比如读取文件、搜索代码、执行 shell 命令——然后 Agent 框架实际执行这些操作，把结果告诉 LLM，让 LLM 基于真实数据继续推理。这就是 Agent 能力的核心来源。

### 11.1 工具执行伪代码

```
// ─── ① 提取工具调用 ───
handle_output_item_done(item):
    call = ToolRouter::build_tool_call(item)
        // 根据 item 类型解析:
        //   FunctionCall { name, arguments, call_id } → ToolPayload::Mcp/Function
        //   ToolSearchCall                            → ToolPayload::ToolSearch
        //   CustomToolCall { name, input }            → ToolPayload::Custom
        //   LocalShellCall { action }                 → ToolPayload::Shell

    if call is Some:
        record_conversation_item(item)                      // 立即记录到历史
        future = tool_runtime.handle_tool_call(call, cancel) // 创建执行future
        in_flight_futures.push(future)                      // ★ 推入 FuturesOrdered
        needs_follow_up = true
    else:
        emit(ItemCompleted)
        record_conversation_item(item)
        last_agent_message = extract_text(item)

// ─── ② 并行/串行执行调度 ───
ToolCallRuntime::handle_tool_call(call, cancel):
    if tool_supports_parallel(call):
        // ★ 并行锁 (读操作: search, read_file, list_files)
        acquire_read_lock()
    else:
        // ★ 串行锁 (写操作: edit_file, exec_command)
        acquire_write_lock()

    result = tool_registry.dispatch_any(ToolInvocation {
        payload: call.payload,
        call_id: call.id,
        cancellation: cancel,
    })
    return ToolCallResult { output, error, ... }

// ─── ③ 等待所有工具完成 ───
drain_in_flight(session, ctx, tool_runtime, cancel) → Vec<ResponseInputItem>:
    results = []
    while let Some(result) = tool_runtime.next_result(cancel):
        match result:
            Ok(output):
                // ★ 将工具输出记录回对话历史
                session.record_conversation_items([FunctionCallOutput {
                    call_id: output.call_id,
                    output: output.content,
                }])
                results.push(output)
            Err(e):
                session.send_event(ctx, Error(e))
    return results
    // 返回后, 外层循环的下次迭代将把这些结果作为 prompt 的一部分发给LLM
```

### 11.2 工具调用提取

**文件：** `core/src/stream_events_utils.rs`，第220行

`handle_output_item_done()` 是提取工具调用的关键点：

1. 调用 `ToolRouter::build_tool_call()`（`tools/router.rs`，第175行）
2. 如果是工具调用（`Ok(Some(call))`）：
   - 立即将原始响应项记录到对话历史
   - 创建future：`tool_runtime.handle_tool_call(call, cancellation_token)`
   - 将future推送到 `in_flight: FuturesOrdered` 集合
   - 设置 `needs_follow_up = true`
3. 如果不是工具调用（`Ok(None)`）：
   - 通过 `sess.emit_turn_item_completed()` 发出事件
   - 记录响应项
   - 提取 `last_agent_message`

### 11.3 ToolCallRuntime 并行执行

**文件：** `core/src/tools/parallel.rs`，第64行

```rust
pub(crate) async fn handle_tool_call(
    &self,
    call: ToolCall,
    cancellation_token: CancellationToken,
) -> AgereResult<ToolCallResult> {
    self.handle_tool_call_with_source(call, cancellation_token).await
}
```

工具并行策略：
- **并行工具**（读操作，如搜索、读取文件）：可以与其他并行工具并发执行
- **串行工具**（写操作，如编辑文件、执行命令）：需要写锁，排队执行

### 11.4 工具路由器分派

**文件：** `core/src/tools/router.rs`，第268行

```rust
pub(crate) async fn dispatch_tool_call_with_code_mode_result(
    &self,
    invocation: ToolInvocation,
    cancellation_token: CancellationToken,
) -> ToolCallResult {
    self.registry.dispatch_any(invocation).await
}
```

注册表按名称查找工具处理程序并执行。工具类型包括：
- **MCP工具**：`ToolPayload::Mcp { server, tool, raw_arguments }`
- **Shell命令**：`ToolPayload::Shell { params }`
- **自定义工具**：`ToolPayload::Custom { input }`
- **函数工具**：`ToolPayload::Function { arguments }`
- **工具搜索**：`ToolPayload::ToolSearch`

### 11.5 工具结果如何回到LLM

工具执行结果通过 `record_conversation_items()` 记录到对话历史。在主循环的下一次迭代中，`build_prompt_input()` 读取更新后的历史，包含：
- 之前的assistant消息（含工具调用）
- 所有工具执行结果（`FunctionCallOutput`）

模型因此能看到工具执行结果并决定下一步（继续调用工具或生成最终回复）。


> **扩展阅读**：
> - `ToolHandler` trait 是工具系统的核心抽象，20+ 种内置 handler 实现（详见 Ch21.8-21.9）
> - `ToolOrchestrator` 管理审批+重试流程，串行工具的写锁排队机制（详见 Ch21.10）
> - MCP 工具通过 `McpHandler` 走独立的分发路径（详见 Ch22.5-22.6）
> - 工具执行前/后的 Hook 拦截（详见 Ch23.6）
> - 审批 6 步链路：ExecPolicy → PreToolUse → PermissionRequest → Guardian → 用户 UI → PostToolUse（详见 Ch24.1）

---

## 12. 事件发送与消费链路

> **初学者理解**：如果把 Agent 比作一家公司，`Event` 就是内部通知系统。每当有重要的事情发生——Turn开始了、AI打出了一段字、工具开始执行了、出错了——就会发出一份通知。这份通知经过三层处理（包装→持久化→推送），最终送到所有"订阅者"手中（TUI、远程客户端等）。`AgentStatus` 则像公司门口的状态牌：空闲中/工作中/已下班。

### 12.1 Event 与 EventMsg 定义

**文件：** `protocol/src/protocol.rs`

```rust
// 第1078-1083行
pub struct Event {
    pub id: String,       // 提交/回合 ID
    pub msg: EventMsg,    // 载荷
}

// 第1091-1307行
pub enum EventMsg {
    // 会话生命周期
    SessionConfigured(SessionConfiguredEvent),
    ShutdownComplete,

    // Turn生命周期
    TurnStarted(TurnStartedEvent),
    TurnComplete(TurnCompleteEvent),
    TurnAborted(TurnAbortedEvent),
    ContextCompacted(ContextCompactedEvent),
    ThreadRolledBack(ThreadRolledBackEvent),

    // 消息
    AgentMessage(AgentMessageEvent),
    AgentMessageDelta(AgentMessageDeltaEvent),
    AgentMessageContentDelta(AgentMessageContentDeltaEvent),
    AgentReasoning(AgentReasoningEvent),
    AgentReasoningDelta(AgentReasoningDeltaEvent),
    UserMessage(UserMessageEvent),

    // 工具/操作
    ItemStarted(ItemStartedEvent),
    ItemCompleted(ItemCompletedEvent),
    ExecCommandBegin, ExecCommandDelta, ExecCommandEnd,
    McpToolCallBegin, McpToolCallEnd,
    WebSearchBegin, WebSearchEnd,
    ImageGenerationBegin, ImageGenerationEnd,
    PatchApplyBegin, PatchApplyUpdated, PatchApplyEnd,

    // 审批
    ExecApprovalRequest,
    ApplyPatchApprovalRequest,
    GuardianAssessment,
    RequestPermissions,
    RequestUserInput,
    DynamicToolCallRequest,

    // 追踪
    HookStarted, HookCompleted,
    TokenCount(TokenCountEvent),

    // 系统
    Error(ErrorEvent),
    Warning(WarningEvent),

    // 协作/多代理
    CollabAgentSpawnBegin/End,
    CollabAgentInteractionBegin/End,
    CollabWaitingBegin/End,
    CollabCloseBegin/End,
    CollabResumeBegin/End,

    // ... 更多变体
}
```

### 12.2 事件发送三层次伪代码

```
// ─── 层次1: send_event (高级包装, turn中使用) ───
Session::send_event(turn_context, msg):
    // 记录分布式追踪
    self.services.rollout_thread_trace.record_agere_turn_event(...)

    // 构造并发送
    event = Event { id: turn_context.sub_id, msg }
    self.send_event_raw(event)

    // 副作用
    self.maybe_notify_parent_of_terminal_turn(...)      // 通知父代理
    self.maybe_mirror_event_text_to_realtime(...)       // 镜像到实时对话

// ─── 层次2: send_event_raw (持久化 + 追踪 + 传递) ───
Session::send_event_raw(event):
    // ① 持久化到 rollout 存储 (用于崩溃恢复)
    self.persist_rollout_items([RolloutItem::EventMsg(event.msg.clone())])

    // ② 记录到分布式追踪
    self.services.rollout_thread_trace.record_protocol_event(&event.msg)

    // ③ 传递到通道
    self.deliver_event_raw(event)

// ─── 层次3: deliver_event_raw (状态更新 + 通道发送) ★ ───
Session::deliver_event_raw(event):
    // ① ★ 更新 AgentStatus watch 通道 (所有订阅者收到通知)
    if let Some(status) = agent_status_from_event(&event.msg):
        self.agent_status.send_replace(status)

    // ② ★ 通过无界 async_channel 发送给消费者
    self.tx_event.send(event)           // ← 消费者通过 Agere::rx_event 接收
        .unwrap_or_else(|_| debug!("channel closed, dropping event"))
```

> **设计决策：为什么要分三个层次？** 这是"关注点分离"的体现。`send_event` 负责业务逻辑（追踪、副作用），`send_event_raw` 负责持久化（崩溃恢复），`deliver_event_raw` 负责通道推送和状态同步。每一层都可以独立测试、独立修改，互不影响。

> **Rust知识点：** `send_replace` 是 `watch::Sender` 的方法，它直接覆盖当前值（不阻塞），比 `send` 更高效因为它不需要等待所有接收者读完旧值。

### 12.3 AgentStatus 状态机

**文件：** `core/src/agent/status.rs`，第1-28行

```rust
pub(crate) fn agent_status_from_event(msg: &EventMsg) -> Option<AgentStatus> {
    match msg {
        EventMsg::TurnStarted(_)           => Some(AgentStatus::Running),
        EventMsg::TurnComplete(ev)         => Some(AgentStatus::Completed(ev.last_agent_message)),
        EventMsg::TurnAborted(ev) if Interrupted => Some(AgentStatus::Interrupted),
        EventMsg::TurnAborted(ev)          => Some(AgentStatus::Errored(...)),
        EventMsg::Error(ev)                => Some(AgentStatus::Errored(ev.message)),
        EventMsg::ShutdownComplete         => Some(AgentStatus::Shutdown),
        _                                 => None,  // 无变化
    }
}
```

**状态转换序列：**
```
正常路径：PendingInit → Running → Completed(...)
中断路径：PendingInit → Running → Interrupted
错误路径：PendingInit → Running → Errored(...)
无运行关闭：PendingInit → Shutdown
```

### 12.4 消费端：从Event到JSON-RPC伪代码

```
// ─── 后台监听器任务 (每个会话一个) ───
ensure_conversation_listener_task(conversation, outgoing_sender):
    loop {
        event = conversation.next_event()          // Agere::rx_event.recv()
        thread_state.track_current_turn_event(&event.id, &event.msg)

        // ★ EventMsg → JSON-RPC Notification
        notification = match event.msg:
            EventMsg::TurnStarted       → jsonrpc("conversation/turnStarted")
            EventMsg::TurnComplete      → jsonrpc("conversation/turnCompleted")
            EventMsg::AgentMessageDelta → jsonrpc("conversation/messageDelta")
            EventMsg::AgentReasoningDelta → jsonrpc("conversation/reasoningDelta")
            EventMsg::ExecCommandBegin  → jsonrpc("conversation/execStarted")
            EventMsg::ExecCommandDelta  → jsonrpc("conversation/execDelta")
            EventMsg::ExecCommandEnd    → jsonrpc("conversation/execCompleted")
            EventMsg::ItemStarted       → jsonrpc("conversation/itemStarted")
            EventMsg::ItemCompleted     → jsonrpc("conversation/itemCompleted")
            EventMsg::McpToolCallBegin  → jsonrpc("conversation/mcpToolCallStarted")
            EventMsg::McpToolCallEnd    → jsonrpc("conversation/mcpToolCallCompleted")
            EventMsg::Error             → jsonrpc("conversation/error")
            EventMsg::TokenCount        → jsonrpc("conversation/tokenCount")
            EventMsg::ShutdownComplete  → jsonrpc("conversation/shutdownComplete")
            ...

        // 发送给所有订阅的连接
        outgoing_sender.send_notification(notification)
            // 嵌入模式: → InProcessServerEvent → TUI
            // 远程模式: → WebSocket → 远程客户端
    }
```

### 12.5 消费端：BespokeEventHandling

**文件：** `app-server/src/agere_message_processor.rs`，第6784行

消费端在app-server中运行，是一个后台监听器任务：

```rust
event = conversation.next_event() => {
    thread_state.track_current_turn_event(&event.id, &event.msg);
    apply_bespoke_event_handling(
        event, conversation_id, conversation, outgoing_sender, ...
    ).await;
}
```

**文件：** `app-server/src/bespoke_event_handling.rs`

`apply_bespoke_event_handling` 将每一种 `EventMsg` 变体转换为对应的 JSON-RPC 通知：

| EventMsg | JSON-RPC 通知 |
|----------|--------------|
| `TurnStarted` | `conversation/turnStarted` |
| `TurnComplete` | `conversation/turnCompleted` |
| `AgentMessageDelta` | `conversation/messageDelta` |
| `ExecCommandBegin` | `conversation/execStarted` |
| `ExecCommandEnd` | `conversation/execCompleted` |
| `McpToolCallBegin` | `conversation/mcpToolCallStarted` |
| ... | ... |

这些通知通过 `OutgoingMessageSender` 分发给所有已订阅的连接（对于嵌入模式，通过 `InProcessServerEvent` 通道；对于远程模式，通过WebSocket）。

---

## 13. 子代理生成与代理间通信

> **初学者理解**：Agent 可以"分身"——当任务足够复杂时，主 Agent 可以派生子 Agent 去处理子任务，就像经理分配工作给下属。子代理共享父代理的服务（模型、工具、MCP），但有自己的对话历史和Turn循环。关键设计是：子代理的审批请求不直接发给用户，而是**冒泡**到父代理，由父代理决定是否打断用户。

### 13.1 AgereDelegate 设计

**文件：** `core/src/agere_delegate.rs`，共854行

"Delegate" 不再是独立的 Rust 类型，而是通过自由函数实现的委托模式——一个子 `Agere` 实例，其审批通道路由到父会话。

### 13.2 run_agere_thread_interactive 完整伪代码

```
run_agere_thread_interactive(parent_session, subagent_source, ...) → Agere:
    ── ① ★ 调用 Agere::spawn 创建子代理 ★ ──
    child = Agere::spawn(AgereSpawnArgs {
        session_source: SessionSource::SubAgent(subagent_source),
        persist_extended_history: false,                  // 子代理不持久化
        inherited_exec_policy: Some(parent.exec_policy),  // 继承父级策略
        environments: parent_ctx.environments,            // 继承环境
        models_manager, skills_manager, plugins_manager,  // ★ 共享服务
        mcp_manager, skills_watcher, thread_store,        // ★ 共享服务
        parent_trace: None,                               // 不继承追踪
        ...
    })
    // child = { tx_sub, rx_event, agent_status, session, ... }

    ── ② 创建取消令牌 ──
    cancel_token = parent_cancel_token.child_token()      // 父取消 → 级联到子

    ── ③ ★ 创建通道用于与调用者通信 ★ ──
    (tx_ops, rx_ops) = async_channel::bounded(512)        // 调用者 → 子代理 (操作)
    (tx_sub, rx_sub) = async_channel::unbounded()         // 子代理 → 调用者 (事件)

    ── ④ ★ 启动事件转发任务 (子代理 → 调用者) ★ ──
    tokio::spawn(forward_events(child, tx_sub, cancel_token, parent_session))
        // 循环: child.next_event() → 过滤/拦截 → tx_sub.send(event)

    ── ⑤ ★ 启动操作转发任务 (调用者 → 子代理) ★ ──
    tokio::spawn(forward_ops(child, rx_ops, cancel_token))
        // 循环: rx_ops.recv() → child.submit(submission)

    ── ⑥ ★ 返回通道交换后的 Agere ★ ──
    return Agere {
        tx_sub,                  // 调用者的操作发送端
        rx_event: rx_sub,        // 调用者的事件接收端
        agent_status: child.agent_status.clone(),
        session: parent_session, // 引用父会话 (用于审批冒泡)
        session_loop_termination: child.session_loop_termination,
    }
```

### 13.3 forward_events——事件过滤与转发伪代码

```
forward_events(child_agere, tx_sub, cancel_token, parent_session):
    pending_mcp = HashMap::new()          // 缓存 MCP 调用, 用于处理旧版审批

    loop {
        event = child_agere.next_event().or_cancel(cancel_token)

        match event.msg:
            // ── 忽略: 消费者不关心这些 ──
            AgentMessageDelta | AgentReasoningDelta | TokenCount
            | SessionConfigured | ThreadNameUpdated:
                continue

            // ── ★ 拦截: 审批冒泡到父会话 ★ ──
            ExecApprovalRequest(params):
                decision = if guardian_active:
                    spawn_guardian_review(params)
                else:
                    parent_session.request_command_approval(params)
                child_agere.submit(Op::ExecApproval { call_id, decision })
                continue

            ApplyPatchApprovalRequest(params):
                decision = parent_session.request_patch_approval(params)
                child_agere.submit(Op::PatchApproval { call_id, decision })
                continue

            RequestPermissions(params):
                response = parent_session.request_permissions_for_cwd(params)
                child_agere.submit(Op::RequestPermissionsResponse { call_id, response })
                continue

            RequestUserInput(params):
                // 检查是否是旧版 MCP 审批 (已在 pending_mcp 中)
                if auto_review_mcp_request(params, pending_mcp):
                    child_agere.submit(Op::UserInputAnswer { call_id, answer: auto })
                else:
                    answer = parent_session.request_user_input(params)
                    child_agere.submit(Op::UserInputAnswer { call_id, answer })
                continue

            // ── ★ 转发 + 缓存: MCP工具调用 ★ ──
            McpToolCallBegin(params):
                pending_mcp.insert(params.tool_call_id, params)
                tx_sub.send(event)                         // 转发给消费者

            McpToolCallEnd(params):
                pending_mcp.remove(params.tool_call_id)
                tx_sub.send(event)                         // 转发给消费者

            // ── ★ 转发: 所有其他事件 (TurnComplete, TurnAborted, Error, ...) ★ ──
            _:
                tx_sub.send(event)                         // 转发给消费者
    }
```

### 13.4 事件类型处理策略总表

| 事件类型 | 处理方式 |
|---------|---------|
| `AgentMessageDelta`, `AgentReasoningDelta` | **忽略**（消费者从别处获取流式增量） |
| `TokenCount` | **忽略** |
| `SessionConfigured` | **忽略** |
| `ThreadNameUpdated` | **忽略** |
| `ExecApprovalRequest` | **拦截** → 通过 `parent_session.request_command_approval` 处理 → 用 `Op::ExecApproval` 回复 |
| `ApplyPatchApprovalRequest` | **拦截** → 通过 `parent_session.request_patch_approval` 处理 → 用 `Op::PatchApproval` 回复 |
| `RequestPermissions` | **拦截** → 通过 `parent_session.request_permissions_for_cwd` 处理 → 用 `Op::RequestPermissionsResponse` 回复 |
| `RequestUserInput` | **拦截** → 通过 `parent_session.request_user_input` 处理 → 用 `Op::UserInputAnswer` 回复 |
| `McpToolCallBegin` | **转发**给消费者 + 在 `pending_mcp_invocations` 中缓存 |
| `McpToolCallEnd` | **转发**给消费者 + 从缓存中移除 |
| 所有其他事件 | **转发**给消费者 |

### 13.5 邮箱系统

**文件：** `core/src/agent/mailbox.rs`，第1-72行

每个Session有自己的 `Mailbox`，用于代理间通信：

```rust
pub(crate) struct Mailbox {
    tx: mpsc::UnboundedSender<InterAgentCommunication>,
    seq_tx: watch::Sender<u64>,
}

pub(crate) struct MailboxReceiver {
    rx: mpsc::UnboundedReceiver<InterAgentCommunication>,
    buffer: VecDeque<InterAgentCommunication>,
    last_seen_seq: u64,
}
```

**消息类型：**
```rust
pub struct InterAgentCommunication {
    pub author: AgentPath,
    pub recipient: AgentPath,
    pub other_recipients: Vec<AgentPath>,
    pub content: String,
    pub trigger_turn: bool,
}
```

创建方式（第23-25行）：
```rust
let (tx, rx) = mpsc::unbounded_channel::<InterAgentCommunication>();
let (seq_tx, seq_rx) = watch::channel(0);
```

### 13.6 子代理关闭伪代码

```
shutdown_delegate(agere, cancel_token):
    agere.submit(Op::Interrupt)              // 中断正在运行的turn
    agere.submit(Op::Shutdown)               // 终止子代理循环

    // 排空事件, 等待 TurnAborted 或 TurnComplete
    timeout(500ms):
        loop:
            event = agere.next_event()
            if event is TurnAborted or TurnComplete:
                break                         // 子代理已完全停止
```


> **扩展阅读**：
> - `Mailbox` 系统使用 `mpsc::UnboundedSender` + `watch::Sender<u64>` 实现代理间异步通信（详见 Ch26.4）
> - `InterAgentCommunication` 的 `trigger_turn` 标志控制接收后是否自动启动新 Turn
> - spawn_agent V2 支持 `fork_turns` 参数控制历史继承深度（详见 Ch26.2）
> - `forward_events` 审批冒泡：子代理的审批请求重定向到父代理（详见 Ch26.5）

---

## 14. TUI 事件处理与渲染管线

> **初学者理解**：TUI 看起来是"打字机式"的简单界面，但底层是复杂的异步事件系统。`App::run()` 的 `select!` 宏**同时等待**四种不同来源的事件——键盘按下、内部消息、线程通知、服务器推送——任何一个先到就先处理。渲染则采用 **diff 策略**：比较新旧画面的差异，只重绘变化的部分，节省终端带宽和CPU。

### 14.1 键盘事件分发伪代码

```
handle_key_event(app, key_event):
    // ── 第1优先级: 覆盖层激活时 ──
    if app.overlay.is_active:
        return handle_backtrack_overlay_event(app, key_event)

    // ── 第2优先级: 全局快捷键 ──
    match key_event:
        Alt+Left              → switch_to_prev_thread()
        Alt+Right             → switch_to_next_thread()
        keymap("app/open_transcript")   → open_transcript()
        keymap("app/open_editor")       → open_external_editor()
        keymap("app/copy")              → copy_to_clipboard()
        keymap("app/clear")             → clear_terminal()

    // ── 第3优先级: 聊天级快捷键 ──
    match key_event:
        keymap("chat/increase_reasoning")  → increase_reasoning_effort()
        keymap("chat/decrease_reasoning")  → decrease_reasoning_effort()
        keymap("chat/edit_queued")         → edit_queued_message()

    // ── 第4优先级: 转发给 ChatWidget ──
    if !handled:
        chat_widget.handle_key_event(key_event)
            // 如果 bottom_pane 有活动视图 → 转发给视图
            // 如果 composer 聚焦 → ChatComposer::handle_key_event()
            // 如果未处理 → 检查 quit (Ctrl+C) / interrupt (Ctrl+D)
```

### 14.2 渲染管线伪代码

```
// ─── 当收到 TuiEvent::Draw 或 TuiEvent::Resize ───
render_frame(app, tui, event):
    if event is Resize:
        handle_draw_pre_render()             // resize-reflow: 重新计算布局

    chat_widget.pre_draw_tick()              // 处理计时器
    height = chat_widget.desired_height()    // 计算所需视口高度

    tui.draw(height, |frame|:
        // ① 渲染对话区
        chat_widget.render(frame.area(), frame.buffer)
            // 遍历 transcript_cells, 每个 HistoryCell 渲染为行
            // active_cell (流式cell) 追加到末尾
            // 检测溢出, 需要时向上滚动

        // ② 渲染底部面板
        bottom_pane.render(frame.bottom_area(), frame.buffer)
            // 状态栏 + composer 输入框 + 弹出层 (审批/文件搜索/状态行)
    )

    // ③ ★ diff渲染: 仅将变化写入终端 ★
    new_buffer = frame.buffer
    diff = diff_buffers(prev_buffer, new_buffer)
    for (pos, cell) in diff:
        terminal.write_cell(pos, cell)
    prev_buffer = new_buffer
```

### 14.3 TuiEventStream

**文件：** `tui/src/tui/event_stream.rs`，第51-82行

```rust
pub struct EventBroker<S: EventSource = CrosstermEventSource> {
    state: Mutex<EventBrokerState<S>>,
    resume_events_tx: watch::Sender<()>,
}
```

`TuiEventStream` 轮询两个源：
1. **Crossterm EventStream**（通过共享的 `EventBroker`）
2. **Draw broadcast channel**

Round-robin调度以确保公平性：在draw事件和crossterm事件之间交替。

事件映射：
- `Event::Key(key_event)` → `TuiEvent::Key(key_event)`
- `Event::Resize(cols, rows)` → `TuiEvent::Resize(cols, rows)`
- `Event::Paste(text)` → `TuiEvent::Paste(text)`
- draw channel receive → `TuiEvent::Draw`
- Focus事件：更新 `terminal_focused` AtomicBool 用于桌面通知抑制
- Unix：Ctrl+Z触发作业控制暂停

### 14.4 键位绑定系统

**文件：** `tui/src/keymap.rs`，第273行：`RuntimeKeymap::from_config()`

优先级：上下文特定 > 全局 > 内置默认值

7个绑定上下文：
- `App`：全局应用操作
- `Chat`：聊天特定操作
- `Composer`：输入框操作
- `Editor`：外部编辑器绑定
- `Pager`：分页/滚动
- `List`：列表选择
- `Approval`：审批对话框

保留键：Ctrl+C、Ctrl+D、Ctrl+V、Shift+Tab、Esc、Alt+Left/Right、`/`、`!`、`@`、`$`

### 14.5 自定义Terminal

**文件：** `tui/src/custom_terminal.rs`，第764行

对ratatui `Terminal` 的自定义分支，新增功能：
- **内联视口支持**：当TUI未占据全屏时（例如在IDE集成中）
- **基于diff的渲染**：检测自上一帧以来的变化，仅将差异写入终端
- `Frame` 结构体具有变化的宽度上下文、insets和列布局

**ChatWidget**（`tui/src/chatwidget.rs`，约12300行）是主要的UI组件：
```rust
pub struct ChatWidget {
    active_cell: Option<Box<dyn HistoryCell>>,   // 当前正在流式传输的cell
    stream_controller: Option<StreamController>, // Assistant消息流状态
    plan_stream_controller: Option<PlanStreamController>, // Plan输出流状态
    bottom_pane: BottomPane,                      // 底部区域
    // ... 60+ 个更多字段
}
```

---

## 15. 流式内容显示与提交动画

> **初学者理解**："流式显示"是用户感知到的Agent行为——AI的回复一个字一个字地出现在屏幕上。但背后的机制并不简单：LLM API返回的是原始token流（可能是每个字、每个标点），TUI收到后要积累、解析markdown、重绘。为了既跟得上LLM速度又流畅显示，OpenAgere使用了**自适应分块策略**——内容少时逐行平滑动画，内容多时批量追赶。

### 15.1 从AppServer通知到ChatWidget伪代码

```
App::handle_app_server_event(notification):
    match notification:
        ServerNotification::TurnStarted(id):
            chat_widget.on_turn_started(id)
            frame_requester.schedule_frame()

        ServerNotification::ItemStarted(item):
            chat_widget.handle_item_started(item)
                // 创建 active_cell: TextCell / ExecCell / ToolCallCell
            frame_requester.schedule_frame()

        ServerNotification::ItemDelta(id, delta):
            chat_widget.handle_streaming_delta(id, delta)   // ★ 核心流处理
            frame_requester.schedule_frame()

        ServerNotification::ItemCompleted(id, item):
            chat_widget.handle_item_completed(id, item)
                // 如果是 agent message: finalize stream → ConsolidateAgentMessage
                // 如果是 tool call: 记录完成状态
            frame_requester.schedule_frame()

        ServerNotification::TurnCompleted(result):
            chat_widget.on_turn_completed(result)
            frame_requester.schedule_frame()

        ServerNotification::Error(error):
            chat_widget.show_error(error)
            frame_requester.schedule_frame()
```

### 15.2 StreamController——流式消息处理伪代码

```
ChatWidget::handle_streaming_delta(item_id, delta_text):
    controller = self.stream_controller.as_mut()
    if controller is None: return

    controller.push(delta_text)
        // ─── StreamCore::push_delta ───
        // ① 追加到 Markdown 缓冲区
        markdown_buffer += delta_text

        // ② 检查换行: 有完整行时提交
        if '\n' in delta_text:
            completed_source = markdown_buffer.commit_complete_source()
                // 返回自上次换行以来完成的源片段

            // ③ 渲染累积源
            new_lines = append_markdown(completed_source)
                // 解析 markdown → 渲染为 Line 对象 (含语法高亮)

            // ④ ★ 将新行排入渲染队列 ★
            stream_state.sync_queue_to_render(new_lines)

            // ⑤ 发送动画事件
            app_event_tx.send(AppEvent::StartCommitAnimation)
```

### 15.3 CommitTick动画伪代码

```
// ─── CommitTick 事件处理 ───
App::handle_commit_tick():
    result = run_commit_tick(stream_state, adaptive_policy, transcript_cells)

    if result.did_drain:
        frame_requester.schedule_frame()       // 触发重绘

    if result.has_more:
        app_event_tx.send(AppEvent::CommitTick) // 安排下一次tick

// ─── run_commit_tick 核心逻辑 ───
run_commit_tick(stream_state, policy, transcript_cells):
    snapshot = stream_state.queue_snapshot()
        // { depth: 队列中排队的行数, oldest_age: 最旧行的等待时间 }

    decision = policy.decide(snapshot)

    match decision:
        ChunkingDecision::Smooth:
            // ★ 平滑模式: 每tick排空1行 (逐行动画)
            line = stream_state.dequeue(1)
            transcript_cells.append(line)
            return { did_drain: true, has_more: snapshot.depth > 1 }

        ChunkingDecision::CatchUp:
            // ★ 追赶模式: 一次性排空所有排队行
            lines = stream_state.dequeue_all()
            transcript_cells.extend(lines)
            return { did_drain: true, has_more: false }

        ChunkingDecision::Hold:
            // ★ 保持模式: 暂不排空, 等待更多内容到达
            return { did_drain: false, has_more: true }
```

### 15.4 自适应分块策略

**文件：** `tui/src/streaming/chunking.rs`，第458行

参数设计用于在响应性和流畅性之间取得平衡：

```
进入追赶模式：队列深度 > 8行 或 最旧行 > 120ms
退出追赶模式：队列深度 ≤ 2行 且 最旧行 ≤ 40ms，持续250ms
重新进入保持期：250ms（若严重：深度 > 64 或 最旧 > 300ms 则绕过）
```

- **平滑模式**：每tick排空1行，产生流畅的逐行动画
- **追赶模式**：一次性排空所有排队行，用于快速跟上LLM输出
- **保持模式**：等待更多内容到达，避免抖动的模式切换

### 15.5 最终化伪代码

```
StreamController::finalize():
    // ★ 流式传输结束: 返回完整结果
    remaining = markdown_buffer.flush()              // 排空剩余缓冲区
    final_lines = append_markdown(remaining)
    stream_state.sync_queue_to_render(final_lines)

    // 排空所有排队的行
    while stream_state.has_queued():
        run_commit_tick(...)

    raw_source = markdown_buffer.full_source()       // 完整原始 markdown

    // ★ 创建最终 cell: AgentMarkdownCell (持有完整 markdown 源)
    final_cell = AgentMarkdownCell::new(raw_source, rendered_lines)

    // ★ 发送合并事件: 将瞬时流cell替换为最终cell
    app_event_tx.send(AppEvent::ConsolidateAgentMessage {
        item_id,
        final_cell,
    })
```

---

## 16. 关闭与清理流程

### 16.1 Shutdown处理伪代码

```
// ─── 当 submission_loop 收到 Op::Shutdown ───
shutdown(session, sub_id):
    // ① 中止正在运行的 turn
    session.interrupt_active_turn()

    // ② 发送最后的 ShutdownComplete 事件
    session.send_event_raw(Event {
        id: sub_id,
        msg: EventMsg::ShutdownComplete,
    })

    // ③ 返回 true → submission_loop 退出

// ─── submission_loop 退出后 ───
cleanup_session(session):
    // ① 终止所有统一执行管理器进程
    session.services.exec_manager.terminate_all()

    // ② 关闭所有 MCP 连接
    session.services.mcp_manager.close_all_connections()

    // ③ 排空 guardian 审查会话
    session.guardian_review_session.drain_and_deactivate()

    // ④ 等待所有后台任务完成
    session.wait_for_pending_tasks().await

// ─── 子代理关闭 ───
shutdown_delegate(agere, cancel_token):
    agere.submit(Op::Interrupt)              // 中断正在运行的turn
    agere.submit(Op::Shutdown)               // 终止子代理循环
    // 排空事件, 等待 TurnAborted 或 TurnComplete
    timeout(500ms):
        loop:
            event = agere.next_event()
            if event is TurnAborted or TurnComplete: break

// ─── TUI 退出 ───
App::run() → AppControl::Exit:
    terminal.restore()                       // 退出 raw 模式, 恢复 alt screen
    return AppExitInfo { ... }              // 返回退出信息给 CLI 层
```

### 16.2 cleanup_session

```rust
async fn cleanup_session(sess: &Arc<Session>) {
    // 终止所有统一执行管理器进程
    // 关闭MCP连接
    // 排空guardian状态
    // 发送最后的 ShutdownComplete 事件
}
```

### 16.3 TUI退出

**文件：** `tui/src/lib.rs`，第2338行

`App::run()` 返回 `AppControl::Exit` 时：
1. Tokio运行时开始关闭
2. `TerminalRestoreGuard` 的Drop实现恢复终端状态（退出raw模式、恢复alt screen）
3. `run_ratatui_app()` 返回 `AppExitInfo`
4. 如果有待处理的自动更新，CLI层处理之

### 16.4 完整生命周期伪代码总结

```
// ═══════════════════════════════════════════════════════════
//    从 CLI 参数到 Agent 完成: 完整生命周期 (含 Rust 类型注解)
// ═══════════════════════════════════════════════════════════

// 关键类型速查:
//   Submission = { id: Uuid, op: Op, trace: Option<W3cTraceContext> }
//   Op = UserInput | Interrupt | Shutdown | ExecApproval | ...
//   Event = { id: String, msg: EventMsg }
//   EventMsg = TurnStarted | AgentMessageDelta | TurnComplete | ...
//   Sender<T> = 通道发送端 (只写)
//   Receiver<T> = 通道接收端 (只读)
//   Arc<T> = 原子引用计数共享指针

main():
    runtime = new TokioRuntime(multi_thread, stack=16MB)
    runtime.block_on(cli_main())
    // block_on: 将异步代码桥接到同步main函数

cli_main():
    (config_overrides, subcommand) = MultitoolCli::parse()
    match subcommand:
        None → run_interactive_tui(cli, config_overrides, remote, arg0_paths)

run_interactive_tui():
    config = load_config_or_exit()                // 六层配置堆叠
    app_server_session = AppServerSession::new()  // 嵌入模式: 同进程启动AppServer
    run_ratatui_app(config, app_server_session)

run_ratatui_app():
    terminal = CustomTerminal::new()              // ratatui + crossterm, inline viewport
    tui = Tui::new(terminal)                      // 创建 draw_tx, event_broker, frame_requester
    app = App::new(tui, app_server_session)       // 创建 app_event_tx, chat_widget, keymap
    return app.run()                              // ★ 阻塞 TUI 主循环, 直到用户退出

// ─── 用户输入 "帮我写一个函数" ───
App::run():
    // 用户按下回车
    TuiEvent::Key(Enter) → handle_key_event()
        → ChatComposer::submit()
        → AppEvent::SubmitThreadOp(Op::UserInput { items: ["帮我写一个函数"], ... })
        → InProcessClientSender.send(Notification(Op::UserInput {...}))
            // 通过 mpsc 通道发到 AppServer

    // AppServer 接收并转换为 Submission
    MessageProcessor::process(connection_id, jsonrpc)
        → agere.submit(Op::UserInput { items, ... })
        → tx_sub.send(Submission { id: uuid_v7(), op: Op::UserInput, trace: None })
            // tx_sub 是 async_channel::Sender<Submission>，有界512

    // 提交循环处理
    submission_loop:
        rx_sub.recv() → sub.op = Op::UserInput
        → user_input_or_turn(session, sub_id, sub.op)
            → steer_input() → Err(NoActiveTurn)  // 当前没有运行的Turn
            → spawn_task(RegularTask::new())
                → tokio::spawn(RegularTask::run())

    // Turn 执行 (在 tokio 任务中运行)
    RegularTask::run():
        session.send_event(EventMsg::TurnStarted)
            // → AgentStatus 从 PendingInit → Running

        run_turn(session, context, input, None):
            // 记录用户输入到对话历史
            record_conversation_items([Message { role: "user", content: "帮我写一个函数" }])

            // ★★★ LLM调用循环 ★★★
            loop:  // 循环次数 = 1 (无工具调用) 到 N (多次工具调用)
                prompt = build_prompt_input(history, instructions, tools)
                    // history 包含: system指令 + 用户消息 + 之前轮次的assistant/工具结果

                result = run_sampling_request():
                    stream = client.stream(prompt, model_info)
                        // HTTP POST 或 WebSocket 连接到 LLM API

                    // 逐事件处理 SSE 流
                    while event = stream.next():
                        match event:
                            OutputItemAdded(item):
                                // 新的内容块开始。可能是文本/工具调用/推理
                                active_item = item

                            OutputTextDelta(delta):
                                // ★★★ 关键：每收到一段文字，立即发送事件 ★★★
                                session.send_event(EventMsg::AgentMessageContentDelta(delta))
                                // → deliver_event_raw:
                                //   ① agent_status.send_replace(AgentStatus::Running)
                                //   ② tx_event.send(Event{id, msg: AgentMessageContentDelta(delta)})
                                // → AgereMessageProcessor::next_event() 收到
                                // → apply_bespoke_event_handling():
                                //   → ServerNotification::ItemDelta(id, delta)
                                // → OutgoingMessageSender::send_notification()
                                // → TUI::handle_app_server_event():
                                //   → ChatWidget::handle_streaming_delta(delta)
                                //   → StreamController::push(delta)
                                //   → 换行检测 → commit_complete_source()
                                //   → CommitTick → 逐行动画
                                // → FrameRequester::schedule_frame()
                                // → TuiEvent::Draw → render_frame()

                            OutputItemDone(item):
                                // 内容块完成
                                call = ToolRouter::build_tool_call(item)
                                if call is Some:
                                    // ★ 工具调用：排队执行
                                    in_flight.push(tool_runtime.handle_tool_call(call))
                                else:
                                    // 文本消息：记录为最终回复
                                    last_agent_message = item.text

                            Completed { end_turn: Some(false) }:
                                needs_follow_up = true  // "tool_use" → 还有工具结果要发
                            Completed { end_turn: Some(true) }:
                                needs_follow_up = false // "end_turn" → 模型完成

                    // 等待所有排队的工具执行完
                    drain_in_flight(session, ctx, tool_runtime, cancel)
                        // 每个结果追加到对话历史
                        // 下次循环迭代时作为 prompt 一部分发给 LLM
                        // 例如: history += [{call_id: "call_1", output: "文件内容..."}]

                if !needs_follow_up: break    // 模型说结束了

        // Turn 完成
        session.send_event(EventMsg::TurnComplete { last_agent_message: Some("这是函数...") })
            // → AgentStatus 从 Running → Completed("这是函数...")

    // ─── 用户再次输入 或 Ctrl+C 退出 ───
    ...
    → app.run() returns Exit
    → terminal.restore()     // 恢复终端: 退出raw模式, 恢复光标, 恢复alt screen
    → 进程退出
```

---

## 17. 完整实例追踪：一次真实的 Agent 交互

> 本章用一个具体的用户请求，逐步追踪从键盘输入到最终回复的**完整数据流**。你可以看到每一步通道中传递的数据结构、LLM 的请求/响应内容、工具调用的参数和返回值、以及双层循环（Turn外层 + LLM内层）的运转过程。

### 17.1 场景设定

**用户输入：** "帮我在项目里加一个健康检查的 API 端点，返回 JSON 格式的服务状态"

**项目背景：** `src/api/` 目录下已有 `users.rs` 和 `products.rs`，使用 `axum` 框架。

**审批策略：** `AskForApproval::default()`（写文件和执行命令需要用户确认）

### 17.2 整体时间线概览

```
时间轴 ──────────────────────────────────────────────────────►

[0ms] 用户按Enter
  │
  ▼
[1ms] Submission 进入 tx_sub 通道
  │
  ▼
[2ms] submission_loop 收到 → user_input_or_turn
  │
  ▼
[3ms] RegularTask::run → TurnStarted 事件
  │
  ▼
[5ms] ★ 进入 run_turn 外层循环 ★
  │
  ├── [10ms]  ★ LLM 调用 #1 ★
  │    │  发送: system prompt + 用户消息 + 工具列表
  │    │  模型决定: 先看看现有 API 是怎么写的 (调用 read_file ×2)
  │    │  返回: FunctionCall("read_file", "src/api/users.rs")
  │    │        FunctionCall("read_file", "src/api/products.rs")
  │    │
  │    ├── [15ms]  工具执行: read_file("src/api/users.rs") → 文件内容
  │    ├── [16ms]  工具执行: read_file("src/api/products.rs") → 文件内容
  │    │   (两个读操作并行执行, 因为都是只读工具)
  │    │
  │    └── [18ms]  needs_follow_up = true → 继续内层循环
  │
  ├── [25ms]  ★ LLM 调用 #2 ★
  │    │  发送: 上一次的完整历史 + 两个文件的内容
  │    │  模型决定: 我了解了项目结构, 现在创建 health.rs (调用 write_file)
  │    │  返回: FunctionCall("write_file", {path: "src/api/health.rs", content: "..."})
  │    │
  │    ├── [30ms]  工具执行: write_file(...)
  │    │    │  检测到这是写操作, 触发了审批流程
  │    │    │  ┌──────── 审批子流程 ────────┐
  │    │    │  │ Session 发送              │
  │    │    │  │ EventMsg::ExecApprovalRequest │
  │    │    │  │   → AppServer → TUI       │
  │    │    │  │ TUI 弹出 ApprovalOverlay   │
  │    │    │  │ 用户看到:                 │
  │    │    │  │  "允许写入 src/api/health.rs?" │
  │    │    │  │  [y] 允许  [n] 拒绝       │
  │    │    │  │ 用户按键: y               │
  │    │    │  │ TUI → AppServer → Session │
  │    │    │  │ Op::ExecApproval{id, Approved} │
  │    │    │  │ oneshot 通道解除阻塞       │
  │    │    │  └──────────────────────────┘
  │    │    │  文件写入成功
  │    │
  │    └── [500ms] needs_follow_up = true → 继续内层循环
  │
  ├── [510ms] ★ LLM 调用 #3 ★
  │    │  发送: 完整历史 + 写入成功确认
  │    │  模型决定: 任务完成, 总结一下
  │    │  返回: 纯文本回复 (end_turn = true)
  │    │  流式增量: "已" → "经" → "创" → "建" → ...
  │    │  每个增量 → AgentMessageContentDelta 事件 → TUI 渲染
  │    │
  │    └── [1500ms] needs_follow_up = false → 退出内层循环
  │
  ▼
[1505ms] stop hook → after_agent hook → TurnComplete 事件
  │
  ▼
[1510ms] TUI 显示最终结果, Agent 等待下一次用户输入
```

### 17.3 详细步骤追踪

---

#### 步骤 1：用户按下 Enter——Submission 进入通道

**TUI 层——ChatComposer::submit():**
```
用户输入文本: "帮我在项目里加一个健康检查的 API 端点，返回 JSON 格式的服务状态"

ChatComposer 将文本封装为 AppEvent:
  AppEvent::SubmitThreadOp {
      thread_id: "thread-001",
      op: AppCommand::SubmitUserInput {
          items: [UserInput::Text {
              text: "帮我在项目里加一个健康检查的 API 端点，返回 JSON 格式的服务状态",
              text_elements: []
          }],
          updates: SessionSettingsUpdate::default()
               // cwd: "/home/user/myproject"
               // model: None (使用当前模型, 如 claude-sonnet-4-7)
               // approval_policy: None (使用当前策略)
      }
  }

AppEvent 通过 mpsc::UnboundedSender<AppEvent> 发送 → App::run() 的 select! 捕获
  → event_dispatch.rs 匹配 SubmitThreadOp → submit_thread_op()
  → 判断为需要发送到 AppServer 的操作
  → InProcessClientSender.send(Notification(ClientNotification::SubmitUserInput { ... }))
```

**AppServer 层——MessageProcessor:**
```
InProcessClientMessage 到达 AppServer:
  MessageProcessor::process(connection_id, jsonrpc):
    → 解析为 JSON-RPC method "session/input"
    → 构建 Op::UserInput {
          items: [UserInput::Text {
              text: "帮我在项目里加一个健康检查的 API 端点，返回 JSON 格式的服务状态",
              text_elements: []
          }],
          environments: None,
          final_output_json_schema: None,
          responsesapi_client_metadata: None
      }
    → agere.submit(op)
```

**Core 层——Agere::submit():**
```rust
// 最终发送的 Submission:
Submission {
    id: "01968a2b-7d3c-4f5e-8a9b-c0d1e2f3a4b5",  // UUID v7
    op: Op::UserInput {
        items: [UserInput::Text {
            text: "帮我在项目里加一个健康检查的 API 端点，返回 JSON 格式的服务状态",
            text_elements: []
        }],
        environments: None,
        final_output_json_schema: None,
        responsesapi_client_metadata: None
    },
    trace: None  // 没有父级追踪上下文
}

tx_sub.send(submission).await  // ★ 通过有界通道(容量512)发送到 submission_loop
```

---

#### 步骤 2：submission_loop 接收并分发

```
submission_loop 的 rx_sub.recv() 收到 Submission:
  → match sub.op:
      Op::UserInput { .. } → user_input_or_turn(&sess, sub.id, sub.op)

user_input_or_turn_inner():
  → 创建 TurnContext {
        sub_id: "01968a2b-7d3c-4f5e-8a9b-c0d1e2f3a4b5",
        cwd: AbsolutePathBuf("/home/user/myproject"),
        model_info: ModelInfo {
            name: "claude-sonnet-4-7",
            provider: "anthropic",
            context_window: Some(200000),
            wire_api: WireApi::Anthropic,
            ...
        },
        approval_policy: Constrained(AskForApproval::default()),
        permission_profile: PermissionProfile::default(),
        collaboration_mode: CollaborationMode::None,
        ...
    }
  → steer_input(items, turn_context) → Err(NoActiveTurn)
      // 当前没有正在运行的 Turn, 需要创建新的
  → sess.spawn_task(turn_context, items, RegularTask::new())
```

---

#### 步骤 3：Turn 开始

```
RegularTask::run(context, cancel_token):
  → session.send_event(TurnContext, EventMsg::TurnStarted(TurnStartedEvent {
        thread_id: "thread-001",
        turn_id: "01968a2b-7d3c-4f5e-8a9b-c0d1e2f3a4b5"
    }))
      // → AgentStatus 从 PendingInit → Running
      // → TUI 显示 "Agent is thinking..."

  → 记录用户输入到对话历史:
      record_conversation_items([ResponseInputItem::Message {
          role: "user",
          content: [ContentItem::InputText {
              text: "帮我在项目里加一个健康检查的 API 端点，返回 JSON 格式的服务状态"
          }],
          phase: None
      }])

  → prewarmed_session = None  // 没有预热的 WebSocket 会话

  → ★ 进入外层循环 ★
      第一次迭代: current_input = items (用户输入)
      run_turn(session, context, current_input, None)
```

---

#### 步骤 4：★ LLM 调用 #1——模型决定调研项目结构 ★

```
run_turn() 设置阶段:
  → 注入系统指令 (system prompt):
      "You are a helpful coding assistant. You have access to tools..."
  → 注入可用技能说明:
      "Available skills: file-reading, code-generation, ..."
  → 注入项目上下文 (connectors):
      "Current working directory: /home/user/myproject"
      "Project structure: src/api/users.rs, src/api/products.rs, Cargo.toml, ..."
  → 构建 ModelClientSession

内层循环——第1次迭代:

① 构建 prompt——发送给 LLM 的数据 (完整):

POST /v1/messages  HTTP/1.1
Host: api.anthropic.com
Content-Type: application/json

{
  "model": "claude-sonnet-4-7",
  "max_tokens": 8192,
  "stream": true,
  "system": [
    {
      "type": "text",
      "text": "You are a helpful coding assistant. You have access to tools for reading files, writing files, searching code, and executing shell commands. The user's project is located at /home/user/myproject. Be thorough and careful. Explain your reasoning before taking actions."
    }
  ],
  "messages": [
    {
      "role": "user",
      "content": [
        {
          "type": "text",
          "text": "帮我在项目里加一个健康检查的 API 端点，返回 JSON 格式的服务状态"
        }
      ]
    }
  ],
  "tools": [
    {
      "name": "read_file",
      "description": "Read the contents of a file",
      "input_schema": {
        "type": "object",
        "properties": {
          "path": {"type": "string", "description": "Path to the file"}
        },
        "required": ["path"]
      }
    },
    {
      "name": "write_file",
      "description": "Write or overwrite a file",
      "input_schema": {
        "type": "object",
        "properties": {
          "path": {"type": "string", "description": "Path to write to"},
          "content": {"type": "string", "description": "Content to write"}
        },
        "required": ["path", "content"]
      }
    },
    {
      "name": "grep",
      "description": "Search for a pattern in files",
      "input_schema": {
        "type": "object",
        "properties": {
          "pattern": {"type": "string"},
          "path": {"type": "string"}
        },
        "required": ["pattern"]
      }
    },
    {
      "name": "bash",
      "description": "Execute a shell command",
      "input_schema": {
        "type": "object",
        "properties": {
          "command": {"type": "string"}
        },
        "required": ["command"]
      }
    }
    // ... 更多工具 (MCP 工具, 自定义工具等)
  ]
}

② LLM 返回——SSE 流:

event: message_start
data: {"type":"message_start","message":{"id":"msg_001","model":"claude-sonnet-4-7"}}

event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"我来"}}

// → process_anthropic_sse 解析 → emit(OutputTextDelta("我来"))
// → session.send_event(AgentMessageContentDelta{delta: "我来"})
//    → TUI 收到通知 → ChatWidget 显示 "我来"

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"看看"}}

// 流式显示的细节:
//   ChatWidget::handle_streaming_delta("我来")
//   → StreamController::push("我来")
//   → markdown_buffer: "我来"
//   ChatWidget::handle_streaming_delta("看看")
//   → StreamController::push("看看")
//   → markdown_buffer: "我来看看"
//   ChatWidget::handle_streaming_delta("项目")
//   → markdown_buffer: "我来看看项目"

// ... 继续流式输出 ...

event: content_block_delta
data: {"delta":{"type":"text_delta","text":"中现有的 API 实现，然后创建健康检查端点。"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

// → emit(OutputItemDone(Message{content: "我来看看项目中现有的 API 实现，然后创建健康检查端点。"}))

// LLM 决定调用工具——读取现有文件
event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_001","name":"read_file","input":{}}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\""}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"src/api/"}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"users.rs\"}"}}

event: content_block_stop
data: {"type":"content_block_stop","index":1}

// → emit(OutputItemDone(FunctionCall{
//       name: "read_file",
//       call_id: "toolu_001",
//       arguments: "{\"path\":\"src/api/users.rs\"}"
//   }))

// LLM 还要读另一个文件
event: content_block_start
data: {"index":2,"content_block":{"type":"tool_use","id":"toolu_002","name":"read_file","input":{}}}

// ... 流式传输 input_json_delta ...

event: content_block_stop
data: {"type":"content_block_stop","index":2}

// → emit(OutputItemDone(FunctionCall{
//       name: "read_file",
//       call_id: "toolu_002",
//       arguments: "{\"path\":\"src/api/products.rs\"}"
//   }))

event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null}}

// → emit(Completed{end_turn: Some(false)})  ← ★ 关键: end_turn=false
//   → needs_follow_up = true

③ handle_output_item_done() 提取工具调用:

对于 call_id="toolu_001":
  ToolRouter::build_tool_call() →
    ToolCall {
        tool_name: "read_file",
        call_id: "toolu_001",
        payload: ToolPayload::Function {
            arguments: "{\"path\":\"src/api/users.rs\"}"
        }
    }

  → 记录到对话历史:
      history += ResponseInputItem::Message {
          role: "assistant",
          content: [
              ContentItem::OutputText { text: "我来看看项目中现有的 API 实现..." },
              // (FunctionCall 本身由后续的 item 表示)
          ]
      }
  → tool_runtime.handle_tool_call(call, cancel_token)
  → in_flight.push(future)

对于 call_id="toolu_002": 同上, 读取 src/api/products.rs

④ drain_in_flight()——两个读操作并行执行:

// 因为是只读操作，可以并行
并行任务1: read_file("src/api/users.rs")
  → 实际读取文件内容:
      "use axum::{Router, Json};\nuse serde::Serialize;\n\n
       #[derive(Serialize)]\n
       pub struct User { id: u64, name: String, email: String }\n\n
       pub fn routes() -> Router {\n
           Router::new().route(\"/api/users\", get(list_users))\n
       }\n
       async fn list_users() -> Json<Vec<User>> { ... }"

并行任务2: read_file("src/api/products.rs")
  → "use axum::{Router, Json};\n..."

// 结果回写对话历史:
  history += ResponseInputItem::FunctionCallOutput {
      call_id: "toolu_001",
      output: FunctionCallOutputPayload { text: "use axum::{Router, Json};..." }
  }
  history += ResponseInputItem::FunctionCallOutput {
      call_id: "toolu_002",
      output: FunctionCallOutputPayload { text: "use axum::{Router, Json};..." }
  }
```

---

#### 步骤 5：★ LLM 调用 #2——模型决定创建文件 ★

```
内层循环——第2次迭代:

① 构建 prompt——发送给 LLM 的数据:

{
  "model": "claude-sonnet-4-7",
  "messages": [
    {
      "role": "user",
      "content": [{"type": "text", "text": "帮我在项目里加一个健康检查的 API 端点..."}]
    },
    {
      "role": "assistant",
      "content": [
        {"type": "text", "text": "我来看看项目中现有的 API 实现，然后创建健康检查端点。"},
        {
          "type": "tool_use",
          "id": "toolu_001",
          "name": "read_file",
          "input": {"path": "src/api/users.rs"}
        },
        {
          "type": "tool_use",
          "id": "toolu_002",
          "name": "read_file",
          "input": {"path": "src/api/products.rs"}
        }
      ]
    },
    {
      "role": "user",
      "content": [
        {
          "type": "tool_result",
          "tool_use_id": "toolu_001",
          "content": "use axum::{Router, Json};\nuse serde::Serialize;\n\npub struct User { ... }\n\npub fn routes() -> Router {\n    Router::new().route(\"/api/users\", get(list_users))\n}\n..."
        }
      ]
    },
    {
      "role": "user",
      "content": [
        {
          "type": "tool_result",
          "tool_use_id": "toolu_002",
          "content": "use axum::{Router, Json};\nuse serde::Serialize;\n\npub struct Product { ... }\n\npub fn routes() -> Router {\n    Router::new().route(\"/api/products\", get(list_products))\n}\n..."
        }
      ]
    }
  ]
  // 注意: 现在对话历史包含了 4 条消息 (用户→助手(含工具调用)→工具结果1→工具结果2)
}

② LLM 返回:

event: content_block_start  [text block]
// ... 流式输出文本 ...
data: {"delta":{"type":"text_delta","text":"我了解了项目的 API 结构。"}}
data: {"delta":{"type":"text_delta","text":"现在创建 health.rs 端点。它使用同样的 Router 模式。"}}

event: content_block_start  [tool_use block]
// ... 流式输出 JSON ...
data: {"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"src/api/health.rs\",\"content\":\"use axum::{Router, Json};\\nuse serde::Serialize;\\n\\n#[derive(Serialize)]\\n..."}}

event: content_block_stop  [tool_use block]

// → emit(OutputItemDone(FunctionCall{
//       name: "write_file",
//       call_id: "toolu_003",
//       arguments: "{\"path\":\"src/api/health.rs\",\"content\":\"use axum::{Router, Json};\\nuse serde::Serialize;\\n\\n#[derive(Serialize)]\\npub struct HealthStatus {\\n    status: String,\\n    uptime: u64,\\n}\\n\\npub fn routes() -> Router {\\n    Router::new().route(\\\"/api/health\\\", get(health_check))\\n}\\n\\nasync fn health_check() -> Json<HealthStatus> {\\n    Json(HealthStatus {\\n        status: \\\"ok\\\".to_string(),\\n        uptime: std::process::id() as u64,\\n    })\\n}\"}"
//   }))

event: message_delta
data: {"stop_reason":"tool_use"}

// → Completed{end_turn: Some(false)} → needs_follow_up = true

③ 工具执行——write_file 触发审批流程:

ToolRouter::build_tool_call() →
  ToolCall {
      tool_name: "write_file",
      call_id: "toolu_003",
      payload: ToolPayload::Function {
          arguments: "{\"path\":\"src/api/health.rs\",\"content\":\"...\"}"
      }
  }

→ tool_runtime.handle_tool_call(call, cancel_token)
→ 检测到这是写操作 (write 需要审批)
→ ★ 触发审批流程 ★

┌─────────────────────────────────────────────────────────┐
│                    审批流程详细数据                         │
├─────────────────────────────────────────────────────────┤

│ ① Session 发送审批请求:
│
│ ExecApprovalRequestEvent {
│     call_id: "toolu_003",
│     approval_id: None,
│     turn_id: "01968a2b-7d3c-4f5e-8a9b-c0d1e2f3a4b5",
│     command: ["write_file", "src/api/health.rs"],
│     cwd: AbsolutePathBuf("/home/user/myproject"),
│     reason: Some("创建新文件 src/api/health.rs"),
│     additional_permissions: None,
│     available_decisions: None,  // 使用默认选项
│     ...
│ }
│
│ session.send_event(EventMsg::ExecApprovalRequest(event))
│   → deliver_event_raw():
│       ① agent_status 不变 (保持 Running)
│       ② tx_event.send(Event{id: "...", msg: ExecApprovalRequest(...)})
│
│ ★ oneshot 通道创建:
│   (tx_approve, rx_approve) = oneshot::channel()
│   存储: active_turn.pending_approvals["toolu_003"] = tx_approve
│   ★ 工具执行线程在此阻塞等待 rx_approve ★
│
│ ② AppServer → TUI:
│
│ AgereMessageProcessor 的 next_event() 收到 Event
│   → apply_bespoke_event_handling()
│   → ServerRequest::CommandExecutionRequestApproval(event)
│   → OutgoingMessageSender.send_request()
│   → InProcessServerEvent::ServerRequest(request)
│
│ ③ TUI 端:
│
│ App::handle_app_server_event(ServerRequest(request)):
│   → ChatWidget::on_exec_approval_request(id, event)
│   → build ApprovalRequest::Exec {
│         call_id: "toolu_003",
│         command: ["write_file", "src/api/health.rs"],
│         cwd: "/home/user/myproject",
│         reason: "创建新文件 src/api/health.rs"
│     }
│   → BottomPane::push_approval_request()
│   → 创建 ApprovalOverlay 并推入 view_stack
│
│ ④ TUI 渲染审批对话框:
│
│ ┌──────────────────────────────────────────┐
│ │  ⚠ Agere  wants to run a command        │
│ │                                          │
│ │  Command: write_file src/api/health.rs   │
│ │  Reason:  创建新文件 src/api/health.rs   │
│ │  CWD:     /home/user/myproject           │
│ │                                          │
│ │  ────────────────────────────────────    │
│ │  > [y] Yes, proceed                     │
│ │    [p] Yes, and don't ask again for...  │
│ │    [a] Yes, and don't ask again         │
│ │    [d] No, continue without running it  │
│ │    [n] No, and tell Agere what to do    │
│ │  ────────────────────────────────────    │
│ └──────────────────────────────────────────┘
│
│ ⑤ 用户按键 y:
│
│ ApprovalOverlay::handle_key_event(KeyEvent('y'))
│   → try_handle_shortcut() → 匹配到 options[0].shortcut = 'y'
│   → apply_selection(0)
│   → handle_exec_decision(id="toolu_003", decision=ReviewDecision::Approved)
│   → app_event_tx.exec_approval(thread_id, "toolu_003", Approved)
│   → AppEvent::SubmitThreadOp {
│         thread_id: "thread-001",
│         op: Op::ExecApproval {
│             id: "toolu_003",
│             turn_id: None,
│             decision: ReviewDecision::Approved
│         }
│     }
│
│ ⑥ 审批决定返回 Core:
│
│ AppServer 收到 → MessageProcessor
│   → agere.submit(Op::ExecApproval { id: "toolu_003", decision: Approved })
│   → tx_sub.send(Submission { id: "...", op: Op::ExecApproval { ... } })
│
│ submission_loop 收到:
│   → exec_approval(session, "toolu_003", None, Approved)
│   → sess.notify_approval("toolu_003", Approved)
│
│ ★ notify_approval 解除 oneshot:
│   let tx_approve = pending_approvals.remove("toolu_003");
│   tx_approve.send(Approved);   // ★ 解除工具执行线程的阻塞!
│
│ ⑦ 工具执行继续:
│   rx_approve.recv() → Approved
│   → write_file 实际执行:
│       创建 src/api/health.rs
│       写入内容: "use axum::{Router, Json};..."
│       → 成功: 512 bytes written
│
└─────────────────────────────────────────────────────────┘

④ 结果回写对话历史:

  history += ResponseInputItem::FunctionCallOutput {
      call_id: "toolu_003",
      output: FunctionCallOutputPayload {
          text: "File written successfully: src/api/health.rs (512 bytes)"
      }
  }
```

---

#### 步骤 6：★ LLM 调用 #3——模型完成任务 ★

```
内层循环——第3次迭代:

① 构建 prompt——发送给 LLM 的数据:

{
  "messages": [
    // ... 之前的4条消息 ...
    {
      "role": "assistant",
      "content": [
        {"type": "text", "text": "我了解了项目的 API 结构。现在创建 health.rs 端点。"},
        {
          "type": "tool_use",
          "id": "toolu_003",
          "name": "write_file",
          "input": {"path": "src/api/health.rs", "content": "use axum::{Router, Json};..."}
        }
      ]
    },
    {
      "role": "user",
      "content": [{
        "type": "tool_result",
        "tool_use_id": "toolu_003",
        "content": "File written successfully: src/api/health.rs (512 bytes)"
      }]
    }
  ]
  // 现在有 6 条消息的完整对话历史
}

② LLM 返回:

event: message_start

// 流式输出最终回复:
event: content_block_delta
data: {"delta":{"type":"text_delta","text":"已创建"}}

// → session.send_event(AgentMessageContentDelta{delta: "已创建"})
// → TUI 显示 "已创建"

event: content_block_delta
data: {"delta":{"type":"text_delta","text":"健康检查"}}

// → TUI 显示 "已创建健康检查"

// ... 继续流式输出 ...

event: content_block_delta
data: {"delta":{"type":"text_delta","text":"API 端点。"}}

// → TUI 显示 "已创建健康检查 API 端点。"

// ★★★ 关键: stop_reason = "end_turn" ★★★
event: message_delta
data: {"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null}}

// → emit(Completed{end_turn: Some(true)})
// → needs_follow_up = false

// 没有工具调用 (handle_output_item_done 只提取到纯文本)

③ 判断:

  needs_follow_up = false → 准备退出内层循环

  // stop hook 检查:
  stop_decision = run_hook("stop")
    // 用户可能自定义了 stop hook, 但这里没有 → Allow

  // after_agent hook:
  run_hook("after_agent")
    // 后处理, 如发送桌面通知等

  break  // ★ 退出内层循环 ★

④ 返回:

  run_turn() 返回 Some("已创建健康检查 API 端点。\n\n我在 `src/api/health.rs` 创建了...")
  // 这是最后一次 LLM 调用中 assistant 的完整文本

⑤ RegularTask 外层循环检查:

  pending = session.drain_pending_input()
  pending.is_empty() → true → 退出外层循环

⑥ 发送 TurnComplete 事件:

  session.send_event(EventMsg::TurnComplete(TurnCompleteEvent {
      thread_id: "thread-001",
      turn_id: "01968a2b-7d3c-4f5e-8a9b-c0d1e2f3a4b5",
      last_agent_message: Some("已创建健康检查 API 端点。\n\n我在 `src/api/health.rs` 创建了..."),
      usage: Some(TokenUsage { input_tokens: 2847, output_tokens: 412 }),
      ...
  }))
    // → AgentStatus 从 Running → Completed("已创建健康检查...")
    // → TUI 收到通知 → 显示完成状态, 等待下一次用户输入
```

---

### 17.4 关键数据流总结

```
┌─ Turn 外层循环 ──── 共1次迭代 (没有 pending 注入) ────────────────┐
│                                                                   │
│  ┌─ LLM 内层循环 ──── 共3次迭代 ────────────────────────────┐     │
│  │                                                           │     │
│  │  迭代1: LLM → 文本: "我来看看..."                          │     │
│  │                → read_file("users.rs")   ← 并行执行        │     │
│  │                → read_file("products.rs") ← 并行执行        │     │
│  │                → needs_follow_up = true                    │     │
│  │                                                           │     │
│  │  迭代2: LLM → 文本: "现在创建..."                          │     │
│  │                → write_file("health.rs")                   │     │
│  │                   → ┌ 审批中断 ┐                           │     │
│  │                   → │ 用户确认 │ → Approved                │     │
│  │                   → └──────────┘                           │     │
│  │                → needs_follow_up = true                    │     │
│  │                                                           │     │
│  │  迭代3: LLM → 文本: "已创建健康检查..."/"HealthCheck..."    │     │
│  │                → 无工具调用                                 │     │
│  │                → needs_follow_up = false → 退出            │     │
│  │                                                           │     │
│  └───────────────────────────────────────────────────────────┘     │
│                                                                   │
│  pending 输入为空 → 退出外层循环                                    │
└───────────────────────────────────────────────────────────────────┘

累计发生:
  - 3 次 LLM 调用 (每次携带的对话历史逐次增长)
  - 2 次只读工具调用 (并行执行)
  - 1 次写工具调用 (含审批流程)
  - 1 次用户审批交互 (ApprovalOverlay)
  - 几十次 AgentMessageContentDelta 事件 (流式文本, 每次几个字)
  - 多次帧重绘 (FrameRequester 合并调度)
  - 4 次 AgentStatus 状态变更:
      PendingInit → (TurnStarted) → Running → (TurnComplete) → Completed
```

### 17.5 如果用到了子代理

如果用户请求更复杂（比如"同时给三个微服务添加健康检查端点"），主 Agent 可能派生子代理：

```
主 Agent:
  派生子代理1 → 处理 service-a 的健康检查
  派生子代理2 → 处理 service-b 的健康检查
  派生子代理3 → 处理 service-c 的健康检查

  子代理内部各有自己的 Turn 循环和 LLM 调用

  子代理的审批请求被 forward_events 拦截
    → 冒泡到父代理
    → 父代理决定是直接批准、询问用户、还是交给 guardian 审查

  子代理完成后发送 TurnComplete
    → 父代理收集结果
    → 父代理整合三个子代理的输出
    → 父代理的 Turn 继续 → 最终回复用户

```


## 18. ModelProvider 与 ModelsManager

> **初学者理解**：ModelProvider 是"模型厂商的抽象接口"，ModelsManager 是"模型目录管理器"。类比：Provider 是 USB 集线器——不管插什么设备，只要符合 USB 协议就能用。Manager 是设备管理器——列出已连接设备、刷新列表、选择默认设备。两者配合：Provider 决定了"用哪个通信协议"，Manager 决定了"当前哪个模型可用"。

### 18.1 核心架构

```
config.toml / 内置默认值
        │
        ▼
ModelProviderInfo (序列化配置: name, base_url, wire_api, auth...)
        │
        ▼ create_model_provider()
ModelProvider (trait: info(), auth(), api_provider(), models_manager())
        │
        ├── ConfiguredModelProvider (wire_api=Responses, AuthManager)
        ├── AnthropicModelProvider (wire_api=Anthropic, env API key)
        └── AmazonBedrockModelProvider (wire_api=Responses, SigV4 auth)
        │
        ▼ models_manager()
ModelsManager (trait: list_models(), build_available_models())
        │
        ├── OpenAiModelsManager (GET /models → 内存+磁盘缓存)
        └── StaticModelsManager (静态列表，无刷新)
```

### 18.2 ModelProviderInfo —— 可序列化的提供商配置

**文件：** `model-provider-info/src/lib.rs:87-136`

详细字段表：

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | `String` | 友好显示名称 |
| `base_url` | `Option<String>` | API 基础 URL |
| `env_key` | `Option<String>` | 存储 API Key 的环境变量名 |
| `env_key_instructions` | `Option<String>` | 获取 API Key 的帮助文本 |
| `experimental_bearer_token` | `Option<String>` | 直接 bearer token（不推荐使用） |
| `auth` | `Option<ModelProviderAuthInfo>` | Command-backed bearer-token 认证 |
| `aws` | `Option<ModelProviderAwsAuthInfo>` | AWS SigV4 认证配置 |
| `wire_api` | `WireApi` | 通信协议：`Responses` 或 `Anthropic` |
| `query_params` | `Option<HashMap>` | base URL 附加查询参数 |
| `http_headers` | `Option<HashMap>` | 静态 HTTP 请求头 |
| `env_http_headers` | `Option<HashMap>` | 从环境变量读取的动态 HTTP 头 |
| `request_max_retries` | `Option<u64>` | HTTP 请求重试上限（默认 4） |
| `stream_max_retries` | `Option<u64>` | 流式重连次数上限（默认 5） |
| `stream_idle_timeout_ms` | `Option<u64>` | 流式空闲超时（默认 300 秒） |
| `websocket_connect_timeout_ms` | `Option<u64>` | WebSocket 连接超时（默认 15 秒） |
| `requires_provider_auth` | `bool` | 是否需要提供商登录认证 |
| `supports_websockets` | `bool` | 是否支持 Responses API WebSocket 传输 |

**WireApi 枚举** (`model-provider-info/src/lib.rs:46`)：

```rust
pub enum WireApi {
    Responses,   // OpenAI Responses API (/v1/responses)
    Anthropic,   // Anthropic Messages API (/v1/messages)
}
// 注意: "Chat" API (wire_api = "chat") 已废弃迁移
```

**ProviderCapabilities** — 标志控制可选功能：

```rust
pub struct ProviderCapabilities {
    pub namespace_tools: bool,   // 支持命名空间工具（MCP 分组展示）
    pub image_generation: bool,   // 支持图片生成功能
    pub web_search: bool,         // 支持网络搜索功能
}
```

### 18.3 ModelProvider trait —— 运行时抽象

**文件：** `model-provider/src/provider.rs:81`

```rust
#[async_trait]
pub trait ModelProvider: fmt::Debug + Send + Sync {
    fn info(&self) -> &ModelProviderInfo;
    fn capabilities(&self) -> ProviderCapabilities;
    fn auth_manager(&self) -> Option<Arc<AuthManager>>;
    async fn auth(&self) -> Option<AgereAuth>;
    fn account_state(&self) -> ProviderAccountResult;
    async fn api_provider(&self) -> Result<Provider>;
    async fn api_auth(&self) -> Result<SharedAuthProvider>;
    fn models_manager(...) -> SharedModelsManager;
}
```

**工厂函数分发逻辑** (`model-provider/src/provider.rs:130`)：

```rust
pub fn create_model_provider(provider_info, auth_manager) -> Box<dyn ModelProvider> {
    match provider_info.wire_api {
        WireApi::Anthropic => AnthropicModelProvider::new(provider_info),
        _ if provider_info.is_amazon_bedrock() => AmazonBedrockModelProvider::new(provider_info),
        _ => ConfiguredModelProvider::new(provider_info, auth_manager),
    }
}
```

### 18.4 三种 Provider 实现详解

#### 1. ConfiguredModelProvider — OpenAI 兼容

**文件：** `model-provider/src/provider.rs:144`

- 默认 Provider，适配所有 `wire_api=Responses` 的服务
- **Auth**: 通过 `AuthManager`（`requires_provider_auth=true` 时）或环境变量 API key（`env_key`）
- **ModelsManager**: `OpenAiModelsManager`（动态 /models 端点，支持刷新和缓存）
- **Capabilities**: 由 `ModelProviderInfo` 配置控制
- **WebSocket**: 支持（`supports_websockets=true` 时）

#### 2. AnthropicModelProvider

**文件：** `model-provider/src/anthropic/mod.rs:56`

- **WireApi**: Anthropic Messages API (`/v1/messages`)
- **Auth**: 环境变量 API key（`env_key`）或直接 bearer token（`experimental_bearer_token`）
- **ModelsManager**: `StaticModelsManager`（Anthropic 不提供 /models 端点）
- **Capabilities**: **全部禁用** — namespace_tools=false, image_generation=false, web_search=false
- **内置模型** (`model-provider/src/anthropic/catalog.rs:16`)：

| 模型名 | 用途 |
|--------|------|
| `claude-sonnet-4-6` | Sonnet 4.6 |
| `claude-opus-4-7` | Opus 4.7 |
| `deepseek-v4-flash` | DeepSeek V4 Flash |

#### 3. AmazonBedrockModelProvider

**文件：** `model-provider/src/amazon_bedrock/mod.rs:30`

- **WireApi**: Responses (via Mantle endpoint, `https://bedrock-mantle.us-east-1.api.aws/openai/v1`)
- **Auth**: AWS SigV4 签名（无 provider auth manager，无 env keys）
- **ModelsManager**: `StaticModelsManager`
- **Capabilities**: **全部禁用**
- **内置模型** (`model-provider/src/amazon_bedrock/catalog.rs:20`)：

| 模型名 | 用途 |
|--------|------|
| `openai.gpt-5.4` | 默认模型 |
| `openai.gpt-oss-120b` | 120B OSS 模型 |
| `openai.gpt-oss-20b` | 20B OSS 模型 |

### 18.5 ModelsManager —— 模型目录管理

**文件：** `models-manager/src/manager.rs:77`

```rust
pub trait ModelsManager: Send + Sync + Debug {
    async fn list_models(&self, strategy: RefreshStrategy) -> Vec<ModelPreset>;
    async fn raw_model_catalog(&self, strategy: RefreshStrategy) -> Option<ModelsResponse>;
    fn get_default_model(&self) -> Option<ModelInfo>;
    fn get_model_info(&self, model_slug: &str) -> Option<ModelInfo>;
    async fn refresh_if_new_etag(&self) -> Result<()>;
}
```

#### RefreshStrategy —— 刷新策略

```rust
pub enum RefreshStrategy {
    Online,              // 始终从网络获取
    Offline,             // 仅使用磁盘缓存
    OnlineIfUncached,    // 缓存有效则用缓存，过期才请求
}
```

#### OpenAiModelsManager 工作流程

```
list_models(strategy):
  should_refresh_models():
    仅当 Provider 使用 Agere 后端或有 command auth 时返回 true
    └─ 否 → 仅使用离线缓存

  match strategy:
    Offline → try_load_cache() 仅读磁盘
    OnlineIfUncached:
      try_load_cache():
        1. 文件存在且可解析?
        2. client_version 匹配当前二进制版本?
        3. fetched_at 在 TTL (5 分钟) 内?
        └─ 全部通过 → 返回缓存
        └─ 任一失败 → GET /models → 解析 → 写入磁盘
    Online:
      直接 GET /models → 解析 → 写入磁盘

  缓存文件: $AGERE_HOME/models_cache.json
  支持 ETag 条件请求（If-None-Match）

  build_available_models(remote_models):
    排序 → 转 ModelPreset → 过滤 auth 不可用 → 标记默认模型
```

**StaticModelsManager** (`models-manager/src/manager.rs:191`)：
存储 `Vec<ModelInfo>` 在内存中，无需网络刷新。`refresh_if_new_etag()` 是 no-op。用于 Anthropic 和 Bedrock。

### 18.6 模型回退机制

**文件：** `models-manager/src/model_info.rs:66`

```
get_model_info(slug="custom/gpt-5.3"):
  1. 最长前缀匹配: 在 remote catalog 中查找 "custom/gpt-5.3"
  2. 命名空间后缀匹配: 回退查找 "gpt-5.3"
  3. slug 推断: 未知模型默认 272K context window
  4. with_config_overrides(): 应用用户配置覆盖 (context_window, auto_compact, tool_output_token_limit, base_instructions)
```

### 18.7 内置 Provider 完整对比

| 特性 | OpenAI | Amazon Bedrock | Anthropic |
|------|--------|----------------|-----------|
| WireApi | Responses | Responses | Anthropic |
| 基础 URL | api.openai.com | bedrock-mantle...aws | api.anthropic.com |
| 认证方式 | AuthManager | AWS SigV4 | env API key |
| 模型管理 | OpenAiModelsManager (动态) | StaticModelsManager (静态) | StaticModelsManager (静态) |
| WebSocket 支持 | 是 | 否 | 否 |
| Capabilities | 功能齐全 | 全部禁用 | 全部禁用 |
| 可自定义 | 不可覆盖 | 仅 aws.profile/region | 可自定义 |

#### OSS Provider (gpt-oss)

**文件：** `model-provider-info/src/lib.rs:461`

```rust
create_oss_provider_with_base_url(url)
```

- 任意 base_url，无认证，无重试
- 用于 LM Studio、Ollama 等本地/开源端点
- 模型名: `gpt-oss`

### 18.8 端到端数据流

```
config.toml [model_provider] = "anthropic"
  │
  ▼
create_model_provider("anthropic")
  ├─ provider_info.wire_api == Anthropic
  └─ → AnthropicModelProvider::new(provider_info)
        └─ models_manager = StaticModelsManager(catalog)
             └─ list_models(Online) → [claude-sonnet-4-6, claude-opus-4-7, deepseek-v4-flash]
               └─ build_available_models() → Vec<ModelPreset>
                  └─ TUI 选择器显示可用模型列表
  │
  ▼
SessionConfiguration {
    model: "claude-opus-4-7",   // 用户选择或默认
    provider: provider_info,
    collaboration_mode: ...,
}
```

启动时通过 `ThreadConfigSnapshot` 序列化到 rollout，恢复时反序列化回 `SessionConfiguration`。

### 18.9 完整实例追踪

**场景**：用户首次启动 Agere，默认使用 Anthropic Provider

```
1. CLI 启动 → 加载 config.toml
   └─ 读取: model_provider = "anthropic"

2. 构建 ModelProvider
   └─ create_model_provider(provider_info, auth_manager)
      └─ provider_info.wire_api == Anthropic
      └─ → AnthropicModelProvider::new(provider_info)
         ├─ info().base_url = "https://api.anthropic.com"
         ├─ info().env_key = "ANTHROPIC_API_KEY"
         ├─ capabilities() → { namespace_tools: false, image_generation: false, web_search: false }
         └─ models_manager() → StaticModelsManager(catalog)

3. 模型解析
   └─ get_model_info("claude-sonnet-4-6")
      └─ 最长前缀匹配 → ModelInfo {
            name: "claude-sonnet-4-6",
            context_window: Some(200000),
            max_tool_call_output_tokens: Some(4096),
            ...
         }

4. 会话配置
   └─ SessionConfiguration.model = "claude-sonnet-4-6"
   └─ 持久化到 rollout JSONL
```

### 18.10 设计决策

**Q1: 为什么 WireApi 只有两种（Responses / Anthropic）？**

**A:** Chat API（`wire_api = "chat"`）已在配置迁移中移除。Responses API 是推荐标准，Anthropic API 仅用于 Anthropic 模型。减少维护成本和代码分支。

**Q2: 为什么 Anthropic 用 StaticModelsManager 而不是动态刷新？**

**A:** Anthropic API 不提供 `/models` 端点。静态列表硬编码在 `model-provider/src/anthropic/catalog.rs` 中。当 Anthropic 发布新模型时需要升级客户端。

**Q3: 为什么 Amazon Bedrock 用 SigV4 而不是 API key？**

**A:** AWS 的标准认证方式是 SigV4 签名。统一认证方式避免用户管理额外的 API key，也避免了在 config.toml 中存储敏感凭证。

**Q4: 为什么缓存验证要检查 client_version？**

**A:** 版本升级可能改变模型目录结构（如新增字段、重命名模型）。旧版本缓存会导致模型不可用或信息错误。版本匹配确保缓存与当前客户端一致。

---

## 19. /model 模型切换全链路

> **初学者理解**：在 Agere 中切换模型类似换引擎——随时切换，上下文和工具状态不变。新模型自动收到"切换指令"告知对话历史。类比：开车时换挡位——车速不变，但动力特性改变。模型切换不会中断当前 Turn（当前 Turn 用旧模型完成），新模型在下个 Turn 收到包含历史背景的 `<model_switch>` 指令。

### 19.1 三种触发方式

| 触发方式 | 入口 | 说明 |
|---------|------|------|
| `/model` 命令 | TUI 输入框 | 打开完整模型选择器弹窗 |
| 状态栏 / 快捷方式 | 点击状态栏或按 `Ctrl+R` | 快速切换 reasoning effort |
| 启动时模型迁移 | 自动弹出 | 厂商标记旧模型过时，推荐替代方案 |

### 19.2 /model 完整执行流程

**涉及文件：** `tui/src/chatwidget.rs:8793`, `tui/src/chatwidget/slash_dispatch.rs:177`, `tui/src/app/event_dispatch.rs:868`

```
用户输入 "/model" 并按 Enter
    │
    ├─ ChatComposer::handle_key_event()
    │   └─ "/" 键 → 激活 CommandPopup → 过滤 "model"
    │      └─ Enter → try_dispatch_bare_slash_command("model")
    │         └─ 返回 InputResult::Command(SlashCommand::Model)
    │
    ├─ ChatWidget::handle_slash_command_dispatch(Model)         // slash_dispatch.rs:177
    │   └─ ChatWidget::open_model_popup()                       // chatwidget.rs:8793
    │       ├─ is_session_configured? → 否 → 提示"请等待 Agent 初始化"
    │       ├─ model_catalog.try_list_models()                  // model_catalog.rs
    │       │   └─ manager.list_models(OnlineIfUncached)
    │       │      └─ 缓存有效(5min TTL) → 使用缓存
    │       │      └─ 缓存过期 → GET /models → 解析 → 写入磁盘
    │       │      └─ build_available_models(remote_models)
    │       │         └─ 排序 → 转 ModelPreset → 过滤不可用 → 标记默认
    │       │
    │       └─ open_model_popup_with_presets(presets)           // chatwidget.rs:9081
    │           ├─ 过滤: show_in_picker == true
    │           ├─ 分组: 按 provider 分组显示
    │           └─ 创建 ModelSelectionPopup 链表
    │
    ├─ 用户在弹窗中选择模型 (Up/Down 导航, Enter 确认)
    │   └─ ModelSelectionPopup::handle_key_event()
    │       ├─ 可选 reasoning effort 子选择 (Minimal → XHigh)
    │       └─ 触发 AppEvent::PersistModelSelection { model, effort }
    │
    └─ App::handle_event(PersistModelSelection)                  // event_dispatch.rs:868
        └─ ConfigEditsBuilder::set_model(model, effort).apply()
            ├─ 写入 config.toml [model] 段
            ├─ 发送 info_message: "Model set to claude-opus-4-7"
            └─ 下次 Turn 自动注入 <model_switch> 开发者消息
```

### 19.3 CommandPopup —— 可搜索选择器

**文件：** `tui/src/bottom_pane/command_popup.rs:133`

当用户按下 `/` 时，`CommandPopup` 被激活：

```rust
fn filtered(&self, filter_text: &str) -> Vec<SlashCommand> {
    // 1. 精确匹配（排在最前面）
    let exact = self.commands.iter().find(|c| c.name() == filter_text);
    // 2. 前缀匹配（排在精确匹配之后）
    let prefix = self.commands.iter()
        .filter(|c| c.name().starts_with(filter_text));
    exact.into_iter().chain(prefix).collect()
}
```

**功能特性：**
- **Tab 补全**：自动完成当前输入的命令名
- **特性门控**：Plans / Apps / Plugins / Fast / Goal / Personality / Realtime / Settings / Collab 根据功能开关显示
- **别名隐藏**：`quit` 和 `approvals` 在默认列表中隐藏，输入前缀时才显示
- **调试隐藏**：`debug*` 前缀命令仅 `debug_assertions` 时可见
- **Up/Down 导航**：`Ctrl+P`/`Ctrl+N` 或 `j`/`k`
- **Esc 关闭**：不执行任何操作

### 19.4 Reasoning Effort —— 推理力度

**文件：** `protocol/src/openai_models.rs:43-51`

```rust
pub enum ReasoningEffort {
    None,       // 无推理，最快速度
    Minimal,    // 最小推理
    Low,        // 低推理
    Medium,     // 中等推理（默认）
    High,       // 高推理
    XHigh,      // 极高推理，最深度思考
}
```

**Anthropic 映射** (`anthropic-client/src/translate/thinking.rs:16-40`)：

```
ReasoningEffort → Anthropic ThinkingConfig / OutputConfig:

None / None        → thinking: None, output_config: None
Minimal / Low      → thinking: { type: "adaptive" }, output_config: "low"
Medium             → thinking: { type: "adaptive" }, output_config: "medium"
High               → thinking: { type: "adaptive" }, output_config: "high"
XHigh              → thinking: { type: "adaptive" }, output_config: "xhigh"
```

- **adaptive 模式**：`budget_tokens` 始终为 `None`，模型自行决定思考长度
- Opus 4.7 优化了自适应思考，不需要手动设置预算

### 19.5 /fast —— 快速模式

```
/fast → ServiceTier::Fast
```

`/fast` 切换快速模式 — 使用 Opus 4.6 fast tier，推理速度更快但质量略低。在模型选择弹窗中也可切换。

### 19.6 ModelMigration —— 启动升级提示

**文件：** `tui/src/model_migration.rs:61`

当 Agere 升级后，某些模型可能被移除或重命名：

```
select_model_availability_nux():
  遍历 migration_target_models
  └─ 检查目标模型在 catalog 中可达
  └─ 检查目标模型未 exhausted（未被用户用完配额）
  └─ 弹出迁移 UI
     "你的默认模型 claude-sonnet-4-5 将被移除，
      推荐使用 claude-sonnet-4-6"
```

### 19.7 线程启动/恢复时的模型解析优先级

```
模型解析优先级（从高到低）：
1. 线程 rollout 持久化的 model（会话恢复时优先）
2. config.toml 的 model（启动时的用户选择）
3. model_catalog 默认值（catalog 中排序第一的可见模型）
```

### 19.8 完整实例追踪

**场景**：用户从 claude-sonnet-4-6 切换到 claude-opus-4-7

```
[0s] 用户输入 "/model" → 按 Enter
[10ms] CommandPopup 弹出，显示可用模型列表
        ├─ claude-sonnet-4-6 (当前)
        ├─ claude-opus-4-7
        └─ deepseek-v4-flash

[2s] 用户选择 claude-opus-4-7 → 按 Enter
[5ms] AppEvent::PersistModelSelection { model: "claude-opus-4-7", effort: Medium }
[10ms] ConfigEditsBuilder::set_model("claude-opus-4-7", Medium).apply()
        ├─ 写入 config.toml: model = "claude-opus-4-7"
        └─ info_message: "Model set to claude-opus-4-7"
[50ms] ★ 当前 Turn 继续使用 claude-sonnet-4-6（不中断）
        └─ Turn 完成 → TurnComplete 事件

[下次用户输入] → 新 Turn 开始
        ├─ 检测到模型切换 (claude-sonnet-4-6 → claude-opus-4-7)
        ├─ 注入 <model_switch> 开发者消息:
        │   "You were previously using claude-sonnet-4-6.
        │    The conversation history was generated by that model.
        │    You are now claude-opus-4-7."
        └─ 新模型看到完整历史 + 切换说明 → 继续对话
```

### 19.9 设计决策

**Q1: 为什么模型切换不中断当前 Turn？**

**A:** 当前 Turn 已经开始流式输出，中断会导致不完整的回复。切换在下个 Turn 生效更安全——用户看到完整回复后再切换，体验更平滑。

**Q2: 为什么需要 `<model_switch>` 注入？**

**A:** 新模型不知道之前的对话是谁生成的、用了什么能力。`<model_switch>` 告知历史背景（"这对话是 claude-sonnet-4-6 帮你处理的，你现在继续"），防止模型困惑于对话风格不一致。

**Q3: 为什么用 ConfigEditsBuilder 而不是直接写 config.toml？**

**A:** `ConfigEditsBuilder` 提供统一的配置修改接口：支持验证（检查模型名有效性）、回滚（修改失败不破坏配置）、通知（UI 更新）。直接写文件会绕过这些保护。

---

## 20. ModelClient 与多 API 适配

> **初学者理解**：ModelClient 是 Agent 与 LLM API 之间的"翻译层"。它处理两种传输协议（HTTP/WebSocket）、两种消息格式（Anthropic Messages / OpenAI Responses）、SSE 流解析、重试和超时。类比：ModelClient 是旅行社——你需要去"Anthropic"或"OpenAI"旅行，旅行社会帮你订票（选择传输协议）、翻译（格式转换）、处理延误（重试/超时）。

### 20.1 核心架构

```
ModelClientSession
        │
        ├── stream(prompt, model_info)
        │       │
        │       ▼
        │   match wire_api:
        │       ├── WireApi::Responses
        │       │   ├── WebSocket 预热 (generate=false)
        │       │   ├── stream_via_websocket(prompt)
        │       │   │   └─ 426 Upgrade Required → 回退 HTTP
        │       │   └── stream_via_http_responses_api(prompt)
        │       │
        │       └── WireApi::Anthropic
        │           ├── 消息转换: ResponseItems → Anthropic Messages 格式
        │           ├── 工具转换: ToolSpec → Anthropic Tool 定义
        │           ├── POST /v1/messages (含 thinking/output_config)
        │           └── process_anthropic_sse() 后台解析任务
        │
        └── 重试/超时策略

WebSocket 切换流程:

HTTP 请求 → 426 Upgrade Required → 永久切换到 WebSocket
WebSocket 连接超时 (15s) → 回退到 HTTP
WebSocket 成功建立 → 当前 session 内复用连接
```

### 20.2 ModelClientSession 管理

**文件：** `core/src/client.rs`

`ModelClientSession` 是 Turn 级别的 HTTP/WS 连接管理器。Turn 内复用同一连接以提高效率，减少重复握手开销。

```rust
impl ModelClientSession {
    async fn stream(&mut self, prompt, model_info) -> ResponseStream {
        // 1. 如果启用 WebSocket → 尝试预热连接
        // 2. 根据 wire_api 选择传输协议:
        //    Responses → HTTP POST /v1/responses 或 WebSocket
        //    Anthropic → HTTP POST /v1/messages (SSE)
        // 3. 发送请求 → 流式接收 → 包装为 ResponseStream
    }
}
```

### 20.3 Anthropic 消息格式转换

**文件：** `anthropic-client/src/translate/`

```
┌─ Responses API Items ─┐         ┌─ Anthropic Messages ─┐
│ ResponseInputItem[]    │   →     │ MessagesRequest       │
│  - Message(user)       │         │  - model              │
│  - Message(assistant)  │         │  - max_tokens         │
│  - FunctionCallOutput  │         │  - system: [TextBlock]│
│  - Reasoning           │         │  - messages: [Message]│
│  - ...                 │         │  - tools: [Tool]      │
│ System instructions    │         │  - thinking            │
│ ToolSpec[]             │         │  - output_config       │
└────────────────────────┘         └────────────────────────┘
```

**build_anthropic_request() 流程：**

```
① system prompt → Vec<TextBlock>
② messages → Vec<Message { role, content[] }>
③ tools → Vec<Tool { name, description, input_schema }>
④ thinking → ThinkingConfig { type: "adaptive" }
⑤ output_config → OutputConfig { type: "low"|"medium"|"high"|"xhigh" }
⑥ POST /v1/messages + anthropic-beta header
```

最终请求结构：
```json
{
  "model": "claude-sonnet-4-7",
  "max_tokens": 8192,
  "stream": true,
  "system": [{"type": "text", "text": "You are a helpful coding assistant..."}],
  "messages": [
    {"role": "user", "content": [{"type": "text", "text": "帮我加一个健康检查API"}]}
  ],
  "tools": [
    {"name": "read_file", "description": "Read a file", "input_schema": {...}},
    {"name": "write_file", "description": "Write a file", "input_schema": {...}}
  ]
}
```

### 20.4 SSE 流解析 (process_anthropic_sse)

**文件：** `anthropic-client/src/translate/response.rs`

在独立 tokio 任务中运行，逐行解析 SSE 事件：

| SSE 事件 | 转换结果 | 关键标志 |
|---------|---------|---------|
| `message_start` | 记录 response_id, server_model | 流开始 |
| `content_block_start { text }` | `OutputItemAdded(Message{role:"assistant",content:[text]})` | 文本块开始 |
| `content_block_start { tool_use }` | `OutputItemAdded(FunctionCall{name,call_id})` | 工具调用开始 |
| `content_block_start { thinking }` | `OutputItemAdded(Reasoning{...})` | 推理块开始 |
| `content_block_delta { text_delta }` | `OutputTextDelta(delta)` | ★ token 增量 |
| `content_block_delta { input_json_delta }` | `ToolCallInputDelta(json)` | ★ 工具参数流 |
| `content_block_delta { thinking_delta }` | `ReasoningContentDelta(delta)` | 推理增量 |
| `content_block_stop { text }` | `OutputItemDone(Message{完整文本})` | 文本块完成 |
| `content_block_stop { tool_use }` | `OutputItemDone(FunctionCall{完整参数JSON})` | 工具参数完成 |
| `message_delta { stop_reason: "tool_use" }` | `Completed{end_turn: Some(false)}` | ★ 模型要继续 |
| `message_delta { stop_reason: "end_turn" }` | `Completed{end_turn: Some(true)}` | ★ 模型结束 |
| `message_delta { stop_reason: "stop_sequence" }` | `Completed{end_turn: Some(true)}` | 达到停止序列 |
| `error` | `Err(mapped_error_code)` | 错误映射 |

**关键理解：** `end_turn: false` = "tool_use" → 模型调用工具，需要 follow-up。`end_turn: true` = 模型完成回复，Turn 可以结束。

### 20.5 Responses API 构建

**文件：** `core/src/client.rs:1105`

```rust
async fn stream_responses_api(&mut self, ...) -> Result<ResponseStream<'_>> {
    let request = build_responses_api_request(
        model, instructions, input, tools, reasoning, text_format,
    );
    // POST /v1/responses
    // 映射原始 API 流到 ResponseStream<ResponseEvent>
}
```

**stream_responses_websocket** (`core/src/client.rs:1202`)：
- 通过 `build_responses_request()` 构建请求
- 通过 `websocket_connection()` 管理 WS 连接（惰性打开，按需重连）
- 支持增量请求（仅发送自上次响应以来新的输入项）
- 服务器返回 `UPGRADE_REQUIRED` → 回退到 HTTP

### 20.6 WebSocket 预热机制

**文件：** `core/src/client.rs:1450-1500`

```
prewarm_websocket():
  1. WebSocket 已禁用或有待处理请求? → 跳过
  2. 发送 response.create (generate=false)
     └─ 建立 WebSocket 连接
     └─ 通过 previous_response_id 实现粘性路由
  3. 等待响应直到 Completed 事件
  4. 失败处理:
     ├─ 426 Upgrade Required → 禁用 WebSocket, 切换 HTTP
     └─ 其他错误 → 记录指标，下次请求重试
```

**监控指标** (`otel/src/metrics/names.rs:36-39`)：
- `agere.startup_prewarm.duration_ms` — 预热尝试持续时间
- `agere.startup_prewarm.age_at_first_turn_ms` — 预热到第一个真实 Turn 的年龄

### 20.7 重试/超时/回退策略

**来源：** `model-provider-info/src/lib.rs` 中的配置字段

| 策略 | 默认值 | 说明 |
|------|--------|------|
| HTTP 请求重试 | 4 次 | 网络错误时指数退避重试 |
| 流式重连 | 5 次 | 自动恢复已接收内容，从断点继续 |
| 流空闲超时 | 300s | 无新数据则断开连接 |
| WebSocket 连接超时 | 15s | 超时回退 HTTP |

**ContextWindowExceeded 特殊处理：**
- **压缩时**：移除最旧项目 → 重试压缩请求
- **主 Turn 时**：调用 `set_total_tokens_full()` → 标记窗口已满 → 触发自动压缩
- **历史只剩 1 项仍超出**：返回错误

### 20.8 完整实例追踪

**场景**：用户向 Anthropic API 发送消息并接收流式回复

```
1. Turn 开始 → ModelClientSession::stream(prompt, model_info)
   └─ wire_api == Anthropic
   └─ 启用 SSE 流式请求

2. build_anthropic_request():
   ├─ system: "You are a helpful coding assistant..."
   ├─ messages: [{role: "user", content: "帮我写代码"}]
   ├─ tools: [read_file, write_file, grep, bash, ...]
   └─ thinking: {type: "adaptive"}

3. POST https://api.anthropic.com/v1/messages
   Headers: anthropic-beta: prompt-caching-2024-07-31

4. tokio::spawn(process_anthropic_sse(bytes_stream)):
   └─ 逐行解析 SSE:
        event: message_start → 记录 msg_001
        event: content_block_start {text} → OutputItemAdded(Message{role:"assistant"...})
        event: content_block_delta {text_delta: "我来"} → OutputTextDelta("我来")
           → Session::send_event(AgentMessageContentDelta{delta:"我来"})
           → TUI 显示 "我来"
        event: content_block_delta {text_delta: "帮你"} → 流式显示 "我帮你"
        ... 继续流式输出 ...
        event: content_block_stop {text} → OutputItemDone(完整文本)
        event: message_delta {stop_reason:"end_turn"} → Completed{end_turn:true}
           → needs_follow_up = false → Turn 完成
```

### 20.9 设计决策

**Q1: 为什么需要 WebSocket 和 HTTP 双通道？**

**A:** WebSocket 提供低延迟、全双工通信（适合实时对话场景）。HTTP 更简单、兼容性更好（适合无 WS 支持的环境）。双通道确保任何网络环境都能工作——自动检测、自动切换。

**Q2: 为什么 SSE 解析放在独立 tokio 任务？**

**A:** 流式解析是 I/O 密集型操作。放在独立任务不阻塞主事件循环，也能正确响应 CancellationToken（取消信号）。任务结束时自动清理资源。

**Q3: 为什么 WebSocket 预热用 generate=false？**

**A:** 预热不是为了生成回复，而是为了建立连接和初始化缓存。`generate=false` 让 API 准备好但不生成 token，节省成本。第一次真实请求时连接已就绪，延迟更低。

**Q4: 为什么 Anthropic 消息格式需要单独的转换层？**

**A:** Agere 内部使用统一的 Responses API 格式（OpenAI 标准）。Anthropic API 使用不同的消息和工具格式。转换层屏蔽了两者的差异——上层代码只需要处理一种格式。

---

## 21. Tools 工具系统详解

> **初学者理解**：工具调用是 Agent 能力的核心。LLM 只能生成文本，但通过工具系统，Agent 可以让 LLM 决定"调用 read_file" → 实际读取文件 → 把结果告诉 LLM → LLM 基于真实数据继续推理。这个 **ReAct Loop**（Reason + Act）是现代 Agent 系统的标准模式。

### 21.1 工具生命周期：6 阶段总览

```
阶段A: 规划与注册          ── build_tool_registry_plan()           tools/src/tool_registry_plan.rs:74
阶段B: 实例化              ── build_specs_with_discoverable_tools() core/src/tools/spec.rs:71
阶段C: 路由器构建           ── ToolRouter::from_config()           core/src/tools/router.rs:55
阶段D: ToolCall 提取        ── ToolRouter::build_tool_call()       core/src/tools/router.rs:175
阶段E: 并行调度             ── ToolCallRuntime::handle_tool_call() core/src/tools/parallel.rs:64
阶段F: 分发与执行           ── ToolRegistry::dispatch_any()        core/src/tools/registry.rs:263
```

### 21.2 核心数据结构

**ToolPayload** — 6 种工具来源 (`core/src/tools/context.rs:58-77`)：

```rust
pub enum ToolPayload {
    Function { arguments: String },                          // 模型生成的函数调用
    ToolSearch { arguments: SearchToolCallParams },          // 延迟工具搜索
    Custom { input: String },                                // 自定义工具
    LocalShell { params: ShellToolCallParams },              // 本地 Shell
    Mcp { server: String, tool: String, raw_arguments: String }, // MCP 工具
}
```

**ToolCall** — 解析后的工具调用 (`core/src/tools/router.rs:32-36`)：

```rust
pub struct ToolCall {
    pub tool_name: ToolName,   // 工具名称（含命名空间）
    pub call_id: String,       // 调用标识符
    pub payload: ToolPayload,  // 载荷
}
```

**ToolInvocation** — 运行时执行上下文 (`core/src/tools/context.rs:47-56`)：

```rust
pub struct ToolInvocation {
    pub session: Arc<Session>,
    pub turn: Arc<TurnContext>,
    pub cancellation_token: CancellationToken,
    pub tracker: SharedTurnDiffTracker,
    pub call_id: String,
    pub tool_name: ToolName,
    pub source: ToolCallSource,    // Direct(模型) 或 CodeMode(嵌套)
    pub payload: ToolPayload,
}
```

### 21.3 阶段 A-B — 规划与实例化

**build_tool_registry_plan()** (`tools/src/tool_registry_plan.rs:74`)：

```
接收: MCP工具 / 延迟工具 / 动态工具 / 发现工具
遍历所有可能的工具类型:
  shell / code_mode / apply_patch / plan / goal / search /
  MCP namespaces / multi_agent / RgHandler / ListDir / ...
    ├─ plan.push_spec(spec, parallel_support, code_mode)
    │   └─ 添加到 ToolSpec 列表
    └─ plan.register_handler(name, ToolHandlerKind)
        └─ 注册 handler 名称 → 种类映射
```

**build_specs_with_discoverable_tools()** (`core/src/tools/spec.rs:71-343`)：

- 调用 `build_tool_registry_plan()` 获取计划
- 为每种 `ToolHandlerKind` 创建 `Arc<Handler>` 单例
- `ToolRegistryBuilder::build()` → `(Vec<ConfiguredToolSpec>, ToolRegistry)`

### 21.4 阶段 C — 路由器构建

**ToolRouter::from_config()** (`core/src/tools/router.rs:55-100`)：

```
参数:
  - mcp_tools: Option<HashMap<String, ToolInfo>>        // 已连接的 MCP 工具
  - deferred_mcp_tools: Option<HashMap<String, ToolInfo>> // 延迟加载的 MCP 工具
  - unavailable_called_tools: Vec<ToolName>              // 之前尝试但不可用的工具
  - parallel_mcp_server_names: HashSet<String>           // 支持并行的服务器
  - dynamic_tools: &[DynamicToolSpec]                   // 运行时注册的工具

存储:
  - registry: ToolRegistry              ← handler 名称 → 处理器映射
  - specs: Vec<ConfiguredToolSpec>      ← 所有工具规范
  - model_visible_specs: Vec<ToolSpec>  ← 模型可见规范（不含延迟工具）
  - parallel_mcp_server_names           ← 并行服务器名单
```

### 21.5 阶段 D — ToolCall 提取

**ToolRouter::build_tool_call()** (`core/src/tools/router.rs:175-265`)：

```
ResponseItem::FunctionCall { name, namespace, arguments, call_id }:
  1. ToolName::new(namespace, name)
  2. session.resolve_mcp_tool_info(&tool_name)
     ├─ 匹配到 MCP 工具 → ToolPayload::Mcp { server, tool, raw_arguments }
     └─ 未匹配 → ToolPayload::Function { arguments }

ResponseItem::ToolSearchCall → ToolPayload::ToolSearch
ResponseItem::CustomToolCall { name, input } → ToolPayload::Custom
ResponseItem::LocalShellCall → ToolPayload::LocalShell
```

### 21.6 阶段 E — 并行调度

**ToolCallRuntime** (`core/src/tools/parallel.rs:28-197`)：

```rust
// 核心并行控制
let _guard = if supports_parallel {
    Either::Left(lock.read().await)    // 共享读锁 → 多工具并发执行
} else {
    Either::Right(lock.write().await)  // 独占写锁 → 串行排队执行
};
```

**并行策略表：**

| 工具 | 并行支持 | 原因 |
|------|---------|------|
| read_file / rg / list_dir / tool_search | 是 | 只读操作，无副作用 |
| view_image / test_sync_tool | 是 | 无状态修改 |
| apply_patch / plan / goal / code_mode | 否 | 写操作或状态改变 |
| multi_agent (spawn/send/wait/close) | 否 | 代理间通信需要顺序一致性 |
| MCP 工具 | 取决于配置 | `parallel_mcp_server_names` 决定 |

### 21.7 阶段 F — 分发与执行（10 步生命周期）

**ToolRegistry::dispatch_any()** (`core/src/tools/registry.rs:263-501`)：

```
1. 活跃 Turn 计数: turn_state.tool_calls += 1
2. Handler 查找: self.handler(&tool_name) → None? → 不可用工具错误
3. Kind 匹配: handler.matches_kind(payload) → 不匹配? → 错误
4. ★ PreToolUse 钩子: run_pre_tool_use_hooks() → 可能阻止
5. ★ 工具门: is_mutating()? → 等待 tool_call_gate (串行工具排队)
6. ★ Handler 执行: handler.handle_any(invocation) → 实际工具调用
7. OTEL 指标: otel.log_tool_result_with_tags() → 遥测记录
8. ★ PostToolUse 钩子: run_post_tool_use_hooks() → 附加上下文/停止
9. Legacy AfterToolUse: dispatch_after_tool_use_hook()
10. Goal 通知: goal_runtime_apply(ToolCompleted)
```

### 21.8 ToolHandler trait

**文件：** `core/src/tools/registry.rs:42-90`

```rust
pub trait ToolHandler: Send + Sync {
    type Output: ToolOutput + 'static;
    fn kind(&self) -> ToolKind;                    // Function | Mcp
    fn matches_kind(&self, payload: &ToolPayload) -> bool;
    fn is_mutating(&self, invocation: &ToolInvocation) -> impl Future<Output = bool>;
    fn pre_tool_use_payload(...) -> Option<PreToolUsePayload>;
    fn post_tool_use_payload(...) -> Option<PostToolUsePayload>;
    fn create_diff_consumer(...) -> Option<Box<dyn ToolArgumentDiffConsumer>>;
    fn handle(&self, invocation: ToolInvocation) -> impl Future<Output = Result<Self::Output, FunctionCallError>>;
}
```

### 21.9 全部内置 Handler 清单

| Handler | Kind | 文件 |
|---------|------|------|
| McpHandler | Mcp | `core/src/tools/handlers/mcp.rs:18` |
| ShellHandler | Function | `core/src/tools/handlers/shell.rs` |
| UnifiedExecHandler | Function | `core/src/tools/handlers/unified_exec.rs` |
| ApplyPatchHandler | Function | `core/src/tools/handlers/apply_patch.rs` |
| PlanHandler | Function | `core/src/tools/handlers/plan.rs` |
| GoalHandler | Function | `core/src/tools/handlers/goal.rs` |
| ToolSearchHandler | Function | `core/src/tools/handlers/tool_search.rs` |
| RgHandler | Function | `core/src/tools/handlers/rg.rs` |
| ListDirHandler | Function | `core/src/tools/handlers/list_dir.rs` |
| ViewImageHandler | Function | `core/src/tools/handlers/view_image.rs` |
| RequestUserInputHandler | Function | `core/src/tools/handlers/request_user_input.rs` |
| DynamicToolHandler | Function | `core/src/tools/handlers/dynamic.rs` |
| CodeModeExecuteHandler | Function | `core/src/tools/code_mode/execute_handler.rs` |
| SpawnAgentHandlerV2 | Function | `core/src/tools/handlers/multi_agents_v2/spawn.rs` |
| SendMessageHandlerV2 | Function | `core/src/tools/handlers/multi_agents_v2/send_message.rs` |
| WaitAgentHandlerV2 | Function | `core/src/tools/handlers/multi_agents_v2/wait.rs` |
| CloseAgentHandlerV2 | Function | `core/src/tools/handlers/multi_agents_v2/close_agent.rs` |
| ListAgentsHandlerV2 | Function | `core/src/tools/handlers/multi_agents_v2/list_agents.rs` |

### 21.10 ToolOrchestrator 审批 + 重试

**文件：** `core/src/tools/orchestrator.rs:34-383`

```
ToolOrchestrator::run():
  1. 审批决定: tool.exec_approval_requirement()
     ├─ Skip → 直接执行
     ├─ NeedsApproval → 请求审批
     └─ Forbidden → 阻止执行

  2. 首次尝试: run_attempt()
     ├─ begin_network_approval()
     ├─ tool.run(req, ctx) → 执行工具
     └─ finish_immediate_network_approval()

  3. 拒绝重试 (仅当 escalate_on_failure):
     └─ 提升权限 → 重新请求审批 → run_attempt()

  4. 审批请求: request_approval()
     ├─ 检查 PermissionRequest 钩子
     ├─ Guardian 自动审查（如适用）
     └─ 用户审批 UI
```

### 21.11 完整工具调用实例追踪

**场景**：LLM 决定创建 `health.rs` 文件

```
① LLM 决定调用工具:
   SSE: content_block_start {tool_use, id:"toolu_003", name:"write_file"}
   SSE: input_json_delta: {"path":"src/api/health.rs","content":"..."}
   SSE: content_block_stop {tool_use}
   SSE: message_delta {stop_reason:"tool_use"} → Completed{end_turn:false}

② handle_output_item_done() 提取:
   FunctionCall { name: "write_file", call_id: "toolu_003",
     arguments: "{\"path\":\"src/api/health.rs\",\"content\":\"use axum::...\"}" }

③ build_tool_call():
   ToolName::new(None, "write_file")
   resolve_mcp_tool_info() → None（不是 MCP 工具）
   → ToolCall { tool_name: "write_file", call_id: "toolu_003",
        payload: ToolPayload::Function { arguments: "{\"path\":...}" } }

④ handle_tool_call():
   supports_parallel("write_file")? → false → 获取写锁(write lock)
   tokio::spawn(async { dispatch_tool_call_with_code_mode_result(call) })

⑤ dispatch_any():
   1. turn_state.tool_calls += 1
   2. handler(&"write_file") → ApplyPatchHandler
   3. matches_kind(ApplyPatch, Function) → true
   4. run_pre_tool_use_hooks() → (如果用户配了 pre-tool-use 钩子)
   5. is_mutating()? → true → 等待 tool_call_gate
   6. ApplyPatchHandler::handle(invocation) → 解析补丁 → 应用文件
      └─ ToolOrchestrator::run() → 审批检查
        ├─ 写操作 → ExecApprovalRequirement::NeedsApproval
        ├─ Guardian 审查 → Allow/Deny
        └─ 用户确认 → Approved → 执行写入
   7. OTEL 记录指标
   8. run_post_tool_use_hooks() → (如果配了 post-tool-use 钩子)
   9. dispatch_after_tool_use_hook()
   10. goal_runtime_apply(ToolCompleted)

⑥ 结果回写:
   record_conversation_items([FunctionCallOutput {
       call_id: "toolu_003",
       output: "File written: src/api/health.rs (512 bytes)"
   }])
   → 下次 LLM 调用能看到这次写入的结果

⑦ needs_follow_up = true → 继续内层循环 → LLM 调用 #N+1
```

### 21.12 设计决策

**Q1: 为什么分 Plan 和 Runtime 两层？**

**A:** Plan 层是纯数据——哪些工具可用、如何分组、可见性规则。Runtime 层依赖 Session/Turn——执行时需要会话上下文。分层让 Plan 可独立测试，也允许同一个 Plan 被不同 runtime 复用。

**Q2: 为什么用 RwLock 而不是 Semaphore 控制并行？**

**A:** RwLock 的读写语义更清晰——只读工具共享读锁（多并发），写工具获取写锁（串行排队）。Semaphore 无法区分读/写，需要额外逻辑。

**Q3: 为什么 ToolHandler 用 trait 而不是 enum dispatch？**

**A:** Trait 允许在 crate 外部添加新 handler（开放扩展）。Enum 需要修改核心枚举定义（封闭）。选择 trait 是面向未来——新工具不需要修改核心代码。

---

## 22. MCP 服务器管理

> **初学者理解**：MCP (Model Context Protocol) 是 Anthropic 推出的开放协议，让 Agent 能连接外部服务。类比：MCP 就像 Agent 的"USB-C 接口"——任何支持 MCP 的服务（文件系统、GitHub、Slack）都能即插即用，无需 Agent 为每个服务编写专用适配器。

### 22.1 MCP 配置格式

**文件：** `config/src/mcp_types.rs:117-177`

**McpServerConfig 字段表：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `transport` | `McpServerTransportConfig` | 传输方式 (Stdio / StreamableHttp) |
| `enabled` | `bool` | 默认 true |
| `required` | `bool` | true 时启动失败会停止会话 |
| `supports_parallel_tool_calls` | `bool` | 是否允许并行工具调用 |
| `startup_timeout_sec` | `Option<Duration>` | 初始化和首次工具列表超时 |
| `tool_timeout_sec` | `Option<Duration>` | 每次工具调用超时 |
| `default_tools_approval_mode` | `Option<AppToolApproval>` | "auto" / "prompt" / "approve" |
| `enabled_tools` | `Option<Vec<String>>` | 允许列表 |
| `disabled_tools` | `Option<Vec<String>>` | 拒绝列表 |
| `scopes` | `Option<Vec<String>>` | OAuth 作用域 |
| `oauth_resource` | `Option<String>` | RFC 8707 资源参数 |

**传输配置** (`config/src/mcp_types.rs:362-392`)：

```rust
pub enum McpServerTransportConfig {
    Stdio {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        cwd: Option<PathBuf>,
    },
    StreamableHttp {
        url: String,
        bearer_token_env_var: Option<String>,
        http_headers: HashMap<String, String>,
    },
}
```

**配置示例：**

```toml
[mcp_servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
disabled_tools = ["write_file"]
supports_parallel_tool_calls = true

[mcp_servers.github-mcp]
url = "https://api.githubcopilot.com/mcp/"
bearer_token_env_var = "GITHUB_TOKEN"
```

### 22.2 连接生命周期

**文件：** `agere-mcp/src/rmcp_client.rs:135-214`, `agere-mcp/src/connection_manager.rs:132-268`

```
Session 初始化
  │
  ├─ McpConnectionManager::new_uninitialized() → 空占位
  │
  ├─ McpConnectionManager::new(servers, auth_statuses, ...)
  │   └─ 遍历每个 enabled 服务器
  │       ├─ 发送 McpStartupUpdateEvent { status: Starting }
  │       ├─ 创建 CancellationToken
  │       └─ 创建 AsyncManagedClient
  │           ├─ 构建传输:
  │           │   Stdio: LocalStdioServerLauncher 生成子进程
  │           │   StreamableHttp: 创建 HTTP 客户端 + bearer token
  │           │
  │           └─ start_server_task():
  │               ├─ 发送 InitializeRequestParams
  │               │   协议版本: 2025-06-18
  │               │   客户端名: "agere-mcp-client"
  │               │   能力: ElicitationCapability
  │               │
  │               ├─ list_tools_for_client_uncached()
  │               │   └─ RmcpClient::list_tools() (调用 tools/list 端点)
  │               │      └─ 每个工具 → ToolInfo { server_name, callable_name, tool }
  │               │
  │               └─ 返回 ManagedClient { cached_tools, rmcp_client }
  │
  └─ 后台聚合器任务 → 等待所有服务器启动 → McpStartupCompleteEvent
```

**启动快照优化**：对于 agere_apps 服务器，如果磁盘缓存存在，启动快照可立即提供工具——在完全初始化之前就能响应 `list_all_tools()`。

**重连/刷新** (`core/src/session/mcp.rs:206-262`)：
- `refresh_mcp_servers_inner()` 创建新的 `McpConnectionManager`
- 用 `std::mem::replace` 原子交换
- 调用 `old_manager.shutdown()` 清理旧连接

**关闭**：`begin_shutdown()` → 取消所有令牌 → 获取所有客户端 → 依次 clean shutdown

### 22.3 工具发现流程

```
阶段1: 服务器端发现
  └─ start_server_task() 调用 tools/list MCP 端点 → ToolInfo 列表

阶段2: 工具限定 (qualify_tools)
  └─ agere-mcp/src/tools.rs:138-229
     ├─ 原始名称去重 (server + namespace + connector + tool name)
     ├─ 名称净化: sanitize_responses_api_tool_name()
     │   └─ 所有非 [a-zA-Z0-9_-] 字符替换为 _
     ├─ 命名空间冲突: 不同服务器用相同名称 → SHA1 哈希后缀
     └─ 名称长度限制: 限定键不超过 64 字节

阶段3: 注册到 ToolRegistryPlan
  └─ tools/src/tool_registry_plan.rs:504-564
     ├─ 按命名空间分组 (mcp__filesystem__)
     ├─ mcp_tool_to_responses_api_tool() 转换为 Responses API 格式
     └─ 注册为 ToolSpec::Namespace

阶段4: 运行时 handler 绑定
  └─ core/src/tools/spec.rs:233
     └─ ToolHandlerKind::Mcp → McpHandler 实例
```

### 22.4 OAuth 认证

**文件：** `agere-mcp/src/mcp/auth.rs`

```
compute_auth_statuses():
  遍历每个 enabled 服务器
  ├─ agere_apps 内置 → 运行时 auth (BearerToken)
  ├─ Stdio 传输 → Unsupported
  └─ StreamableHttp → determine_streamable_http_auth_status()

oauth_login_support():
  探测端点发现 OAuth 支持
  返回: Supported(McpOAuthLoginConfig) / Unsupported / Unknown

resolve_oauth_scopes():
  优先级: 显式作用域 > 配置的作用域 > 发现的作用域 > 空
  重试逻辑: should_retry_without_scopes() — 服务器拒绝时回退
```

### 22.5 MCP 工具调用路径

```
McpHandler::handle()                         core/src/tools/handlers/mcp.rs:18
  └─ handle_mcp_tool_call()                  core/src/session/mcp_tool_call.rs:87
      ├─ parse arguments JSON
      ├─ lookup_mcp_tool_metadata()          — 获取注解、连接器信息
      ├─ maybe_request_mcp_tool_approval():
      │   ├─ auto_approved_by_policy? → 跳过
      │   ├─ 检查会话/持久审批记忆
      │   ├─ 运行审批钩子
      │   └─ Guardian 审查或 RequestUserInput
      │
      └─ handle_approved_mcp_tool_call():
          ├─ rewrite_mcp_tool_arguments_for_openai_files()
          │   └─ 处理 fileParams 元数据，将文件路径转 URL
          │
          └─ execute_mcp_tool_call():
              └─ sess.call_tool(server, tool, arguments, meta)
                  └─ McpConnectionManager::call_tool()
                      └─ RmcpClient::call_tool()  ← 调用后端
```

### 22.6 MCP 工具 vs 内置工具对比

| 方面 | 常规函数工具 | MCP 工具 |
|------|-------------|---------|
| 工具定义源 | 硬编码在 Rust 中 | 启动时从服务器动态发现 |
| 注册方式 | `ToolSpec::Function`（扁平） | `ToolSpec::Namespace`（分层） |
| 命名规则 | 简单单部分名称 | `mcp__{server}__{tool}` 模式 |
| Handler 种类 | 每个工具种类唯一 handler | 共享 `ToolHandlerKind::Mcp` |
| 审批方式 | 各 handler 特定逻辑 | 统一策略（注解驱动：destructive_hint 等） |
| 参数格式 | 结构化 Rust 类型 | 不透明 JSON 字符串 |
| 输出格式 | 结构化 `ToolOutput` | `McpToolOutput` (CallToolResult) |
| 运行时位置 | 进程内/本地执行 | 调用 MCP 服务器（子进程或 HTTP） |
| 连接器元数据 | 不适用 | 保留 connector_id/name（agere_apps） |

### 22.7 完整 MCP 调用实例追踪

**场景**：LLM 决定调用 github-mcp 服务器创建 issue

```
① LLM 返回工具调用:
   FunctionCall { namespace: "mcp__github-mcp__", name: "create_issue",
     arguments: "{\"repo\":\"org/repo\",\"title\":\"Bug report\"}" }

② build_tool_call():
   ToolName::new("mcp__github-mcp__", "create_issue")
   resolve_mcp_tool_info("mcp__github-mcp__create_issue")
     ├─ connection_manager 查找 → 匹配到 github-mcp 服务器
     └─ → ToolPayload::Mcp { server: "github-mcp", tool: "create_issue",
            raw_arguments: "{\"repo\":\"org/repo\",...}" }

③ handle_tool_call():
   supports_parallel("github-mcp")? → 取决于服务器配置
   tokio::spawn(async { dispatch_tool_call_with_code_mode_result(call) })

④ McpHandler::handle() → handle_mcp_tool_call():
   parse: {"repo": "org/repo", "title": "Bug report"}
   metadata: {"destructive_hint": true, "open_world_hint": true}
   maybe_request_mcp_tool_approval():
     ├─ default_tools_approval_mode = "prompt"
     └─ → 请求用户审批

⑤ 用户确认 → handle_approved_mcp_tool_call():
   rewrite_mcp_tool_arguments_for_openai_files() → (无文件参数)
   execute_mcp_tool_call():
     └─ sess.call_tool("github-mcp", "create_issue", {...}, meta)
        └─ McpConnectionManager::call_tool()
           └─ RmcpClient::call_tool()
              └─ POST https://api.githubcopilot.com/mcp/
                 { method: "tools/call", params: { name: "create_issue", ... } }
              └─ 返回: CallToolResult { content: [Text("Issue #42 created")] }

⑥ 结果回写:
   McpToolOutput { call_tool_result: {...}, wall_time: 1234ms }
   → to_response_item() → ResponseInputItem::McpToolCallOutput
   → 记录到对话历史
```

### 22.8 设计决策

**Q1: 为什么 MCP 工具不放在常规 ToolRegistry 中？**

**A:** MCP 工具是动态的——连接前不知道有哪些工具。将 MCP 工具与静态工具分开处理，允许独立管理连接生命周期、审批策略和超时。

**Q2: 为什么 qualify_tools 要用 SHA1 哈希解决命名冲突？**

**A:** 不同 MCP 服务器可能有同名工具（如两个文件系统服务器都有 "read_file"）。SHA1 哈希后缀保证唯一性，同时保持名称的人类可读性。

**Q3: 为什么 MCP 工具参数是不透明 JSON 字符串？**

**A:** MCP 工具的参数 schema 由外部服务器定义，Agere 不了解其结构。作为字符串传递保持了通用性——McpHandler 不需要了解每个工具的参数格式。

---

## 23. Hook 系统

> **初学者理解**：Hook 是 Agent 执行生命周期中的"检查点"。类比餐厅：点餐前确认口味（PreToolUse）、上菜后评价（PostToolUse）、结账时推荐下次来（Stop）。Hook 让外部代码（shell 命令）能在 Agent 的关键时刻介入——检查、修改、阻止。

### 23.1 6 种 Hook 事件总表

**文件：** `hooks/src/lib.rs:9-28`

| 事件 | 触发时机 | 作用域 | 匹配器 |
|------|---------|-------|--------|
| `SessionStart` | 会话启动（启动/恢复/清除） | Thread | 是（source: startup/resume/clear） |
| `UserPromptSubmit` | 用户提示提交前 | Turn | 否（总是全部运行） |
| `PreToolUse` | 工具执行前 | Turn | 是（按工具名称/别名） |
| `PermissionRequest` | 审批路径中 | Turn | 是（按工具名称） |
| `PostToolUse` | 工具成功执行后 | Turn | 是（按工具名称） |
| `Stop` | Turn 完成/停止 | Turn | 否（总是全部运行） |

### 23.2 链路总图

```
工具调用 → PreToolUse → 审批 → 执行 → PostToolUse
用户输入 → UserPromptSubmit → Turn 开始
Turn 结束 → Stop → 后处理
会话启动 → SessionStart → 初始化
```

### 23.3 配置方式

**优先级（从低到高）：**

```
管理需求 (ManagedHooksRequirementsToml)
  └─ hooks.json (配置文件夹中的文件)
  └─ config.toml [hooks] 表 (TOML 内联钩子)
  └─ 插件钩子源 (PluginHookSource → PLUGIN_ROOT + PLUGIN_DATA 环境变量)
```

**hooks.json 配置示例：**

```json
{
  "PreToolUse": [
    {
      "matcher": "Bash",
      "hooks": [
        {
          "type": "command",
          "command": "echo 'Bash command about to run' >> /tmp/agere.log",
          "timeout": 5000
        }
      ]
    }
  ],
  "Stop": [
    {
      "hooks": [
        {
          "type": "command",
          "command": "notify-send 'Agere turn completed'"
        }
      ]
    }
  ]
}
```

**config.toml [hooks] 配置示例：**

```toml
[hooks.PreToolUse.Bash]
command = "cat >> /tmp/bash_log.txt"
timeout_sec = 10

[hooks.UserPromptSubmit]
command = "echo 'User prompt submitted' | tee -a session.log"
```

### 23.4 HookToolName 别名匹配

**文件：** `core/src/tools/hook_names.rs:14-55`

```
apply_patch → 规范名称 "apply_patch"，别名 ["Write", "Edit"]
Bash        → 规范名称 "Bash"，无别名
```

匹配器在调度期间检查规范名称和所有别名——对 Write 或 Edit 配置的钩子也会匹配 apply_patch。

### 23.5 发现→调度→执行流程

**discover_handlers()** (`hooks/src/engine/discovery.rs:38-126`)：

```
遍历配置层（最低优先级→最高）
  ├─ hooks.json → 解析为 HooksFile
  │   └─ 每个事件 → MatcherGroup → Vec<HookHandlerConfig>
  └─ config.toml [hooks] → 解析为 HookEventsToml
      └─ 每个事件 → HookHandlerConfig

append_hook_events() → ConfiguredHandler 平面列表
```

**execute_handlers()** (`hooks/src/engine/dispatcher.rs:83-103`)：

```
select_handlers_for_matcher_inputs():
  ├─ UserPromptSubmit / Stop → 忽略匹配器，选择所有 handler
  └─ 其他事件 → 按事件名称和匹配器过滤

并发执行所有匹配的 Command handler
  └─ 每个 handler:
      ├─ 设置超时定时器
      ├─ 运行 shell 命令
      ├─ 解析 stdout/stderr → Outcome 结构体
      └─ 发射 HookStarted / HookCompleted 事件
```

### 23.6 每种 Hook 的详细运行时

**PreToolUse** (`core/src/hook_runtime.rs:137-182`)：
- 在工具执行**前**运行
- `PreToolUseOutcome.should_block` — true 则阻止工具执行
- 阻止时显示来自钩子的原因消息

**PermissionRequest** (`core/src/hook_runtime.rs:187-215`)：
- 在审批 UI 或 Guardian **之前**运行
- 返回 `Option<PermissionRequestDecision>`:
  - `Some(Allow)` → 自动允许（绕过 Guardian/用户）
  - `Some(Deny { message })` → 自动拒绝
  - `None` → 让正常审批流程继续

**PostToolUse** (`core/src/hook_runtime.rs:223-251`)：
- 在工具成功完成**后**运行
- 返回 `additional_contexts`（注入回会话）、`feedback_message`（给模型）、`should_stop`（停止执行）

**Stop**：
- Turn 完成后运行
- 返回 `continuation_fragments`（动态添加到代理提示）

### 23.7 HookStarted/HookCompleted 事件

每个运行的 hook 产生两个 EventMsg：
```rust
EventMsg::HookStarted { run_id, name, status: "pending" }
EventMsg::HookCompleted { run_id, name, status: "succeeded"|"failed"|"timed_out", duration }
```

### 23.8 设计决策

**Q1: 为什么 Hook 用 shell 命令而不是函数调用？**

**A:** 无需编译——任何脚本语言（Python、Node.js、bash）都能直接用。用户可以用熟悉的工具编写 Hook，不需要学习 Rust。

**Q2: 为什么 UserPromptSubmit 和 Stop 没有匹配器？**

**A:** 它们需要全局生效——不管是哪个工具调用，用户提示提交和 Turn 完成都需要触发。如果加了匹配器会导致部分场景遗漏。

**Q3: 为什么 Prompt 和 Agent 类型的 Hook 未实现？**

**A:** `Prompt` 钩子需要注入额外的对话到 Agent，`Agent` 钩子需要启动子代理——两者的实现复杂度远高于 Command 钩子，当前版本只实现了 Command 类型，其余类型发出警告。

---

## 24. 审批与安全体系

> **初学者理解**：审批是 Agent 的"安全门"。类比公司审批链：先查规章制度（ExecPolicy）→ 部门审核（PreToolUse 钩子）→ 合规检查（PermissionRequest 钩子）→ 领导自动审批（Guardian）→ 最后到老板签字（用户 UI）。每一层都可能放行或拦截——安全优先。

### 24.1 6 步审批链路总图

```
工具调用（如 write_file, bash）
  │
  ▼
步骤1: ExecPolicyManager 评估命令规则
  ├── Forbidden → 阻止执行（返回错误给模型）
  ├── NeedsApproval → 继续审批链路
  └── Skip → 直接执行（已知安全命令）
  │
  ▼
步骤2: PreToolUse 钩子
  └── should_block? → 阻止（返回原因给模型）
  │
  ▼
步骤3: PermissionRequest 钩子
  ├── Allow → 自动允许（绕过后续步骤）
  ├── Deny { message } → 自动拒绝
  └── None → 继续正常流程
  │
  ▼
步骤4: Guardian 自动审查
  ├── Allow → 允许执行
  ├── Deny → 阻止（断路器记录）
  └── 超时/错误 → 失败关闭 = Deny
  │
  ▼
步骤5: 用户审批 UI (ApprovalOverlay)
  ├── Approved → 单次允许
  ├── ApprovedForSession → 缓存（不再重复询问）
  └── Abort → 取消
  │
  ▼
步骤6: PostToolUse 钩子（成功后运行）
  └── additional_contexts / feedback_message / should_stop
```

### 24.2 ExecPolicyManager 命令规则

**文件：** `core/src/exec_policy.rs:235-317`

**ExecApprovalRequirement 枚举** (`core/src/tools/execution.rs:150-167`)：

```rust
pub enum ExecApprovalRequirement {
    Forbidden,          // 禁止执行
    NeedsApproval,      // 需要审批
    Skip,               // 跳过审批，直接执行
}
```

**AskForApproval 策略矩阵：**

| 策略 | 安全命令 | 未知命令 | 危险命令 |
|------|---------|---------|---------|
| `Never` | 允许 | 禁止 | 禁止 |
| `OnFailure` | 允许（失败重试时提示） | 允许（失败重试时提示） | 禁止 |
| `OnRequest` | 允许 | 文件系统不受限→允许 否则→提示 | 禁止 |
| `UnlessTrusted` | 允许 | 提示 | 禁止 |
| `Granular` | 允许 | 提示 + 访问审批检查组 | 禁止 |

**规则文件（.rules）：** 从配置层加载，定义已知安全/危险命令清单。

### 24.3 Guardian 自动审查

**文件：** `core/src/guardian/mod.rs`, `core/src/guardian/review.rs:144-149`

Guardian 是仅在这些条件下运行的**自动 AI 审查员**：
1. `turn.approval_policy` 是 `OnRequest` 或 `Granular`
2. `turn.config.approvals_reviewer == ApprovalsReviewer::AutoReview`
3. 不是用户自己的 shell 命令

**审查参数：**

| 参数 | 值 | 说明 |
|------|-----|------|
| 超时 | 90 秒 | 防止审查过长 |
| 连续拒绝限制 | 3 次/Turn | 达到后中断 Turn |
| 总拒绝限制 | 10 次/Turn | 断路器保护 |
| 消息转录 | 最大 10,000 tokens | 用于审查的上下文量 |
| 工具条目 | 最大 1,000 tokens/条 | 每个工具结果的上限 |

**审查输出（结构化 JSON）：**

```json
{
  "outcome": "allow" | "deny",
  "risk_level": "low" | "medium" | "high" | "critical",
  "rationale": "This command reads a file in the project directory without modifying anything",
  "user_authorization": "unknown" | "low" | "medium" | "high"
}
```

**失败关闭原则**：超时、会话失败、解析错误都导致 `Deny`——宁可错误拒绝也不错误允许。

**审查会话配置**：Guardian 审查会话以只读权限运行（`AskForApproval::Never` + 禁用 AgereHooks / Apps / Plugins / Collab 等功能），使用专门的 `guardian_policy_prompt` 作为基础指令。

**支持的审批请求类型**：Shell / ExecCommand / Execve / ApplyPatch / NetworkAccess / McpToolCall / RequestPermissions

### 24.4 用户审批 UI 完整数据流

**文件：** `core/src/hook_runtime.rs`, `tui/src/bottom_pane/approval_overlay.rs`

```
① Session 发送审批请求:

ExecApprovalRequestEvent {
    call_id: "toolu_003",
    command: ["write_file", "src/api/health.rs"],
    cwd: "/home/user/myproject",
    reason: "创建新文件 src/api/health.rs",
}
  └─ ★ oneshot::channel() → pending_approvals[call_id] = tx_approve
  └─ ★ 工具执行线程在 rx_approve 上阻塞等待

② AppServer → TUI:

session.send_event(ExecApprovalRequest) → tx_event
  → AgereMessageProcessor 收到 Event
  → apply_bespoke_event_handling()
  → ServerRequest::CommandExecutionRequestApproval
  → OutgoingMessageSender.send_request()
  → InProcessServerEvent::ServerRequest

③ TUI 渲染审批对话框:

┌──────────────────────────────────────┐
│ ⚠ Agere wants to run a command      │
│                                      │
│ Command: write_file src/api/health.rs│
│ Reason:  创建新文件                  │
│ CWD:     /home/user/myproject        │
│ ──────────────────────────────────── │
│ > [y] Yes, proceed                  │
│   [a] Yes, and don't ask again      │
│   [n] No, and tell Agere what to do │
└──────────────────────────────────────┘

④ 用户按键 y:

handle_exec_decision("toolu_003", Approved)
  → AppEvent::SubmitThreadOp { op: Op::ExecApproval { call_id: "toolu_003", Approved } }
  → AppServer → MessageProcessor → agere.submit(Op::ExecApproval)
  → submission_loop → exec_approval()
  → notify_approval("toolu_003", Approved)

⑤ 解除阻塞:

tx_approve.send(Approved)  → ★ 工具执行线程恢复!
  → 执行文件写入 → 结果回写对话历史
```

### 24.5 ApprovalStore 会话缓存

**文件：** `core/src/tools/execution.rs:60-106`

```rust
with_cached_approval(approval_keys, call_id, async move {
    // 如果 call_id 在 approval_store 中 → 直接执行
    // 否则 → 请求审批 → 用户选择 "ApprovedForSession" → 缓存
})
```

`ApprovedForSession`：用户选择"don't ask again"，决定被缓存到 `ApprovalStore`，同一会话内的相同操作不再提示。

### 24.6 完整审批实例追踪

**场景**：LLM 决定执行 npm install lodash

```
① ExecPolicyManager 评估:
   "npm install lodash" → 未知命令 → AskForApproval::OnRequest
   → ExecApprovalRequirement::NeedsApproval

② PreToolUse 钩子: (无配置) → 继续

③ PermissionRequest 钩子: (无配置) → 继续

④ Guardian 审查:
   routes_approval_to_guardian()? → OnRequest + AutoReview → YES
   build_guardian_prompt_items():
     用户意图: "帮我在项目中添加健康检查API"
     最近操作: write_file("src/api/health.rs")
     当前命令: "npm install lodash"
   → 审查会话发送到模型
   → 模型返回: { outcome: "allow", risk_level: "low",
        rationale: "安装已知的npm包，风险低" }
   → GuardianAssessment.outcome = Allow → 允许执行

⑤ 执行工具: UnifiedExecHandler → 运行 npm install

⑥ PostToolUse 钩子: (无配置) → 继续

⑦ 结果回写:
   "npm install lodash 执行完成: added 4 packages in 2.3s"
   → 下次 LLM 调用看到安装结果
```

### 24.7 设计决策

**Q1: 为什么分 6 步而不是合并？**

**A:** 关注点分离——策略规则（ExecPolicy）是管理员的职责，钩子（Hook）是开发者的职责，Guardian 是自动化的，用户 UI 是最后的防线。每层独立配置、独立测试、独立修改。

**Q2: 为什么 Guardian 失败关闭（Deny on error）？**

**A:** 安全优先原则。如果审查模型超时或出错，宁可错误拒绝（用户手动重试）也不错误允许（执行不安全命令）。安全永远比便利重要。

**Q3: 为什么用 oneshot channel 而不是 mutex 等待用户审批？**

**A:** 审批是一次性的请求-响应模式——一个审批请求对应一个批准/拒绝决定。oneshot channel 天然适合这种场景，零开销，不会发生死锁。mutex 用于可重入的共享资源保护。

---

## 25. Agent 创建与生命周期

> **初学者理解**：`Agere::spawn` 是创建一个新的 Agent 会话实例——包括它的通道、状态、后台循环。类比：雇佣一个新员工（分配工位、电话、任务清单），`tokio::spawn` = 告诉这个员工"开始干活吧"。Agent 创建是整个系统的**入口**——每个会话、每个子代理都从这里开始生命。

### 25.1 7 层调用链总览

```
AgentControl.spawn_agent_with_metadata()              core/src/agent/control.rs:170
  └─ spawn_agent_internal()                            control.rs:181
     ├─ 预留 spawn slot (AgentRegistry, 限制 max_agents)
     ├─ 分配 nickname 和 AgentPath
     └─ spawn_new_thread_with_source()                 thread_manager.rs:906
        └─ spawn_thread_with_source()                  thread_manager.rs:1043
           └─ Agere::spawn(AgereSpawnArgs)            session/mod.rs:426
              └─ spawn_internal()                      session/mod.rs:450
                 ├─ 环境验证
                 ├─ 加载 Skills & Plugins
                 ├─ 解析执行策略 (inherit/parent/fresh)
                 ├─ ★ 创建核心双通道
                 ├─ ★ Session::new()                  session/session.rs:250
                 └─ ★ tokio::spawn(submission_loop)   session/mod.rs:660
                   └─ finalize_thread_spawn()          thread_manager.rs:1118
```

### 25.2 AgereSpawnArgs 参数结构

**文件：** `core/src/session/mod.rs:385-417`

```rust
pub(crate) struct AgereSpawnArgs {
    pub(crate) config: Config,
    pub(crate) models_manager: SharedModelsManager,
    pub(crate) environment_manager: Arc<EnvironmentManager>,
    pub(crate) skills_manager: Arc<SkillsManager>,
    pub(crate) plugins_manager: Arc<PluginsManager>,
    pub(crate) mcp_manager: Arc<McpManager>,
    pub(crate) skills_watcher: Arc<SkillsWatcher>,
    pub(crate) conversation_history: InitialHistory,     // 新/清除/恢复/Forked
    pub(crate) session_source: SessionSource,             // Session / SubAgent
    pub(crate) agent_control: AgentControl,
    pub(crate) dynamic_tools: Vec<DynamicToolSpec>,
    pub(crate) inherited_exec_policy: Option<Arc<ExecPolicyManager>>,
    pub(crate) inherited_shell_snapshot: Option<Arc<ShellSnapshot>>,
    pub(crate) parent_rollout_thread_trace: ThreadTraceContext,
    pub(crate) parent_trace: Option<W3cTraceContext>,
    pub(crate) thread_store: Arc<dyn ThreadStore>,
    // ... 更多 fields
}
```

### 25.3 核心双通道创建

```
Agere::spawn 中:
  (tx_sub, rx_sub) = async_channel::bounded(512)      // 提交通道: 有界, 512 容量
  (tx_event, rx_event) = async_channel::unbounded()   // 事件通道: 无界

为什么提交有界？背压保护——消费端跟不上时发送者被阻塞，防止内存爆炸。
为什么事件无界？事件不能丢失——漏掉 TurnComplete 会导致 TUI 永远显示 "Agent is thinking..."
```

### 25.4 Session::new 完整流程

**文件：** `core/src/session/session.rs:250`

```
Session::new(config, ..., tx_event, agent_status_tx, ...) → Arc<Session>:

① 4 路并发初始化 (tokio::join!):
   ├── create_or_resume_live_thread()    // 持久化历史
   ├── get_state_db_handle()             // 本地状态 DB
   ├── load_history_metadata()           // 消息历史元数据
   └── fetch_mcp_servers_and_oauth()     // MCP 服务器 + OAuth 状态

② 同步构建:
   ├── build_trace_metadata()            // W3C 追踪
   ├── SessionState::new()
   ├── configure_hooks(config)           // 加载 Hook 配置
   ├── configure_default_shell()         // 默认 shell
   └── SessionServices {
           mcp_manager, exec_manager, model_client,
           hooks, telemetry, network_policy,
           skills_manager, plugins_manager, agent_control, ...
       }

③ 初始事件:
   └── session.deliver_event_raw(Event::SessionConfigured(...))
       └─ ThreadManager 等待这个事件（必须为第一个事件）

④ 后台任务:
   ├── spawn_skills_watcher_listener()    // SkillsWatcher 监听
   ├── reinit_mcp_connection_manager()    // MCP 连接管理
   └── schedule_warmup()                  // WebSocket 预热

⑤ return Arc::new(session)
```

### 25.5 SessionServices 共享服务继承

**文件：** `core/src/state/service.rs:36-74`, `core/src/agere_delegate.rs:77-103`

子代理不创建自己的服务副本——它接收父代理的 `Arc::clone` 句柄：

```rust
AgereSpawnArgs {
    environment_manager: Arc::clone(&parent.services.environment_manager),
    skills_manager: Arc::clone(&parent.services.skills_manager),
    plugins_manager: Arc::clone(&parent.services.plugins_manager),
    mcp_manager: Arc::clone(&parent.services.mcp_manager),
    skills_watcher: Arc::clone(&parent.services.skills_watcher),
    agent_control: parent.services.agent_control.clone(),
    inherited_exec_policy: Some(Arc::clone(&parent.services.exec_policy)),
    thread_store: Arc::clone(&parent.services.thread_store),
}
```

### 25.6 AgentStatus 状态机

**文件：** `core/src/agent/status.rs:6-28`

```
正常路径: PendingInit → Running → Completed("最终回复...")
中断路径: PendingInit → Running → Interrupted
错误路径: PendingInit → Running → Errored("错误消息...")
无运行关闭: PendingInit → Shutdown
```

**状态转换触发：**

| 事件 | 新状态 |
|------|--------|
| `TurnStarted` | `Running` |
| `TurnComplete { last_agent_message }` | `Completed(message)` |
| `TurnAborted { Interrupted }` | `Interrupted` |
| `TurnAborted { error }` | `Errored(error)` |
| `Error { message }` | `Errored(message)` |
| `ShutdownComplete` | `Shutdown` |

### 25.7 finalize_thread_spawn

**文件：** `core/src/thread_manager.rs:1118-1150`

```
finalize_thread_spawn(agere, thread_id, ...):
  1. 等待第一个事件: agere.next_event() → 必须为 SessionConfigured
  2. 包装: Arc<AgereThread> { agere, thread_id, config, ... }
  3. 注册: 插入到活动线程映射表 (thread_map)
  4. 返回 Arc<AgereThread>
```

### 25.8 完整实例追踪

**场景**：用户启动 Agere，创建第一个 Agent 会话

```
① CLI → MultitoolCli::parse() → None（默认交互模式）
  → run_interactive_tui(cli, config_overrides, remote, arg0_paths)

② TUI 启动 → AppServer 初始化 → 用户发送第一条消息

③ ThreadManager::spawn_thread_with_source():
   ├─ validate_environments()
   ├─ register_skills_watcher()
   └─ Agere::spawn(AgereSpawnArgs {
        session_source: SessionSource::Session,
        conversation_history: InitialHistory::New,
        inherited_exec_policy: None,  // 根会话没有父代理
        ...
      })

④ Agere::spawn_internal():
   ├─ load_plugins_and_skills() → SkillsConfiguration
   ├─ resolve_exec_policy() → 从 config.toml 加载
   ├─ select_model() → "claude-sonnet-4-7"
   ├─ 创建双通道: tx_sub(bounded 512) + tx_event(unbounded)
   ├─ 构建 SessionConfiguration
   └─ Session::new(...) → Arc<Session>
      ├─ 4 路 join! (persistence/state_db/history/mcp)
      ├─ SessionServices 初始化
      └─ deliver_event_raw(SessionConfigured)

⑤ tokio::spawn(submission_loop(session, config, rx_sub))

⑥ finalize_thread_spawn():
   ├─ 等待 SessionConfigured 事件 ✓
   ├─ Arc<AgereThread> 创建
   └─ 注册到 thread_map

⑦ 用户输入被 submission_loop 接收 → user_input_or_turn
  → NoActiveTurn → spawn_task(RegularTask::new())
  → ★ 第一个 Turn 开始 ★
```

### 25.9 设计决策

**Q1: 为什么提交通道有界(512)而事件通道无界？**

**A:** 提交是用户行为——慢一点用户可感知但安全。事件是系统通知——丢了会导致 TUI 状态错误（如永远显示 "thinking"）。背压保护内存 vs 不丢失关键通知。

**Q2: 为什么 Session::new 用 4 路 tokio::join! 并发初始化？**

**A:** 4 个操作都是独立 I/O（持久化、DB、历史元数据、MCP/OAuth），串行执行会显著增加启动延迟。并发执行将启动时间最小化。

**Q3: 为什么子代理共享父服务而不是创建副本？**

**A:** 服务创建成本高（网络连接、内存缓存）。子代理共享父服务可以立即启动，资源消耗最小。同时保证一致性——所有代理看到相同的 MCP 工具、Skills、执行策略。

---

## 26. 多代理协作

> **初学者理解**：Agent 可以"分身"——当任务足够复杂时，主 Agent 可以派生子 Agent 去处理子任务，就像经理分配工作给下属。子代理共享父代理的服务（模型、工具、MCP），但有自己的对话历史和 Turn 循环。关键设计：子代理的审批请求不直接发给用户，而是**冒泡**到父代理。

### 26.1 spawn_agent V2 完整流程

**文件：** `core/src/tools/handlers/multi_agents_v2/spawn.rs`

```
① 参数解析:
   message, task_name, agent_type, model,
   reasoning_effort, fork_turns, fork_context

② 深度限制检查:
   child_depth vs max_depth → 超限则返回错误

③ 发送 CollabAgentSpawnBeginEvent (通知 TUI)

④ 构建代理配置:
   ├─ 继承父: provider, auth, approval_policy,
   │            permission_profile, cwd, environments
   ├─ fork_turns = "all" → FullHistory fork (禁止覆盖模型)
   ├─ fork_turns = "3" → LastNTurns(3) fork (允许覆盖模型+角色)
   └─ fork_turns = "none" → 不复制历史

⑤ 组装 spawn source: SessionSource::SubAgent

⑥ 确定初始操作:
   ├─ InterAgentCommunication (协作消息)
   └─ 或 UserInput (用户指令)

⑦ spawn_agent_with_metadata(config, op, source, options)

⑧ 提取 effective_model, effective_reasoning_effort

⑨ 发送 CollabAgentSpawnEndEvent

⑩ 返回 SpawnAgentResult { task_name, nickname }
```

### 26.2 fork_turns 历史继承机制

**SpawnAgentForkMode** (`core/src/agent/control.rs:47-49`)：

```rust
enum SpawnAgentForkMode {
    FullHistory,       // 完整历史
    LastNTurns(usize), // 最后 N 轮
}
```

**Fork 执行流程** (`core/src/agent/control.rs:328-437`)：

```
spawn_forked_thread():
  1. 刷新并物化父 rollout (JSONL)
  2. 加载 rollout 历史 → Vec<RolloutItem>
  3. 如果是 LastNTurns(n):
     └─ truncate_rollout_to_last_n_fork_turns()
        └─ 从末尾向前找第 n 个 fork turn 边界
           └─ fork_turn_positions_in_rollout():
              ├─ 真实用户消息边界 (TurnItem::UserMessage)
              └─ InterAgentCommunication 边界 (trigger_turn=true)
  4. 过滤 MultiAgentV2 usage hints
  5. fork_thread_with_source(config, InitialHistory::Forked(...))
```

### 26.3 keep_forked_rollout_item 过滤

**文件：** `core/src/agent/control.rs:98-125`

| RolloutItem 类型 | 保留？ | 原因 |
|-----------------|-------|------|
| System / Developer / User 消息 | ✅ 保留 | 核心指令和用户意图 |
| Assistant FinalAnswer | ✅ 保留 | 父代理的最终结论 |
| Assistant Reasoning | ❌ 丢弃 | 中间推理，子代理不需要 |
| Tool Calls / Outputs | ❌ 丢弃 | 子代理有自己独立的工具调用 |
| TurnContext | ❌ 丢弃 | 子代理建立自己的基线 |
| Compacted Items | ❌ 丢弃 | 新代理从干净历史开始 |

### 26.4 Mailbox 代理间通信

**文件：** `core/src/agent/mailbox.rs:11-72`

```rust
struct Mailbox {
    tx: mpsc::UnboundedSender<InterAgentCommunication>,
    seq_tx: watch::Sender<u64>,          // 单调递增序列号
}

struct MailboxReceiver {
    rx: mpsc::UnboundedReceiver<InterAgentCommunication>,
    buffer: VecDeque<InterAgentCommunication>,
    last_seen_seq: u64,
}
```

**InterAgentCommunication** (`protocol/src/protocol.rs:806-855`)：

```rust
pub struct InterAgentCommunication {
    pub author: AgentPath,              // 发送者路径
    pub recipient: AgentPath,           // 接收者路径
    pub other_recipients: Vec<AgentPath>,
    pub content: String,                // 消息内容
    pub trigger_turn: bool,             // true = 接收后启动新 Turn
}
```

**集成到 Session：**
- `enqueue_mailbox_communication()` — 发送到邮箱
- `has_pending_mailbox_items()` — 检查是否有未读邮件
- `subscribe_mailbox_seq()` — 获取 watch receiver 用于轮询

### 26.5 forward_events 审批冒泡

**文件：** `core/src/agere_delegate.rs:242-390`

子代理的事件被拦截并重定向：

| 事件类型 | 处理方式 |
|---------|---------|
| `AgentMessageDelta` / `AgentReasoningDelta` / `TokenCount` | **忽略**（从别处获取） |
| `SessionConfigured` / `ThreadNameUpdated` | **忽略** |
| `ExecApprovalRequest` | **拦截** → `parent_session.request_command_approval()` |
| `ApplyPatchApprovalRequest` | **拦截** → `parent_session.request_patch_approval()` |
| `RequestPermissions` | **拦截** → `parent_session.request_permissions_for_cwd()` |
| `RequestUserInput` | **拦截** → `parent_session.request_user_input()` 或 Guardian 自动审查 |
| `McpToolCallBegin` / `McpToolCallEnd` | **转发** + 缓存 |
| 所有其他事件 | **转发** |

### 26.6 5 个代理协作工具

| 工具 | 功能 | 交付模式 |
|------|------|---------|
| `spawn_agent` (`spawn.rs`) | 创建子代理 | — |
| `send_message` (`send_message.rs`) | 发送消息 | QueueOnly（不触发 Turn） |
| `followup_task` (`followup_task.rs`) | 发送跟进任务 | TriggerTurn（触发新 Turn） |
| `wait_agent` (`wait.rs`) | 等待子代理完成 | mailbox seq watch + 超时 |
| `close_agent` (`close_agent.rs`) | 关闭子代理 | 级联关闭（关闭整棵子树） |
| `list_agents` (`list_agents.rs`) | 列出子代理 | 路径前缀过滤 |

### 26.7 代理生命周期与状态流转

```
Running ──→ WaitingForInput ──→ Completed
    │              │                │
    └──────────────┴────→ Errored ──┘
                           │
                           └──→ Cancelled
```

**AgentControl 控制平面** (`core/src/agent/control.rs:133-140`)：
- `AgentRegistry` — 内存中的代理追踪
- `SpawnReservation` — RAII guard（commit/drop）
- 深度限制 — `next_thread_spawn_depth()` 检查

### 26.8 完整实例追踪

**场景**：主 Agent 收到 "同时给三个微服务添加健康检查端点"

```
主 Agent (Agere/0):
  ① LLM 分析 → 决定并行派生子代理
  ② 调用 spawn_agent V2 × 3:

      子代理 A: spawn_agent("service-a 健康检查", task_name="health-a",
                 fork_turns="all")
      子代理 B: spawn_agent("service-b 健康检查", task_name="health-b",
                 fork_turns="all")
      子代理 C: spawn_agent("service-c 健康检查", task_name="health-c",
                 fork_turns="all")

  ③ 每个子代理:
     ├─ fork 父历史（保留用户消息 + FinalAnswer）
     ├─ 创建自己的 Agere 会话
     ├─ 独立执行 Turn 循环
     └─ 子代理 B 需要写入 → ExecApprovalRequest
         └─ forward_events 拦截 → 冒泡到主 Agent
            └─ 用户看到审批对话框 → 批准
            └─ 子代理 B 继续执行

  ④ 主 Agent 调用 wait_agent 等待子代理完成

  ⑤ 子代理返回 Complete → 主 Agent 收集结果

  ⑥ 主 Agent 调用 close_agent 清理:
     shutdown_agent_tree() → 递归关闭所有后代

  ⑦ 主 Agent 整合三个子代理的输出 → 最终回复用户
```

### 26.9 设计决策

**Q1: 为什么审批要冒泡到父代理而不是直接给用户？**

**A:** 用户只需要在一个地方做决定。如果 3 个子代理同时请求审批，用户会被 3 个弹窗淹没。冒泡让用户看到所有审批请求的汇总。

**Q2: 为什么 fork_turns 过滤掉中间推理？**

**A:** 子代理只需要结论（FinalAnswer），不需要父代理的思考过程（中间推理、工具调用细节）。过滤避免 noise 占用子代理的 context window。

**Q3: 为什么 FullHistory 模式禁止覆盖模型和角色？**

**A:** 全量历史 fork 意味着子代理的对话历史与父代理完全一致。如果同时覆盖模型或角色，历史与新配置不一致——模型看到的是父代理的旧对话，但角色设定是新的，产生矛盾。

---

## 27. Context Compact——上下文压缩

> **初学者理解**：AI 的 context window 有限。对话太长时 Agere 自动做"摘要压缩"——用压缩提示让模型总结历史，替换旧对话腾空间。类比：把一本厚书读成读书笔记——丢掉细节，保留关键情节和人物关系。压缩后的历史加上摘要，新 Turn 从摘要出发继续。

### 27.1 触发时机

| 触发 | 条件 | 代码路径 |
|------|------|---------|
| 自动预压缩 | 下轮 token 超过 context window 90% | `run_pre_sampling_compact()` → `run_inline_auto_compact_task` |
| 自动中轮压缩 | 响应中 token 逼近上限 | 流式处理循环中检测 |
| 模型降级 | 切换到更小 context window 的模型 | `maybe_run_previous_model_inline_compact()` |
| 手动 `/compact` | 用户主动输入 | TUI → `AppCommand::Compact` |

**阈值计算** (`protocol/src/openai_models.rs:310-321`)：

```rust
pub fn auto_compact_token_limit(&self) -> Option<i64> {
    let context_limit = self.resolved_context_window()
        .map(|context_window| (context_window * 9) / 10);  // 90%
    // 用户可配置更低的硬限制
    let config_limit = self.auto_compact_token_limit;
    context_limit.map_or(config_limit, |limit| {
        config_limit.map_or(limit, |c| std::cmp::min(limit, c))
    })
}
```

### 27.2 本地压缩 vs 远程压缩

**决策：** `should_use_remote_compact_task(provider_info)` — 仅 OpenAI/Azure 支持远程 compact endpoint。

### 27.3 本地压缩完整流程

**文件：** `core/src/compact.rs`

```
① 读取 templates/compact/prompt.md 构建压缩提示
   "You are performing a CONTEXT CHECKPOINT COMPACTION.
    Create a handoff summary for another LLM that will resume the task."

② run_compact_task_inner() → 调模型 → 流式接收 → 提取 summary
   - drain_to_completed(): 流式接收模型响应
   - ContextWindowExceeded: 移除最旧项目并重试 (front-trimming)
   - 指数退避重试 (最多 stream_max_retries 次)

③ build_compacted_history():
   - collect_user_messages(): 从尾部收集用户消息（新→旧）
     最多 COMPACT_USER_MESSAGE_MAX_TOKENS (20,000 tokens)
   - 过滤掉 previous summary messages (is_summary_message)
   - 摘要作为最后一条用户消息追加
       前缀: templates/compact/summary_prefix.md
       "Another language model started to solve this problem..."

④ InitialContextInjection:
   - BeforeLastUserMessage (中轮压缩):
     插入初始上下文到最后一个真实用户消息之前
   - DoNotInject (预压缩/手动):
     下次 Turn 重新注入完整初始上下文

⑤ replace_compacted_history():
   创建 CompactedItem → 替换 ContextManager.items
   → history_version 递增 → 重建 diff baseline
```

### 27.4 远程压缩完整流程

**文件：** `core/src/compact_remote.rs`

```
① 创建 compaction trace context
② Pre-trimming: trim_function_call_history_to_fit_context_window()
   - Back-trimming: 移除最新的 agent 生成项（工具输出、开发者消息）
   - 确保压缩请求本身适合 context window

③ 调用 sess.services.model_client.compact_conversation_history()
   发送完整的 Prompt（含 base_instructions + tools）

④ process_compacted_history():
   should_keep_compacted_history_item():
     ❌ 丢弃: developer 消息、非真实用户消息、reasoning、
              function/tool calls/outputs、web search、image gen calls
     ✅ 保留: 真实用户消息、hook prompts、assistant 消息、compaction items

⑤ Mid-turn compact: 重建初始上下文 + insert_initial_context_before_last_real_user_or_summary()

⑥ replace_compacted_history() → 替换 ContextManager 历史
   CompactedItem.message = "" (远程压缩不注入摘要字符串)
```

### 27.5 数据保留对比

| 数据 | 本地压缩 | 远程压缩 |
|------|---------|---------|
| 摘要 | 模型生成 + SUMMARY_PREFIX | 模型决定保留内容 |
| 用户消息 | 保留尾部最多 20K tokens | 保留在压缩输出中的 |
| 工具调用/结果 | ❌ 丢弃 | ❌ 丢弃 |
| 推理块 | ❌ 丢弃 | ❌ 丢弃 |
| 开发者指令 | 下次 Turn 重新注入 | 下次 Turn 重新注入 |

### 27.6 ContextManager 配合

**文件：** `core/src/context_manager/history.rs`

```rust
struct ContextManager {
    items: Vec<ResponseItem>,            // 完整对话历史
    history_version: u64,               // 压缩/回滚时递增
    token_info: Option<TokenUsageInfo>, // Token 使用追踪
    reference_context_item: Option<TurnContextItem>, // diff 基线
}
```

**压缩后的 Turn 行为：**
- `reference_context_item = Some`（中轮压缩）→ diff-based context update（仅发送变更）
- `reference_context_item = None`（预压缩/手动）→ 下次 Turn 做 full reinjection via `build_initial_context()`

### 27.7 完整实例追踪

**场景**：对话历史超过 90% 上下文窗口，触发自动预压缩

```
① Turn 完成 → total_usage_tokens = 182,000
   context_window = 200,000
   auto_compact_limit = 200,000 * 9/10 = 180,000
   182,000 >= 180,000 → 触发压缩

② run_pre_sampling_compact():
   ├─ should_use_remote_compact_task(provider) → Anthropic → false
   └─ run_inline_auto_compact_task() → 本地压缩

③ drain_to_completed():
   "Current progress: 已创建健康检查 API 端点，
    还需要添加单元测试和集成测试..."

④ build_compacted_history():
   ├─ 用户消息 1: "帮我在项目里加一个健康检查API端点"
   ├─ 用户消息 2: "还需要添加单元测试和集成测试吗？"
   └─ 摘要: "[SUMMARY_PREFIX]\n
        Current progress: 创建了 src/api/health.rs,
        使用了 axum::Router 模式。
        Next steps: 添加 unit tests + integration tests,
        可参考 src/api/users.rs 的测试。"

⑤ replace_compacted_history():
   ContextManager.items = Vec[用户消息1, 用户消息2, 摘要]
   history_version = 2
   reference_context_item = None

⑥ 下次 Turn:
   build_initial_context() → 完整初始上下文
   for_prompt() → 压缩后的 items（2 条用户消息 + 摘要 + 初始上下文）
   → LLM 看到摘要 "current progress...next steps" → 继续工作
```

### 27.8 设计决策

**Q1: 为什么本地压缩保留尾部用户消息？**

**A:** 用户最近的意图最重要。丢掉早期的用户消息（已被摘要覆盖），但保留最近的——保证 LLM 能看到用户最新的需求变化。

**Q2: 为什么远程压缩用 back-trimming 而不是 front-trimming？**

**A:** 远程压缩需要先发送压缩请求到 API。back-trimming 移除最新的 agent 生成项（通常最长），确保请求本身不超出窗口。front-trimming 用于本地压缩的重试——如果历史太长，从前面删除最旧项。

**Q3: 为什么有 reference_context_item diff baseline？**

**A:** 避免每次 Turn 都发送完整的初始上下文（instructions、tools、permissions 等数千 token）。有了 baseline，后续 Turn 只发送变更（如模型切换、权限更新），显著减少 prompt token 消耗。

---

## 28. Token、Caching 与 Thinking

> **初学者理解**：Token 是 LLM 的基本计量单位（约 4 字节 = 1 token）。Prompt Caching 是 Anthropic 的优化——重复的提示前缀可以被缓存，下次直接使用缓存而不是重新计算。Thinking 是模型的"思考过程"——在输出最终答案之前，模型先生成内部推理。

### 28.1 Token 计数公式

**文件：** `utils/common/src/string.rs:41-48`

```rust
// 约 3.2 字节 = 1 token
pub fn approx_tokens_from_byte_count(bytes: usize) -> usize {
    bytes / 4 + bytes / 16    // bytes * 0.3125
}

// 反向公式
pub fn approx_bytes_for_tokens(tokens: usize) -> usize {
    tokens * 4 - tokens / 5
}
```

**ContextManager 中的 Token 估算** (`core/src/context_manager/history.rs:135-158`)：

- `estimate_token_count()` = base_instructions tokens + 所有 ResponseItem 的估算值
- `estimate_response_item_model_visible_bytes()`: 序列化为 JSON → 字节长度
- 内联 base64 图像：固定替代 ~7373 字节/张 (~1844 tokens)
- detail:"original" 图像：基于 32px 块进行分辨率分析（最大 10,000 块）
- 推理内容：`base64_len * 3 / 4 - 650` 字节

### 28.2 TokenUsageInfo 累计追踪

**文件：** `protocol/src/protocol.rs:1836-1900`

```rust
// 追踪累计 token 使用
new_or_append(prev, new):    // 增量累积
  last_token_usage = new     // 最近一次 API 响应
  total_token_usage += new   // 累计总和

fill_to_context_window():    // 标记窗口已满
  total_token_usage = context_window
  last_token_usage = context_window - previous_total

update_token_usage_info():   // 从流式 API 响应更新
recompute_token_usage():     // 根据历史项目完全重新计算
```

### 28.3 Prompt Caching 机制

**文件：** `anthropic-client/src/config.rs:9`, `anthropic-client/src/types.rs:220-222`

```rust
// DEFAULT_BETA_FEATURES: ["prompt-caching-2024-07-31"]
// 通过 anthropic-beta HTTP 头启用
```

**使用统计：**
```rust
pub cache_creation_input_tokens: Option<i64>,  // 缓存写入
pub cache_read_input_tokens: Option<i64>,      // 缓存读取
// map_usage() 将它们相加到 cached_input_tokens
```

关键：代码库中**没有**显式设置 `cache_control` 断点。缓存完全通过 Anthropic API 的服务器端启发式方法管理——API 自动选择最佳缓存断点。

### 28.4 Extended Thinking 配置

**ReasoningEffort** (`protocol/src/openai_models.rs:43-51`)：

```rust
pub enum ReasoningEffort { None, Minimal, Low, Medium, High, XHigh }
```

**Anthropic 映射** (`anthropic-client/src/translate/thinking.rs:16-40`)：

| ReasoningEffort | ThinkingConfig | OutputConfig |
|----------------|----------------|--------------|
| None | None（完全禁用） | None |
| Minimal / Low | `{ type: "adaptive" }` | "low" |
| Medium | `{ type: "adaptive" }` | "medium" |
| High | `{ type: "adaptive" }` | "high" |
| XHigh | `{ type: "adaptive" }` | "xhigh" |

- **adaptive 模式**: `budget_tokens` 始终为 `None`，模型自行决定思考长度
- Opus 4.7 优化了自适应思考

### 28.5 SSE Thinking 处理

**文件：** `anthropic-client/src/translate/response.rs`

三种 thinking 相关内容块：

| SSE 块类型 | 处理方式 |
|-----------|---------|
| `ContentBlockStart::Thinking` | `ResponseItem::Reasoning { content: vec![] }` |
| `ThinkingDelta` | 增量追加到 reasoning content |
| `ContentBlockStart::RedactedThinking` | `ResponseItem::Reasoning { encrypted_content: Some(data) }` |
| `SignatureDelta` | 签名应用于当前 thinking/redacted_thinking 块 |

### 28.6 WebSocket 预热详细流程

**文件：** `core/src/client.rs:1450-1500`

```
prewarm_websocket():
  └─ response.create (generate=false)
     └─ 建立 WebSocket 连接
     └─ previous_response_id 实现粘性路由
  └─ 等待 Completed 事件
  └─ 失败: 426 Upgrade Required → 切换回 HTTP

监控指标:
  agere.startup_prewarm.duration_ms
  agere.startup_prewarm.age_at_first_turn_ms
```

### 28.7 完整实例追踪

**场景**：一次 Turn 的 Token 计数过程

```
Turn 开始:
  estimate_token_count() → 12,000 tokens (base_instructions + history)

发送 LLM 请求:
  POST /v1/messages → 12,000 input tokens

流式接收:
  SSE stream → 500 output tokens

收到 usage:
  cache_creation_input_tokens: 5,000
  cache_read_input_tokens: 3,000
  input_tokens: 12,000
  output_tokens: 500

update_token_usage_info():
  cached_input_tokens = 5,000 + 3,000 = 8,000
  total_token_usage += 12,000 + 500 = 12,500

TUI 显示: "Tokens: 12.5K / 200K (8K cached)"
```

### 28.8 设计决策

**Q1: 为什么用字节近似而不是真正的分词器？**
A: 真正的分词器（如 tiktoken）需要加载大型词汇表文件。字节近似快 100 倍以上，误差通常在 ±10% 以内，足够用于阈值判断。

**Q2: 为什么不手动设置 cache_control 断点？**
A: Anthropic 的服务器端启发式自动选择最佳断点（在 system prompt 之后、tools 定义之后），比手动设置更智能且适应更多情况。

**Q3: 为什么 thinking 用 "adaptive" 模式？**
A: Opus 4.7 优化了自适应思考——模型自行判断何时需要深度思考、何时可以快速响应。固定预算要么浪费（简单问题思考太多），要么不足（复杂问题思考不够）。

---

## 29. / 命令菜单体系

> **初学者理解**：斜杠命令是 TUI 中用户控制 Agent 的方式。类比：终端中的 CLI 命令，但在 TUI 中以 `/name` 形式输入。按下 `/` 会弹出命令选择器，输入名称时实时过滤。所有常用操作都可通过命令访问。

### 29.1 40+ 斜杠命令完整清单

**文件：** `tui/src/slash_command.rs:12-73`

**会话管理：** `/new` — 新聊天 | `/clear` — 清除终端 | `/resume` — 恢复会话 | `/fork` — 分支会话 | `/quit` `/exit` — 退出

**模型与推理：** `/model` — 选择模型 | `/fast` — 快速模式 | `/personality` — 沟通风格

**内容与输出：** `/compact` — 压缩对话 | `/review` — 审查代码 | `/rename` — 重命名线程 | `/copy` — 复制回复 | `/diff` — 显示 git diff | `/status` — 显示当前状态 | `/mention` — 提及文件 | `/debug-config` — 调试配置层

**模式与协作：** `/plan` — 计划模式 | `/collab` — 协作模式 | `/goal` — 设置目标 | `/side` — 侧边对话 | `/agent` `/subagents` — 代理线程切换

**用户界面：** `/theme` — 主题选择 | `/title` — 标题配置 | `/statusline` — 状态行配置 | `/keymap` — 键位绑定

**权限与配置：** `/approvals` `/permissions` — 审批设置 | `/experimental` — 实验性功能 | `/memories` — 记忆管理 | `/mcp` — MCP 工具列表 | `/apps` — 应用管理 | `/plugins` — 插件浏览 | `/settings` — 实时设置

**调试与工具：** `/skills` — 技能管理 | `/init` — 创建 AGENTS.md | `/feedback` — 发送反馈 | `/ps` — 后台进程 | `/realtime` — 实时语音

**别名：** `/quit` = `/exit`, `/subagents` = `/agent`, `/approvals` = `/permissions`

### 29.2 命令解析和分发链路

```
终端按键 → handle_key_event → 检查全局热键 → ChatWidget → BottomPane
  │
  ├─ "/" 键 → activate_CommandPopup
  │   └─ 实时过滤: 精确匹配 + 前缀匹配
  │   └─ Tab 补全, Up/Down 导航, Esc 关闭
  │
  └─ Enter → try_dispatch_bare_slash_command()
              → InputResult::Command(SlashCommand)
              → chatwidget/slash_dispatch.rs: dispatch_command(cmd)
                 └─ 巨大 match 块覆盖所有 40+ 命令

  └─ Enter 带参数 → try_dispatch_slash_command_with_args()
                    → InputResult::CommandWithArgs(cmd, args, elements)
                    → dispatch_prepared_command_with_args(cmd, args, elements)
                       └─ 处理: /rename <name>, /fast on|off, /mcp verbose 等
```

### 29.3 CommandPopup 可搜索选择器

**文件：** `tui/src/bottom_pane/command_popup.rs:133`

- **过滤算法**：精确匹配（排前面）+ 前缀匹配（排后面）
- **特性门控**：Plans/Apps/Plugins/Fast/Goal/Personality/Realtime/Settings/Collab 根据功能开关
- **别名隐藏**：quit 和 approvals 默认隐藏, 输入前缀时显示
- **调试隐藏**：debug* 前缀命令仅 debug_assertions 可见

### 29.4 7 个键位绑定上下文

| 上下文 | 默认键 | 操作 |
|-------|--------|------|
| app | Ctrl+T / Ctrl+G / Ctrl+O / Ctrl+L | 成绩单 / 编辑器 / 复制 / 清除 |
| chat | Alt+, / Alt+, / Alt+Up | 减少/增加推理 / 编辑排队消息 |
| composer | Enter / Tab / Ctrl+R / Ctrl+S | 提交 / 排队 / 历史搜索前后 |
| editor | Ctrl+A,E / Alt+F,B / Ctrl+K | 15+ Emacs 风格编辑绑定 |
| pager | j,k / PgUp,PgDn / q | 分页/滚动/退出 |
| list | Up,Down / Ctrl+P,N / Enter,Esc | 列表导航/确认/取消 |
| approval | y,a,p / d,n / Esc | 批准/总是批准/拒绝/取消 |

### 29.5 11 个保留键（不可重新绑定）

| 按键 | 操作 | 原因 |
|------|------|------|
| Ctrl+C | interrupt_or_quit | 安全出口 |
| Ctrl+D | quit | 用户中断 |
| Ctrl+V / Ctrl+Alt+V | paste_image | 图片粘贴 |
| Shift+Tab | cycle_collaboration_mode | 模式切换 |
| Esc | backtrack | 返回上一级 |
| Alt+Left/Right | agent 切换 | 线程切换 |
| `/` / `!` / `@` / `$` | slash / shell / files / connectors | 命令前缀 |

### 29.6 完整实例追踪

**场景**：用户执行 `/compact` 命令

```
① 用户按 `/` → CommandPopup 激活
② 输入 "co" → 过滤列表: /compact, /collab, /copy
③ 按 Enter 确认 /compact
④ try_dispatch_bare_slash_command("compact")
   → InputResult::Command(SlashCommand::Compact)
⑤ dispatch_command(SlashCommand::Compact)
   → ChatWidget::dispatch_command()
   → AppEvent::Compact → AppServer → Core
   → submission_loop 收到 Op::Compact
   → run_compact_task() → 压缩完成
   → TUI 显示 "对话已压缩"
```

### 29.7 设计决策

**Q1: 为什么用 CommandPopup 而不是硬编码快捷键？**
A: CommandPopup 提供可发现性（新用户按 `/` 看到所有可用命令）和可搜索性（模糊过滤）。硬编码快捷键只有记住所有快捷键的专家才能高效使用。

**Q2: 为什么保留键不可重新绑定？**
A: 安全出口（Ctrl+C 中断）和核心导航（Esc 返回）在任何键位配置下都必须可用。如果用户可以解绑这些键，系统可能无法控制。

---

## 30. Skills & Plugins 扩展系统

> **初学者理解**：Skills 是预定义的"专业领域知识包"——教 Agent 如何执行特定任务（代码审查、前端设计）。Plugins 是外部扩展——可以添加新的 MCP 服务器、工具、钩子。类比：Skills 是员工的培训手册，Plugins 是公司购买的外部服务。

### 30.1 SkillsManager 技能加载

**文件：** `core/src/session/mod.rs:483-494`, `core/src/skills_watcher.rs`

```
spawn_internal():
  query_skills_for_config(skills_manager, plugins_manager)
    └─ 加载 templates/skills/ 目录下的技能文件
    └─ 返回 SkillsConfiguration

技能注入到 system prompt:
  build_initial_context():
    注入 "Available skills: code-review, frontend-design, ..."
```

### 30.2 SkillsWatcher 热更新

```
fs watcher (监控 templates/skills/ 目录)
  └─ 文件变更事件 (创建/修改/删除)
  └─ broadcast::channel(128) → SkillsWatcherEvent
  └─ Session 监听 → 重新加载技能配置
     └─ Arc::downgrade 模式避免循环引用
```

**文件：** `core/src/skills_watcher.rs:38-47`

```rust
pub struct SkillsWatcher {
    tx: broadcast::Sender<SkillsWatcherEvent>,
}

enum SkillsWatcherEvent { Changed }
```

### 30.3 PluginsManager 插件管理

插件可以提供的 4 种能力：
- MCP 服务器声明 (`mcp_server_names`) — 插件注册的 MCP 服务器
- 工具插件归属 (`ToolPluginProvenance`) — 工具到插件的映射
- 钩子源 (`PluginHookSource → PLUGIN_ROOT + PLUGIN_DATA`) — 插件提供的钩子
- 配置贡献 — 插件可以为 MCP 服务器提供默认配置

### 30.4 Marketplace 插件市场

- `/plugins add <name>` — 安装插件
- `/plugins remove <name>` — 卸载插件
- `/plugins upgrade` — 升级所有插件

### 30.5 apply_role_to_config 角色配置层叠

**文件：** `core/src/agent/role.rs:40-56`

```
resolve_role_config():
  ├─ 用户自定义角色 (config.agent_roles)
  └─ 内置角色: default, explorer, worker

apply_role_to_config(role):
  ├─ 加载角色的 TOML 配置层
  └─ apply_role_to_config_inner():
     ├─ 插入到配置层堆栈 (SessionFlags precedence)
     └─ preservation_policy:
         ├─ 角色层未设 model_provider → 保留当前
         └─ 角色层未设 profile → 保留当前
```

**内置角色** (`core/src/agent/role.rs:352-428`)：
- `default` — 无额外配置，保持当前设置
- `explorer` — 使用 explorer.toml 配置，快速、权威、适合代码库探索
- `worker` — 用于执行/生产工作（实现和测试）

### 30.6 完整实例追踪

**场景**：用户创建自定义技能并立即使用

```
① 用户创建文件: ~/.openagere/skills/code-review.md
   内容: "你是一个代码审查专家。审查时关注:
          1. 安全漏洞 2. 性能问题 3. 代码可读性"

② SkillsWatcher 检测到新文件 → broadcast SkillsWatcherEvent::Changed

③ Session 监听器收到事件 → 重新加载技能
   └─ SkillsConfiguration 更新 → 包含 "code-review"

④ 用户: "帮我审查这段代码"
   → build_initial_context() 注入:
      "Available skills: code-review
       Use Skill tool to invoke: code-review"

⑤ 模型看到可用技能 → 调用 Skill(code-review) 工具
   → 技能内容注入到 system promptV
   → 模型基于技能指导执行审查

⑥ 用户: "审查结果很专业！" → 下次请求继续用该技能
```

### 30.7 设计决策

**Q1: 为什么 Skills 用文件而不是数据库？**
A: 用户友好——可以用任何编辑器修改。版本控制友好——markdown 文件可以提交到 Git。无需额外的数据库管理。

**Q2: 为什么用 fs watcher 而不是轮询？**
A: 实时性——创建文件后立即生效，不需要等待轮询周期。资源消耗更低——只在文件变化时触发。

**Q3: 为什么角色配置用 TOML 层？**
A: 与 config.toml 一致——角色配置使用相同的格式和合并规则。`preservation_policy` 确保角色只覆盖需要修改的字段。

---

## 31. 特殊工具与高级特性

> **初学者理解**：除了标准的工具注册/路由/执行系统，OpenAgere 还有一些"内置原生工具"和高级特性——它们不是通过 MCP 注册的，而是直接嵌入在 Agere 核心中。这些为特定场景优化了性能和用户体验。

### 31.1 arg0 dispatch 机制

**文件：** `arg0/src/lib.rs:58-402`, `cli/src/main.rs`

```
同一个 agere 二进制根据 argv[0] 名称决定执行不同的子命令:

agere          → 默认交互式 TUI
agere-review   → exec mode: 代码审查
apply_patch    → 应用补丁独立工具
```

### 31.2 RgHandler — 原生 ripgrep 搜索

**文件：** `core/src/tools/handlers/rg.rs`

- 参数: pattern, path, case_sensitive, include, exclude
- 执行: 启动 ripgrep 子进程 → 流式输出 → 结果截断
- 路径安全: 必须在项目目录或允许的目录范围内
- 并行支持: 是（只读操作）

### 31.3 GrepSearchManager — TUI grep popup

**文件：** `agere-grep-search/src/`, `tui/src/grep_search.rs`

```
用户触发 grep 搜索 (快捷键或模型调用)
  └─ GrepSearchManager::search()
     ├─ grep-searcher crate 执行搜索
     ├─ StreamingSession 管理流式结果
     └─ SessionReporter 回调 → TUI popup 更新
  └─ TUI GrepSearchPopup 渲染搜索结果
```

### 31.4 apply_patch 解析执行

**文件：** `apply-patch/src/`

```
补丁文本 → parser → 提取 hunks
  └─ seek_sequence: 基于上下文的智能定位
     (不是简单的行号匹配——行号可能偏移)
  └─ standalone 模式: 可以独立于 Agent 运行
  └─ Agent 模式: 工具调用 → 审批 → 执行
```

### 31.5 Realtime Conversation 实时对话

**文件：** `agere-api/src/endpoint/realtime_websocket/`

WebSocket 实时语音对话：
- V1 会话: 基础音频流
- V2 会话: 支持中断 (barge-in)、代理委派 (handoff delegation)
- 与 Turn 模式切换: 实时模式和回合模式可混合使用

### 31.6 Session 恢复与 Fork

**文件：** `core/src/thread_manager.rs`, `core/src/session/mod.rs`

**openagere resume:**
- `--last` → 恢复最近一次会话
- 加载 rollout (JSONL) → 重放历史 → 从断点继续

**agere fork:**
- `--last` → 分支最近一次会话
- `InitialHistory::Forked()` → 过滤历史 → 新 Agent 会话
- fork_turn_positions: 识别用户消息边界和 trigger_turn 边界

### 31.7 远程模式与 AppServer 分离

**嵌入模式 (Embedded):**
- InProcessAppServerClient + mpsc 通道
- 零网络延迟，简单部署

**远程模式 (Remote):**
- RemoteAppServerClient + WebSocket JSON-RPC
- 多客户端共享 Agent，独立重启 TUI
- OutgoingEnvelope::ToConnection vs Broadcast (按 ConnectionId 路由)

### 31.8 设计决策

**Q1: 为什么 rg 用独立 handler 而不是通用 shell 执行？**
A: 性能——专用 handler 利用 ripgrep 的流式输出。结构化输出——结果可以直接解析为搜索结果列表。安全——严格的路径检查。

**Q2: 为什么 apply_patch 有独立的 seek 算法？**
A: 行号可能因编辑不匹配。seek_sequence 基于代码上下文定位（寻找代码开头的唯一行），比简单行号匹配更鲁棒。

**Q3: 为什么提供嵌入和远程两种模式？**
A: 嵌入模式给个人开发者——启动快、零配置。远程模式给团队/服务器——多开发者共享 Agent、独立重启。

---

## 附录A：初学者速查表
### Rust 通道选择指南

| 场景 | 推荐通道 | 原因 |
|------|---------|------|
| 操作队列，需要背压 | `async_channel::bounded(N)` | 满时阻塞发送者，防止内存溢出 |
| 事件流，不能丢失 | `async_channel::unbounded()` | 永不阻塞发送者，保证事件送达 |
| 多对一消息分发 | `tokio::mpsc::channel(N)` | 多生产者支持 |
| 一对多广播 | `tokio::broadcast::channel(N)` | 一条消息多方接收 |
| 状态通知（保留最新值） | `tokio::watch::channel(v)` | 新订阅者立即看到当前值 |
| 一次性请求-响应 | `tokio::oneshot::channel()` | 单发单收，零开销 |

### Agent 领域术语速查

| 术语 | 含义 | 类比 |
|------|------|------|
| **Turn** | 一次完整的用户→AI→回复周期 | 一次"对话回合" |
| **Submission** | 向Agent发送的操作指令 | 给员工下达的任务单 |
| **Event** | Agent发出的状态通知 | 员工的状态汇报 |
| **Session** | Agent的全部运行时状态 | 员工的工作台 |
| **Op** | 操作类型枚举 | 任务类型（用户输入/中断/关闭...） |
| **EventMsg** | 事件类型枚举 | 汇报类型（开始工作/打字中/完成/出错...） |
| **LLM** | Large Language Model | AI大脑 |
| **SSE** | Server-Sent Events | 服务器→客户端的数据推送流 |
| **Tool Call** | LLM决定使用某个工具 | AI说"我需要用搜索工具" |
| **Tool Execution** | Agent框架实际执行工具 | 框架替AI执行搜索 |
| **ReAct Loop** | 推理→行动→观察→推理 循环 | AI边想边做的过程 |
| **Backpressure** | 背压：慢消费→阻塞生产 | 水龙头关小，不淹了厨房 |
| **AppServer** | Agent与外部世界的桥梁 | 公司的前台/接待 |
| **ToolRegistry** | 工具注册表，handler 映射 | 公司的"服务目录" |
| **ToolRouter** | 工具路由器，从 ResponseItem 到 ToolCall 的转换 | 前台分流员 |
| **ToolCallRuntime** | 工具运行时，并行/串行调度 | 项目经理排期 |
| **ToolOrchestrator** | 工具编排器，审批+重试 | 总指挥 |
| **MCP** | Model Context Protocol，外部服务连接协议 | 万能USB接口 |
| **Hook** | 执行生命周期中的外部介入点 | 安检关卡 |
| **Guardian** | 自动 AI 审查员 | 自动质检员 |
| **ExecPolicyManager** | 命令审批策略管理器 | 安保规则制定者 |
| **ApprovalStore** | 审批决定会话缓存 | 审批记录本 |
| **Skill** | 预定义的专业领域知识包 | 员工培训手册 |
| **Plugin** | 外部扩展（MCP 服务器、工具、钩子） | 第三方插件/扩展包 |
| **Rollout** | 对话历史的持久化记录（JSONL 格式） | 会议录音录像 |
| **Compact** | 上下文压缩（摘要压缩对话历史） | 会议纪要代替全程录音 |
| **ReasoningEffort** | 推理力度 (Minimal/Low/Medium/High/XHigh) | 思考深度档位 |
| **ModelMigration** | 启动时模型升级提示 | 系统升级公告 |
| **ForkSnapshot** | Fork 时的历史快照 | 分支存档 |
| **RealtimeConversation** | WebSocket 实时语音对话 | 电话通话 |

### 关键数据流向速查

```
用户输入 ──(JSON-RPC)──→ AppServer ──(Submission)──→ submission_loop
                                                                │
                                                       user_input_or_turn
                                                                │
                                                              run_turn
                                                        ┌───────┴───────┐
                                                   LLM调用          工具执行
                                                        │               │
                                                   流式文本增量      工具结果
                                                        │               │
                                                        └───┬───┘
                                                            │
                                              Session::send_event(Event)
                                                            │
                                              ──(tx_event 无界通道)──
                                                            │
                                              apply_bespoke_event_handling
                                                            │
                                              ──(JSON-RPC 通知)──
                                                            │
                                                         TUI显示
```

---

## 附录B：关键文件索引

| 文件路径 | 核心内容 |
|---------|---------|
| `cli/src/main.rs` | CLI入口、MultitoolCli、子命令调度 |
| `arg0/src/lib.rs` | Tokio运行时创建、arg0调度、.env加载 |
| `tui/src/lib.rs` | TUI入口、run_main()、run_ratatui_app() |
| `tui/src/app.rs` | App结构体、主事件循环、handle_tui_event |
| `tui/src/tui.rs` | Tui结构体、draw方法、终端管理 |
| `tui/src/chatwidget.rs` | ChatWidget、流式增量处理 |
| `tui/src/streaming/controller.rs` | StreamController和流状态机 |
| `tui/src/streaming/commit_tick.rs` | 提交动画tick |
| `tui/src/streaming/chunking.rs` | 自适应分块策略 |
| `tui/src/tui/frame_requester.rs` | 帧请求/调度器actor |
| `tui/src/app_event.rs` | AppEvent枚举定义 |
| `tui/src/app/input.rs` | 键盘事件分发 |
| `tui/src/keymap.rs` | RuntimeKeymap、键位绑定、7个上下文 |
| `core/src/session/mod.rs` | Agere结构体、spawn、send_event |
| `core/src/session/session.rs` | Session结构体、Session::new |
| `core/src/session/handlers.rs` | submission_loop、user_input_or_turn |
| `core/src/session/turn.rs` | run_turn、run_sampling_request、try_run_sampling_request |
| `core/src/session/turn_context.rs` | TurnContext创建 |
| `core/src/agere_delegate.rs` | 子代理生成、forward_events、forward_ops |
| `core/src/agent/mailbox.rs` | 邮箱系统（代理间通信） |
| `core/src/agent/status.rs` | AgentStatus状态机 |
| `core/src/client.rs` | ModelClient和ModelClientSession |
| `core/src/stream_events_utils.rs` | handle_output_item_done、工具调用提取 |
| `core/src/tools/router.rs` | ToolRouter、build_tool_call |
| `core/src/tools/parallel.rs` | ToolCallRuntime并行执行 |
| `core/src/tools/registry.rs` | dispatch_any工具注册表 |
| `core/src/thread_manager.rs` | ThreadManager、spawn_thread_with_source |
| `core/src/tasks/mod.rs` | spawn_task任务生命周期 |
| `core/src/tasks/regular.rs` | RegularTask::run |
| `anthropic-client/src/sse.rs` | Anthropic SSE流解析 |
| `anthropic-client/src/translate/response.rs` | SSE事件到ResponseEvent的转换 |
| `protocol/src/protocol.rs` | Event、EventMsg、AgentStatus、InterAgentCommunication定义 |
| `app-server/src/lib.rs` | AppServer启动、核心通道创建 |
| `app-server/src/transport/mod.rs` | TransportEvent定义 |
| `app-server/src/outgoing_message.rs` | OutgoingEnvelope、OutgoingMessageSender |
| `app-server/src/in_process.rs` | InProcessServerEvent、InProcessClientMessage |
| `app-server/src/agere_message_processor.rs` | 会话事件监听器任务 |
| `app-server/src/bespoke_event_handling.rs` | EventMsg到JSON-RPC通知的转换 |
| `app-server-client/src/lib.rs` | InProcessAppServerClient、RemoteAppServerClient |
| `model-provider-info/src/lib.rs` | ModelProviderInfo定义、WireApi、内置Provider列表 |
| `model-provider/src/provider.rs` | ModelProvider trait、create_model_provider工厂函数 |
| `model-provider/src/models_endpoint.rs` | OpenAiModelsEndpoint，GET /models 网络请求 |
| `models-manager/src/manager.rs` | ModelsManager trait、OpenAiModelsManager、刷新策略、缓存 |
| `models-manager/src/collaboration_mode_presets.rs` | CollaborationModeMask、内置协作模式预设 |
| `models-manager/src/model_info.rs` | ModelInfo、model_info_from_slug、模型推断逻辑 |
| `core/src/compact.rs` | 本地压缩：压缩提示、build_compacted_history、drain_to_completed |
| `core/src/compact_remote.rs` | 远程压缩：OpenAI原生compaction endpoint调用 |
| `core/src/compact_tests.rs` | 压缩逻辑单元测试 |
| `core/src/context/model_switch_instructions.rs` | ModelSwitchInstructions：模型切换后的上下文注入 |
| `core/src/context/subagent_notification.rs` | SubagentNotification：子代理状态通知注入对话 |
| `core/src/tools/handlers/multi_agents_v2/spawn.rs` | spawn_agent V2工具处理器完整流程 |
| `core/src/tools/handlers/multi_agents_v2/send_message.rs` | send_message V2消息发送 |
| `core/src/tools/handlers/multi_agents_v2/wait.rs` | wait_agent V2等待子代理完成 |
| `core/src/tools/handlers/multi_agents_v2/close_agent.rs` | close_agent V2关闭子代理 |
| `core/src/agent/control.rs` | AgentControl多代理控制平面、SpawnAgentForkMode |
| `core/src/agent/role.rs` | 代理角色配置、apply_role_to_config |
| `core/src/context_manager/history.rs` | ContextManager、对话历史管理、token统计 |
| `core/src/tasks/compact.rs` | CompactTask压缩任务调度 |
| `tui/src/model_catalog.rs` | ModelCatalog、模型列表缓存和协作模式 |
| `tui/src/model_migration.rs` | ModelMigrationScreen、启动时模型升级UI |
| `core/src/session/session.rs` | SessionConfiguration（provider + collaboration_mode） |
| `core/src/tools/orchestrator.rs` | ToolOrchestrator、审批+重试 |
| `core/src/tools/spec.rs` | build_specs_with_discoverable_tools |
| `core/src/tools/execution.rs` | Approvable trait、ApprovalStore、ExecApprovalRequirement |
| `core/src/tools/handlers/mcp.rs` | McpHandler |
| `core/src/tools/handlers/rg.rs` | RgHandler (ripgrep) |
| `core/src/tools/handlers/unified_exec.rs` | UnifiedExecHandler |
| `core/src/tools/handlers/list_dir.rs` | ListDirHandler |
| `core/src/tools/handlers/view_image.rs` | ViewImageHandler |
| `core/src/mcp_tool_call.rs` | handle_mcp_tool_call、execute_mcp_tool_call |
| `agere-mcp/src/connection_manager.rs` | McpConnectionManager、连接生命周期 |
| `agere-mcp/src/rmcp_client.rs` | AsyncManagedClient、初始化、list_tools |
| `agere-mcp/src/tools.rs` | ToolInfo、qualify_tools、命名空间冲突解决 |
| `agere-mcp/src/mcp/auth.rs` | OAuth 认证、compute_auth_statuses |
| `hooks/src/lib.rs` | HOOK_EVENT_NAMES、钩子事件常量 |
| `hooks/src/registry.rs` | HooksConfig、Hooks 结构体 |
| `hooks/src/engine/discovery.rs` | discover_handlers、钩子配置发现 |
| `hooks/src/engine/dispatcher.rs` | execute_handlers、调度逻辑 |
| `core/src/hook_runtime.rs` | run_pre_tool_use_hooks、run_post_tool_use_hooks |
| `core/src/exec_policy.rs` | ExecPolicyManager、审批策略 |
| `core/src/guardian/mod.rs` | Guardian 常量配置、断路器 |
| `core/src/guardian/review.rs` | Guardian 自动审查流程 |
| `core/src/guardian/review_session.rs` | 审查会话管理、build_guardian_review_session_config |
| `core/src/context_manager/normalize.rs` | 历史规范化 |
| `core/src/context_manager/updates.rs` | 上下文 diff/更新 |
| `core/src/thread_rollout_truncation.rs` | fork_turn_positions、truncate_rollout |
| `core/src/tools/handlers/multi_agents_v2/list_agents.rs` | list_agents（路径前缀过滤） |
| `tui/src/slash_command.rs` | SlashCommand 枚举、命令描述 |
| `tui/src/bottom_pane/command_popup.rs` | CommandPopup、可搜索选择器 |
| `tui/src/chatwidget/slash_dispatch.rs` | dispatch_command、命令分发 |
| `tui/src/keymap_setup.rs` | /keymap 重新绑定 UI |
| `core/src/skills_watcher.rs` | SkillsWatcher、热更新 |
| `utils/common/src/string.rs` | approx_tokens_from_byte_count（token 估算公式） |
| `anthropic-client/src/translate/thinking.rs` | ReasoningEffort → Thinking 映射 |
| `agere-api/src/endpoint/realtime_websocket/` | 实时对话 WebSocket 协议 |
| `app-server/src/in_process.rs` | InProcessAppServerClient、嵌入模式 |
| `agere-grep-search/src/` | grep 搜索 crate（types, config, engine, session） |
| `apply-patch/src/parser.rs` | patch 解析器 |
| `apply-patch/src/seek_sequence.rs` | seek 定位算法 |


