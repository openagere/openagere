 # `/review` 命令全链路深度走查文档

 > **面向读者**：刚接触 Agere 代码库、希望深入理解 `/review` 斜杠命令完整流程的新手开发者。
 > **文档目标**：从命令解析到子代理执行再到 UI 渲染，让你建立端到端、可落地、可验证的认知。
 > **配套规格**：`docs/superpowers/specs/2026-07-05-review-flow-walkthrough-design.md`
 > **生成日期**：2026-07-05
 > **方案**：Approach C — 混合架构（层参考 + 生命周期追踪 + 横切关注点）

 ---

 ## 目录

 - 第一部分 · 架构总览（§1.1-1.5）
 - 第二部分 · 层参考手册（§2.1-2.9）
 - 第三部分 · 端到端生命周期追踪（§3.1-3.6）
 - 第四部分 · 横切关注点（§4.1-4.7）
 - 第五部分 · 附录（§5.1-5.6）
 - 第六部分 · 逐行代码注解（§6.1-6.3）
 - 第七部分 · 状态机与数据流图（§7.1-7.3）
 - 第八部分 · 测试代码走读（§8.1-8.3）
 - 第九部分 · 调试与排障指南（§9.1-9.3）
 - 第十部分 · session/review.rs 逐行注解（§10.1-10.7）
 - 第十一部分 · review_prompt.md 审查准则详解（§11.1-11.6）
 - 第十二部分 · 完整索引与交叉引用（§12.1-12.3）
 - 第十三部分 · TUI 审查模式代码注解（§13.1-13.9）
 - 第十四部分 · 完整测试代码与注解（§14.1-14.3）
 - 第十五部分 · 速查手册（§15.1-15.6）
 - 第十六部分 · 设计决策与原理（§16.1-16.12）
 - 第十七部分 · 常见模式与惯用法（§17.1-17.12）
 - 第十八部分 · 扩展场景与边界案例（§18.1-18.10）
 - 第十九部分 · 协议层深度走读（§19.1-19.11）
 - 第二十部分 · 综合示例库（§20.1-20.5）
 - 第二十一部分 · handlers.rs 审查入口深度走读（§21.1-21.5）
 - 第二十二部分 · 完整源码清单（§22.1-22.4）
 - 第二十三部分 · 新手阅读指南（§23.1-23.5）
 - 第二十四部分 · 完整函数参考（§24.1-24.10）
 - 第二十五部分 · 完整 JSON 样例库（§25.1-25.8）
 - 第二十六部分 · 完整 ASCII 图表集（§26.1-26.8）
 - 第二十七部分 · 完整测试注解续（§27.1-27.5）
 - 第二十八部分 · 完整对比表集（§28.1-28.10）
 - 第二十九部分 · 详细场景走读续（§29.1-29.6）
 - 第三十部分 · 完整事件参考（§30.1-30.5）

 ---

 # 第一部分 · 架构总览

 本部分用最短的篇幅帮你建立"全景地图"。读完本部分你应当能回答：
 1. `/review` 到底在做什么？
 2. 它跟 Guardian AutoReview 是什么关系？
 3. 一次 `/review` 从敲下回车到看到结果，大致经过哪些阶段？
 4. 涉及哪些核心类型和文件？

 ---

 ## 1.1 `/review` 是什么、解决什么问题、与 Guardian AutoReview 的区别

 ### 1.1.1 一句话定义

 `/review` 是 Agere CLI 中的一个斜杠命令，它会让 Agent **以"代码审查者"身份启动一个隔离的子代理（sub-agent）**，用专门的审查 prompt 对你当前代码库中的改动进行审查，产出**结构化的审查结论**（findings），然后把结论回灌到主会话中供你查看和后续处理。

 ### 1.1.2 解决什么问题

 在没有 `/review` 之前，如果你想让 Agent 帮你审查代码改动，你得手动在对话里说"请帮我 review 一下当前的改动"。这样做有几个问题：

 1. **没有结构化输出**：Agent 的回复是自由文本，你很难程序化地逐条处理。
 2. **没有隔离**：审查过程和主对话混在一起，审查时的中间推理、工具调用会污染主会话上下文。
 3. **没有专门的审查视角**：普通对话中 Agent 是"执行者"身份，而不是"审查者"身份，prompt 也不针对审查优化。
 4. **难以针对不同 target**：你想审查"未提交改动"vs"相对某分支的 diff"vs"某个 commit"，需要手动组织 git 命令。

 `/review` 解决了以上所有问题：

 - **结构化输出**：子代理被要求输出严格匹配 `ReviewOutputEvent` JSON schema 的结果（含 findings 列表、overall_correctness、overall_confidence_score 等）。
 - **隔离执行**：审查在独立子代理中运行，禁用 web search、view image 等工具，权限降级为 `Never`（无需审批），不携带主会话历史。
 - **专门审查 prompt**：`core/review_prompt.md` 定义了详尽的审查准则（bug 判定 8 条、评论撰写 8 条、优先级 P0-P3、输出 schema）。
 - **多 target 支持**：`ReviewTarget` 枚举支持 `UncommittedChanges`、`BaseBranch`、`Commit`、`Custom` 四种审查目标，每种自动生成对应的 git diff prompt。

 ### 1.1.3 与 Guardian AutoReview 的区别

 代码库中有两个名字里带"review"的概念，新手极易混淆。这里一次性讲清：

 **概念 A：`/review` 斜杠命令（本文档主题）**

 - **触发方式**：用户在 TUI 中输入 `/review` 或通过 `Op::Review` 编程提交。
 - **审查对象**：代码改动（working tree / branch diff / commit）。
 - **执行者**：一个隔离的子代理（`SubAgentSource::Review`）。
 - **输出**：`ReviewOutputEvent`（结构化 findings）。
 - **关键文件**：`core/src/tasks/review.rs`、`core/src/session/review.rs`、`core/src/review_prompts.rs`。
 - **生命周期事件**：`EnteredReviewMode` → `ExitedReviewMode`。

 **概念 B：Guardian 审查 / AutoReview（关联但独立）**

 - **触发方式**：当 Agent 要执行某个需要审批的操作（如 shell 命令、apply_patch、网络访问）时，由 Guardian 机制自动触发。
 - **审查对象**：审批请求（approval request），即"这个操作是否应该被放行"。
 - **执行者**：Guardian 审查会话（`core/src/guardian/review.rs`、`core/src/guardian/review_session.rs`）。
 - **输出**：`ReviewDecision`（Approved / ApprovedExecpolicyAmendment / ApprovedForSession / NetworkPolicyAmendment / Abort）。
 - **关键配置**：`approvals_reviewer`（`User` 或 `AutoReview`）。
 - **`/autoreview` 命令**：当 AutoReview 拒绝了一个操作后，用户可用 `/autoreview` 批准一次重试（见 `tui/src/auto_review_denials.rs` 中的 `RecentAutoReviewDenials`）。

 **核心区别一览表：**

 | 维度 | `/review` 斜杠命令 | Guardian AutoReview |
 |------|---------------------|---------------------|
 | 审查对象 | 代码改动 | 审批请求（操作放行） |
 | 触发者 | 用户主动 | 系统自动（审批触发） |
 | 执行者 | 隔离子代理 | Guardian 审查会话 |
 | 输出类型 | ReviewOutputEvent | ReviewDecision |
 | 生命周期事件 | Entered/ExitedReviewMode | GuardianAssessmentEvent |
 | 关键 prompt | core/review_prompt.md | guardian 内部 prompt |
 | 权限模型 | Never（无需审批） | 取决于配置 |
 | 工具可用性 | web_search/view_image 禁用 | 取决于配置 |

 **一句话记忆法**：`/review` 审的是"代码写得对不对"，Guardian AutoReview 审的是"这个操作能不能执行"。

 ---

 ## 1.2 全局架构图

 ### 1.2.1 Mermaid 架构图

 ``**架构图（ASCII 版本见下方 §1.2.2）**

> 因 EPUB/MD 渲染器对 Mermaid 支持不一致，此处使用 ASCII 图表。完整架构关系见下方 ASCII 架构图。``

 ### 1.2.2 ASCII 架构图

 ```
 ┌─────────────────────────────────────────────────────────────────────┐
 │                         用户输入 /review                            │
 └──────────────────────────────┬──────────────────────────────────────┘
                                │
                     ┌──────────▼──────────┐
                     │  TUI: SlashCommand   │  tui/src/slash_command.rs
                     │  ::Review 解析        │
                     └──────────┬──────────┘
                                │ Op::Review { review_request }
                     ┌──────────▼──────────┐
                     │  Session Handler     │  core/src/session/handlers.rs
                     │  review()            │
                     └──────────┬──────────┘
                                │ resolve_review_request()
                     ┌──────────▼──────────┐
                     │  Review Prompts      │  core/src/review_prompts.rs
                     │  生成审查 prompt      │  (git merge-base / diff)
                     └──────────┬──────────┘
                                │ ResolvedReviewRequest
                     ┌──────────▼──────────┐
                     │  Session::review     │  core/src/session/review.rs
                     │  spawn_review_thread │  构建 TurnContext (隔离)
                     └──────────┬──────────┘
                                │ spawn_task(ReviewTask)
                                │ emit EnteredReviewMode
                     ┌──────────▼──────────┐
                     │  ReviewTask::run     │  core/src/tasks/review.rs
                     │  ├ start_review_     │
                     │  │  conversation()   │──┐ run_agere_thread_one_shot
                     │  ├ process_review_   │  │ (子代理: reviewer model)
                     │  │  events()         │◄─┘ Event 流
                     │  └ exit_review_mode()│
                     └──────────┬──────────┘
                                │
                     ┌──────────▼──────────┐
                     │  parse_review_       │  解析 JSON → ReviewOutputEvent
                     │  output_event()      │  失败兜底 → 纯文本
                     └──────────┬──────────┘
                                │
                     ┌──────────▼──────────┐
                     │  exit_review_mode()  │  emit ExitedReviewMode
                     │  + record items      │  record user/assistant msg
                     │  + format findings   │  ensure_rollout_materialized
                     └──────────┬──────────┘
                                │
                     ┌──────────▼──────────┐
                     │  TUI: ChatWidget     │  tui/src/chatwidget.rs
                     │  on_exited_review_   │  渲染 findings / 恢复状态
                     │  mode()              │
                     └─────────────────────┘
 ```

 ### 1.2.3 九层关系速读

 数据流自上而下穿过 9 层，但并非每层都线性串联。理解时记住三条主线：

 **主线 1：命令解析与分发（同步，快）**
 `SlashCommand::Review` → `Op::Review` → `handlers::review()` → `resolve_review_request()` → `spawn_review_thread()`
 这条线在主会话线程上同步执行，负责"准备审查"。

 **主线 2：子代理执行（异步，慢）**
 `ReviewTask::run()` → `start_review_conversation()`（spawn 子代理）→ 子代理跑 reviewer model → `process_review_events()`（消费事件流）→ `parse_review_output_event()`（解析输出）
 这条线在子任务中异步执行，是 `/review` 的核心耗时部分。

 **主线 3：结果回灌与 UI（同步，快）**
 `exit_review_mode()` → emit `ExitedReviewMode` + record conversation items → TUI `on_exited_review_mode()` → 渲染 findings → 恢复 `is_review_mode = false`
 这条线把审查结论回灌到主会话并驱动 UI 更新。

 ---

 ## 1.3 一次 `/review` 的 60 秒速览

 用最短的方式让你建立直觉。假设你在 TUI 中输入 `/review`，以下是接下来发生的事情：

 - **第 0 秒**：`SlashCommand::from_str("review")` 解析出 `SlashCommand::Review`。
 - **第 1 秒**：构造 `ReviewRequest { target: UncommittedChanges }`，提交 `Op::Review`。
 - **第 2 秒**：`handlers::review()` 调 `resolve_review_request()` 生成审查 prompt。
 - **第 3 秒**：`resolve_review_request()` 根据 target 生成 prompt（UncommittedChanges → 固定文本；BaseBranch → 含 merge-base SHA；Commit → 含 sha；Custom → 直接用指令）。
 - **第 4 秒**：`spawn_review_thread()` 构建隔离 TurnContext（禁用 web search、权限 Never、不带 developer/user 指令）。
 - **第 5 秒**：`spawn_task(ReviewTask::new())` + emit `EnteredReviewMode`。
 - **第 6 秒**：TUI 收到 `EnteredReviewMode`，设 `is_review_mode = true`，显示 banner。
 - **第 7-55 秒**：`ReviewTask::run()` → `start_review_conversation()` → `run_agere_thread_one_shot()` 启动子代理。子代理用 reviewer model 执行审查（读 diff、分析代码、生成 findings JSON）。`process_review_events()` 消费事件流，抑制 Delta/ItemCompleted，暂存 AgentMessage，从 TurnComplete 解析输出。
 - **第 56 秒**：`parse_review_output_event()` 三级降级解析（整体 JSON → 子串 JSON → 纯文本兜底）。
 - **第 57 秒**：`exit_review_mode()` 渲染 user message（XML 模板）+ emit `ExitedReviewMode` + record assistant message + `ensure_rollout_materialized()`。
 - **第 58 秒**：TUI `on_exited_review_mode()` 渲染 findings/explanation/error + `exit_review_mode_after_item()` 恢复状态。
 - **第 59-60 秒**：`ReviewTask::run()` 返回，TurnComplete。审查结论已记录在 rollout 中，后续 turn 可引用。

 ---

 ## 1.4 术语表

 按首次出现频率排序，每个术语给出：定义、所在文件、一句话用途。

 - **ReviewRequest**：审查请求，含 `target` 和可选 `user_facing_hint`。`protocol/src/protocol.rs`。`Op::Review` 的载荷。
 - **ReviewTarget**：审查目标 tagged union，4 变体：`UncommittedChanges`、`BaseBranch{branch}`、`Commit{sha,title}`、`Custom{instructions}`。决定生成哪种审查 prompt。
 - **ReviewDelivery**：审查交付方式（`Inline`/`Detached`）。
 - **ReviewOutputEvent**：结构化审查结果，含 `findings: Vec<ReviewFinding>`、`overall_correctness`、`overall_explanation`、`overall_confidence_score`。Default 全空/零值。
 - **ReviewFinding**：单条审查发现，含 `title`、`body`(Markdown)、`confidence_score`(0-1)、`priority`(0-3)、`code_location`。
 - **ReviewCodeLocation**：代码位置，含 `absolute_file_path: PathBuf` 和 `line_range: ReviewLineRange`。
 - **ReviewLineRange**：行范围，含 `start: u32` 和 `end: u32`（inclusive）。
 - **SubAgentSource::Review**：子代理来源标记，序列化为 `"review"`。标记审查子代理身份。
 - **EnteredReviewMode(ReviewRequest)**：事件，`spawn_review_thread()` emit，通知 UI 切换审查模式。
 - **ExitedReviewMode(ExitedReviewModeEvent)**：事件，`exit_review_mode()` emit，携带 `Option<ReviewOutputEvent>`。
 - **ResolvedReviewRequest**：解析后的审查请求，含 `target`、`prompt`、`user_facing_hint`。`core/src/review_prompts.rs`。
 - **ReviewTask**：实现 `SessionTask` trait 的审查任务，审查子代理执行体。`core/src/tasks/review.rs`。
 - **REVIEW_PROMPT**：审查者系统 prompt 常量，来自 `core/review_prompt.md`。`core/src/client_common.rs`。
 - **REVIEW_EXIT_SUCCESS_TMPL / REVIEW_EXIT_INTERRUPTED_TMPL**：退出模板（XML），成功包裹结果，中断提示重运行。
 - **review_model**：配置项，审查子代理模型。未设置时 fallback 主会话模型。
 - **approvals_reviewer**：配置项，Guardian 审批审查者（`User`/`AutoReview`）。
 - **Op::Review**：会话操作枚举的 `Review` 变体，载荷 `review_request: ReviewRequest`。

 ---

 ## 1.5 文件地图

 每个文件一行职责 + 关键行号锚点。

 ### 协议层
 - `protocol/src/protocol.rs`：L827 Op::Review、L1336-1340 Entered/ExitedReviewMode、L1801 ExitedReviewModeEvent、L2798-2882 ReviewDelivery/Target/Request/OutputEvent/Finding/CodeLocation/LineRange、L2552 SubAgentSource::Review

 ### 核心层 — 会话
 - `core/src/session/handlers.rs`：L1002-1024 `review()`、L1210-1211 Op::Review 分发
 - `core/src/session/review.rs`：全文 `spawn_review_thread()` — 构建 TurnContext + spawn_task + emit EnteredReviewMode

 ### 核心层 — 任务
 - `core/src/tasks/review.rs`：L34 ReviewTask、L44-71 run()、L79-115 start_review_conversation()、L117-150 process_review_events()、L155-172 parse_review_output_event()、L177-241 exit_review_mode()

 ### 核心层 — Prompt
 - `core/review_prompt.md`：审查者系统 prompt（bug 判定 8 条、评论 8 条、优先级 P0-P3、输出 JSON schema）
 - `core/src/review_prompts.rs`：L47 resolve_review_request()、L74 review_prompt()、L123 user_facing_hint()
 - `core/src/client_common.rs`：L20 REVIEW_PROMPT、L23-25 退出模板常量
 - `core/templates/review/exit_success.xml`：成功退出模板
 - `core/templates/review/exit_interrupted.xml`：中断退出模板

 ### 核心层 — 格式化
 - `core/src/review_format.rs`：L33 format_review_findings_block()、L73 render_review_output_text()

 ### TUI 层
 - `tui/src/slash_command.rs`：L26-30 Review/AutoReview 变体、L144-148 supports_inline_args
 - `tui/src/chatwidget.rs`：L1022-1025 is_review_mode/pre_review_token_info、L8105-8171 审查模式函数
 - `tui/src/auto_review_denials.rs`：RecentAutoReviewDenials（最多 10 条）

 ### App-Server / 配置 / 测试
 - `app-server-protocol/src/protocol/v2.rs`：L155 NonSteerableTurnKind::Review、L328 ApprovalsReviewer
 - `config/src/config_toml.rs`：review_model / approvals_reviewer
 - `core/tests/suite/review.rs`：8 个集成测试

 ---

 > **一句话回顾**：`/review` 是一个隔离的子代理审查流程，从 TUI 命令解析出发，经协议层构造请求、会话层构建隔离上下文、任务层执行子代理并解析结构化输出，最终回灌结果并驱动 UI——Guardian AutoReview 是另一套审批审查机制，二者不要混淆。

 ---

 # 第二部分 · 层参考手册

 本部分逐层拆解 `/review` 涉及的 9 个架构层。每层遵循统一模板：①职责一句话 ②核心类型/结构体清单 ③公开 API ④内部数据流伪代码 ⑤关键代码引用（文件:行）⑥易踩坑点 ⑦迷你示例。

 ---

 ## 2.1 斜杠命令层

 ### 职责
 解析用户在 TUI 中输入的 `/review`（及 `/autoreview`）文本，转换为 `Op::Review` 操作并提交到会话。

 ### 核心类型
 `SlashCommand::Review` — 对应 `/review`，审查代码改动。`SlashCommand::AutoReview` — 对应 `/autoreview`，批准 AutoReview 拒绝后的重试。

 ### 公开 API
 - `SlashCommand::from_str(s)` — 从字符串解析斜杠命令
 - `command()` — 返回命令字符串（如 `"review"`）
 - `description()` — 返回描述文本
 - `supports_inline_args()` — 对 Review 返回 `true`（L144-148）

 ### 内部数据流伪代码
 ```
 function on_user_input(text):
     if text starts with "/":
         command, args = parse_slash_command(text)
         match command:
             Review => build_review_request(args) → session.submit(Op::Review)
             AutoReview => handle_autoreview()

 function build_review_request(args):
     if args empty → UncommittedChanges
     elif args is branch → BaseBranch { branch: args }
     elif args is SHA → Commit { sha: args, title: None }
     else → Custom { instructions: args }
 ```

 ### 关键代码引用
 - `tui/src/slash_command.rs:26-30` — AutoReview/Review 变体
 - `tui/src/slash_command.rs:85` — Review 描述
 - `tui/src/slash_command.rs:144-148` — supports_inline_args

 ### 易踩坑点
 1. `/review` 和 `/autoreview` 是两个不同命令，不要混淆
 2. inline args 的解析不在 SlashCommand 层完成，`supports_inline_args()` 只声明支持
 3. `AutoReview` 的 `to_string` 是 `"autoreview"`（无下划线）

 ### 迷你示例
 ```
 /review        → ReviewRequest { target: UncommittedChanges }
 /review main   → ReviewRequest { target: BaseBranch { branch: "main" } }
 /autoreview    → 从 RecentAutoReviewDenials 取出拒绝记录并批准重试
 ```

 ---

 ## 2.2 协议层

 ### 职责
 定义审查相关的所有跨层共享类型（请求、输出、事件、子代理来源标记），提供 serde 序列化和 TypeScript schema 生成。

 ### 核心类型

 **请求类型：**
 - `ReviewDelivery`（snake_case）：`Inline` / `Detached`
 - `ReviewTarget`（tag="type", camelCase）：`UncommittedChanges` | `BaseBranch{branch}` | `Commit{sha,title?}` | `Custom{instructions}`
 - `ReviewRequest`：`target: ReviewTarget` + `user_facing_hint: Option<String>`（skip_serializing_if None）

 **输出类型：**
 - `ReviewOutputEvent`：`findings: Vec<ReviewFinding>` + `overall_correctness: String` + `overall_explanation: String` + `overall_confidence_score: f32`。Default 全空/零值。
 - `ReviewFinding`：`title` + `body`(Markdown) + `confidence_score`(f32) + `priority`(i32) + `code_location`
 - `ReviewCodeLocation`：`absolute_file_path: PathBuf` + `line_range: ReviewLineRange`
 - `ReviewLineRange`：`start: u32` + `end: u32`（inclusive）

 **事件类型：**
 - `EnteredReviewMode(ReviewRequest)` — L1337
 - `ExitedReviewMode(ExitedReviewModeEvent)` — L1340
 - `ExitedReviewModeEvent`：`review_output: Option<ReviewOutputEvent>`

 **子代理来源标记：**
 - `SubAgentSource::Review` — 序列化为 `"review"`（L2552）

 **操作类型：**
 - `Op::Review { review_request: ReviewRequest }` — L828

 **Guardian 相关（关联对比）：**
 - `ApprovalsReviewer`：`User` / `AutoReview`
 - `ReviewDecision`：`Approved` / `ApprovedExecpolicyAmendment` / `ApprovedForSession` / `NetworkPolicyAmendment` / `Abort`

 ### 易踩坑点
 1. `ReviewTarget` 是内部标签 union（`#[serde(tag = "type")]`），JSON 中用 `"type"` 字段区分子类
 2. `ReviewOutputEvent` 的 `Default` 是全空/零值，findings 是空 vec 而非 None
 3. `overall_confidence_score` 是 `f32`，测试中直接 `assert_eq!` 比较精确值
 4. `ExitedReviewModeEvent.review_output` 是 `Option`：None = 中断，Some = 成功（但 Some(全空) 也会触发 "failed" 错误）
 5. `ReviewTarget::Custom` 的 instructions 不能为空——校验在 resolve 时（`anyhow::bail!`）

 ### 迷你示例
 ```json
 // ReviewRequest (UncommittedChanges)
 { "target": { "type": "uncommittedChanges" } }

 // ReviewOutputEvent (结构化)
 {
   "findings": [{
     "title": "[P1] Buffer overflow",
     "body": "The parse function doesn't check bounds.",
     "confidence_score": 0.9, "priority": 1,
     "code_location": { "absolute_file_path": "/tmp/file.rs", "line_range": {"start": 10, "end": 20} }
   }],
   "overall_correctness": "patch is incorrect",
   "overall_explanation": "Buffer overflow detected.",
   "overall_confidence_score": 0.85
 }
 ```

 ---

 ## 2.3 会话层

 ### 职责
 接收 `Op::Review`，解析审查 prompt，构建隔离的 `TurnContext`，spawn 审查任务，并 emit `EnteredReviewMode` 事件。

 ### 核心类型
 - `handlers::review()` — 审查入口（`handlers.rs:1002`）
 - `spawn_review_thread()` — spawn 审查线程（`session/review.rs`）
 - `ResolvedReviewRequest` — 解析后的审查请求（`review_prompts.rs:11`）

 ### 内部数据流伪代码
 ```
 // handlers::review()
 async function review(sess, sub_id, review_request):
     turn_context = sess.new_default_turn_with_sub_id(sub_id)
     sess.maybe_emit_unknown_model_warning_for_turn(turn_context)
     sess.refresh_mcp_servers_if_requested(turn_context)
     match resolve_review_request(review_request, &turn_context.cwd):
         Ok(resolved) => spawn_review_thread(sess, turn_context, sub_id, resolved)
         Err(err) => emit ErrorEvent { message: err.to_string() }

 // spawn_review_thread()
 async function spawn_review_thread(sess, parent_ctx, sub_id, resolved):
     model = config.review_model.unwrap_or(parent_ctx.model_info.slug)
     review_features.disable(WebSearchRequest, WebSearchCached)
     tools_config = ToolsConfig::new(...).with_*(...)  // 15+ 链式配置
     per_turn_config.model = model; per_turn_config.features = review_features
     TurnContext { config: per_turn_config, developer_instructions: None, user_instructions: None, ... }
     input = [UserInput::Text { text: resolved.prompt }]
     sess.spawn_task(tc, input, ReviewTask::new())
     emit EnteredReviewMode(ReviewRequest { target, hint: Some(hint) })
 ```

 ### 关键代码引用
 - `core/src/session/handlers.rs:1002-1024` — `review()` 入口
 - `core/src/session/handlers.rs:1210-1211` — `Op::Review` 分发
 - `core/src/session/review.rs:1-179` — `spawn_review_thread()` 全文
 - `core/src/session/review.rs:17-19` — review_model fallback
 - `core/src/session/review.rs:21-25` — feature 裁剪
 - `core/src/session/review.rs:102-160` — TurnContext 构建
 - `core/src/session/review.rs:162-179` — spawn_task + emit

 ### 易踩坑点
 1. **review_model vs 主模型**：优先 `config.review_model`，没有则 fallback 到主会话模型
 2. **feature 裁剪有两处**：session 层禁 web search，task 层额外禁 SpawnCsv/Collab
 3. **TurnContext.features 用父的**（非裁剪后的 `review_features`），裁剪后的只用于 `per_turn_config` 和 `tools_config`
 4. **审查不携带 developer/user 指令**：`developer_instructions: None`、`user_instructions: None`
 5. **cwd 覆盖生效**：`resolve_review_request` 使用 `turn_context.cwd`（含覆盖）
 6. **resolve 失败 emit Error 而非 panic**

 ### 迷你示例
 ```
 // UncommittedChanges resolve
 target: UncommittedChanges
 prompt: "Review the current code changes (staged, unstaged, and untracked files)..."
 hint: "current changes"

 // BaseBranch resolve (有 merge-base)
 target: BaseBranch { branch: "main" }
 merge_base_sha: "abc123def"
 prompt: "Review the code changes against the base branch 'main'. The merge base commit...abc123def..."
 hint: "changes against 'main'"
 ```

 ---

 ## 2.4 任务层

 ### 职责
 审查流程的核心执行体：启动子代理对话、消费事件流（有选择地抑制/转发）、解析结构化输出、退出审查模式并回灌结果。

 ### 核心类型
 - `ReviewTask`（零大小类型，Clone+Copy）— 实现 `SessionTask` trait
 - `REVIEW_EXIT_SUCCESS_TEMPLATE`（LazyLock）— 退出成功模板

 ### 内部数据流伪代码
 ```
 // ReviewTask::run()
 async function run(session, ctx, input, cancellation_token):
     counter("agere.task.review", 1)
     output = match start_review_conversation(...):
         Some(receiver) => process_review_events(...)
         None => None
     if not cancellation_token.is_cancelled():
         exit_review_mode(session, output, ctx)
     return None

 // start_review_conversation()
 sub_agent_config = config.clone()
 web_search_mode.set(Disabled)  // panic if fails
 features.disable(SpawnCsv, Collab)
 base_instructions = REVIEW_PROMPT
 approval_policy = Never
 model = review_model.unwrap_or(主模型)
 run_agere_thread_one_shot(..., SubAgentSource::Review, None, None)
 → Some(receiver) 或 None

 // process_review_events()
 while let Ok(event) = receiver.recv():
     AgentMessage => 暂存（转发上一条）
     ItemCompleted(AgentMessage) | AgentMessageDelta | AgentMessageContentDelta => 抑制
     TurnComplete => parse last_agent_message → return Some/None
     TurnAborted => return None
     other => 转发
 return None  // channel 关闭

 // parse_review_output_event()
 策略1: serde_json::from_str(整体) → 成功返回
 策略2: find{}/rfind{}/from_str(子串) → 成功返回
 策略3: ReviewOutputEvent { overall_explanation: text, ..Default }

 // exit_review_mode()
 Some(output) => findings_str = explanation + findings_block
                  user_msg = render_exit_success(findings_str)  // XML 模板
                  asst_msg = render_review_output_text(output)  // 纯文本
 None => user_msg = 中断模板; asst_msg = "Review was interrupted..."
 record user message (id: "review_rollout_user")
 emit ExitedReviewMode(review_output)
 record assistant message (id: "review_rollout_assistant")
 ensure_rollout_materialized()

 // abort()
 exit_review_mode(session, None, ctx)
 ```

 ### 关键代码引用
 - `core/src/tasks/review.rs:44-71` — run()
 - `core/src/tasks/review.rs:79-115` — start_review_conversation()
 - `core/src/tasks/review.rs:117-150` — process_review_events()
 - `core/src/tasks/review.rs:155-172` — parse_review_output_event()
 - `core/src/tasks/review.rs:177-241` — exit_review_mode()

 ### 易踩坑点
 1. **AgentMessage 暂存机制**：不立即转发，等下一条或 TurnComplete。只有最后一条含完整 JSON
 2. **事件抑制设计**：抑制 Delta/ItemCompleted(AgentMessage) 避免触发 legacy AgentMessage
 3. **Constrained panic 保证**：task 层 `web_search_mode.set(Disabled)` 失败会 panic
 4. **run() 返回 None**：审查结果通过事件传递，不通过返回值
 5. **exit_review_mode 在 cancel 检查之后**：被取消时 abort() 负责 exit(None)
 6. **模板渲染 panic 策略**：parse/render 都是 `unwrap_or_else(panic!)`
 7. **review_output 的 Option 语义**：Some+有findings=正常；Some+全空="failed"；None=中断

 ---

 ## 2.5 Prompt 层

 ### 职责
 定义审查者系统 prompt（`review_prompt.md`）、根据 target 生成审查 user prompt（`review_prompts.rs`）、退出消息模板（`exit_success.xml`/`exit_interrupted.xml`）。

 ### 核心 prompt 模板
 - `UNCOMMITTED_PROMPT`：固定文本 "Review the current code changes..."
 - `BASE_BRANCH_PROMPT`：含 `{{base_branch}}`/`{{merge_base_sha}}`，有 merge-base 时用
 - `BASE_BRANCH_PROMPT_BACKUP`：含 `{{branch}}`，无 merge-base 时用（含 git 命令示例）
 - `COMMIT_PROMPT`：含 `{{sha}}`，无 title
 - `COMMIT_PROMPT_WITH_TITLE`：含 `{{sha}}`/`{{title}}`，有 title

 ### 退出模板
 - `exit_success.xml`：`<user_action>...<results>{{results}}</results></user_action>`
 - `exit_interrupted.xml`：`<user_action>...interrupted...<results>None.</results></user_action>`

 ### review_prompt.md 要点
 - 角色：审查者（"acting as a reviewer for a proposed code change"）
 - Bug 判定 8 条：实质性影响/离散可操作/严谨度匹配/本次引入/作者会修复/不依赖假设/可证明影响/非故意
 - 评论 8 条：说明原因/恰当严重性/简洁(≤1段)/代码块≤3行/明确触发条件/客观语气/立即可理解/避免奉承
 - 优先级：P0(blocking)/P1(urgent)/P2(normal)/P3(low)，JSON priority 0-3
 - 输出 schema：严格 JSON，不用 markdown fence，code_location 必填

 ### 关键代码引用
 - `core/review_prompt.md` — 审查者系统 prompt 全文
 - `core/src/client_common.rs:20` — REVIEW_PROMPT 常量
 - `core/src/review_prompts.rs:47-72` — resolve_review_request()
 - `core/src/review_prompts.rs:74-103` — review_prompt()
 - `core/templates/review/exit_success.xml` / `exit_interrupted.xml`

 ---

 ## 2.6 格式化层

 ### 职责
 将 `ReviewOutputEvent` 格式化为人类可读的纯文本，供会话记录和 UI 渲染使用。UI 无关（返回纯字符串）。

 ### 公开 API
 - `format_review_findings_block(findings, selection)` — 格式化 findings 列表（支持 checkbox `[x]`/`[ ]` 或简单 bullet `-`）
 - `render_review_output_text(output)` — 渲染审查摘要（explanation + findings 或 fallback）
 - `format_location(item)` — 格式化 `"path:start-end"`

 ### 伪代码
 ```
 format_review_findings_block(findings, selection):
     lines = ["", findings.len()>1 ? "Full review comments:" : "Review comment:"]
     for (idx, item) in findings.enumerate():
         lines.push("")
         location = format_location(item)  // "path:start-end"
         if selection: marker = checked ? "[x]" : "[ ]"; lines.push("- {marker} {title} — {location}")
         else: lines.push("- {title} — {location}")
         for body_line in item.body.lines(): lines.push("  {body_line}")  // 2空格缩进
     return lines.join("\n")

 render_review_output_text(output):
     sections = []
     if explanation non-empty: sections.push(explanation)
     if findings non-empty: sections.push(format_review_findings_block(findings, None).trim())
     if sections empty: return "Reviewer failed to output a response."
     return sections.join("\n\n")
 ```

 ### 关键代码引用
 - `core/src/review_format.rs:18-24` — format_location()
 - `core/src/review_format.rs:33-66` — format_review_findings_block()
 - `core/src/review_format.rs:73-93` — render_review_output_text()

 ### 易踩坑点
 1. UI 无关设计——返回纯字符串，由 TUI 处理样式
 2. selection 越界默认选中（`unwrap_or(true)`）
 3. 单数 vs 复数 header（`len()>1` → "Full review comments:"，否则 "Review comment:"）
 4. body 每行缩进 2 空格
 5. 全空时返回 "Reviewer failed to output a response."

 ---

 ## 2.7 TUI 层

 ### 职责
 在终端 UI 中处理审查模式生命周期事件（进入/退出审查模式）、渲染审查结果、管理 UI 状态（banner、token info 保存/恢复、布局调整）。

 ### 核心字段
 - `is_review_mode: bool`（L1023）— 审查模式标志
 - `pre_review_token_info: Option<Option<TokenUsageInfo>>`（L1025）— 审查前 token 快照（外层=是否在审查模式，内层=审查前 token info）

 ### 核心函数
 - `enter_review_mode_with_hint(hint, from_replay)` — 保存 token + 设标志 + banner
 - `exit_review_mode_after_item()` — flush 三件套 + 设 false + 恢复 token + 完成 banner
 - `on_exited_review_mode(review)` — 渲染 findings/explanation/error
 - `restore_pre_review_token_info()` — take + 恢复 token info

 ### 关键代码引用
 - `tui/src/chatwidget.rs:1022-1025` — is_review_mode / pre_review_token_info
 - `tui/src/chatwidget.rs:8013-8016` — 事件分发
 - `tui/src/chatwidget.rs:8105-8171` — 审查模式函数
 - `tui/src/auto_review_denials.rs` — RecentAutoReviewDenials（最多 10 条，LIFO，去重）

 ### 易踩坑点
 1. token info 保存/恢复：审查子代理 token 消耗不混入主会话统计
 2. replay vs 非 replay：replay 不设 task running
 3. 审查模式下抑制用户消息渲染（XML 模板消息不显示为普通用户输入）
 4. Guardian 审查 UI 与 /review UI 独立（`pending_guardian_review_status` vs `is_review_mode`）
 5. AutoReviewDenials 去重（retain id）+ 截断（10 条）+ LIFO（push_front）

 ---

 ## 2.8 App-Server 层

 ### 职责
 在 app-server v2 协议中定义审查 wire types（`ApprovalsReviewer`、`NonSteerableTurnKind::Review`），负责审查事件转发。

 ### 核心类型
 - `NonSteerableTurnKind::Review`（v2.rs:155）— 不可引导的审查 turn
 - `ApprovalsReviewer`（v2.rs:328）— `User` / `AutoReview`（experimental）
 - `ReviewDecision` → `CommandExecutionApprovalDecision` 映射（Approved→Accept, Abort→Cancel 等）

 ### 易踩坑点
 1. v2 是活跃开发层（"Do not add new API surface area to v1"）
 2. `approvals_reviewer` 是 experimental 字段
 3. "cannot steer a review turn"——审查属于 NonSteerableTurnKind

 ---

 ## 2.9 配置层

 ### 职责
 定义审查配置项（`review_model`、`approvals_reviewer`），提供配置加载、约束校验和默认值。

 ### 核心配置
 - `review_model: Option<String>` — 审查子代理模型（未设置时 fallback 主模型）
 - `approvals_reviewer: Option<ApprovalsReviewer>` — Guardian 审查者
 - `allowed_approvals_reviewers: Option<Vec<ApprovalsReviewer>>` — 允许的审查者约束

 ### 易踩坑点
 1. 两处 `web_search_mode.set(Disabled)` 行为不同：task 层 panic，session 层 warn + 保持原值
 2. `Constrained<T>` 约束系统：`set()` 在值不被允许时返回 Err
 3. profile 可覆盖 `review_model` 等配置

 ---

 > **一句话回顾**：层参考手册逐层拆解了 9 个架构层——斜杠命令（解析）、协议（类型定义）、会话（隔离上下文构建）、任务（核心执行）、Prompt（审查准则+模板）、格式化（UI无关渲染）、TUI（生命周期+状态）、App-Server（wire types）、配置（review_model+约束）。

 ---

 # 第三部分 · 端到端生命周期追踪

 本部分追踪 6 个真实场景，从用户敲下回车到 UI 显示结果，逐层穿越整个 `/review` 流程。每个场景遵循统一模板：①触发条件 ②时序图 ③逐层穿越伪代码 ④真实代码引用 ⑤数据样例 ⑥边界与错误分支。

 ---

 ## 3.1 场景一：正常 `/review`（working tree 改动）全链路

 ### 3.1.1 触发条件
 用户在 TUI 中输入 `/review`（无参数），当前工作目录是一个 git 仓库且有未提交的改动。

 ### 3.1.2 时序图
 ```
用户        TUI       Handler    review_prompts  session::review  ReviewTask   子代理
 |           |           |              |               |              |            |
 |--/review->|           |              |               |              |            |
 |           |--Op::Rev->|              |               |              |            |
 |           |           |--resolve---->|               |              |            |
 |           |           |<-Resolved---|               |              |            |
 |           |           |--spawn_review_thread------>|               |            |
 |           |           |              |              |--spawn_task->|            |
 |           |<-EnteredReviewMode------|              |              |            |
 |           |           |              |              |              |--run_one-->|
 |           |           |              |              |              |            |
 |           |           |              |              |              |<--Event流-|
 |           |           |              |              |              |--parse---->|
 |           |           |              |              |              |            |
 |           |<-ExitedReviewMode(Some)---------------------------------|            |
```

 ### 3.1.3 逐层穿越伪代码
 **阶段 1：命令解析** — `SlashCommand::from_str("review")` → `Op::Review { ReviewRequest { UncommittedChanges } }`

 **阶段 2：分发与解析** — `handlers::review()` → `resolve_review_request()` → `UNCOMMITTED_PROMPT` + `hint = "current changes"` → `ResolvedReviewRequest`

 **阶段 3：构建隔离上下文** — `spawn_review_thread()` → model=review_model/主模型 → disable web search → 构建 ToolsConfig（15+ with_链）→ TurnContext（developer/user_instructions=None）→ `spawn_task(ReviewTask)` → emit `EnteredReviewMode`

 **阶段 4：子代理执行** — `ReviewTask::run()` → `start_review_conversation()`（config clone + REVIEW_PROMPT + Never + Disable SpawnCsv/Collab）→ `run_agere_thread_one_shot(SubAgentSource::Review, None, None)`

 **阶段 5：事件消费与解析** — `process_review_events()`：AgentMessage 暂存、Delta/ItemCompleted 抑制、TurnComplete → `parse_review_output_event()`（三级降级）

 **阶段 6：退出与回灌** — `exit_review_mode(Some(output))`：findings_str = explanation + findings_block → user_msg = XML 模板 → emit `ExitedReviewMode` → record assistant msg → `ensure_rollout_materialized()`

 **阶段 7：UI 渲染** — `on_exited_review_mode()` → `render_review_output_text` → `record_agent_markdown` → `exit_review_mode_after_item()`（is_review_mode=false + 恢复 token + banner）

 ### 3.1.4 数据样例
 ```json
 // 子代理返回的 ReviewOutputEvent
 {
   "findings": [{
     "title": "[P1] Un-padding slices along wrong dimensions",
     "body": "The `unpad` function reverses dimensions...",
     "confidence_score": 0.9, "priority": 1,
     "code_location": { "absolute_file_path": "/home/user/repo/src/model.py", "line_range": {"start": 42, "end": 48} }
   }],
   "overall_correctness": "patch is incorrect",
   "overall_explanation": "The unpadding bug will corrupt outputs...",
   "overall_confidence_score": 0.85
 }
 ```

 ```xml
 <!-- exit_review_mode 渲染的 user message -->
 <user_action>
   <context>User initiated a review task. Here's the full review output from reviewer model...</context>
   <action>review</action>
   <results>
   The unpadding bug will corrupt outputs...
 Full review comments:
 - [P1] Un-padding slices along wrong tensor dimensions — /home/user/repo/src/model.py:42-48
   The `unpad` function reverses dimensions...
   </results>
 </user_action>
 ```

 ### 3.1.5 边界与错误分支
 1. **resolve 失败**：Custom 指令为空 / merge-base 失败 → emit ErrorEvent，不进入审查
 2. **start_review_conversation 返回 None**：run_agere_thread_one_shot Err → exit_review_mode(None) → 中断模板
 3. **非 JSON 输出**：parse 策略 3 兜底 → overall_explanation = 纯文本
 4. **部分 JSON**：parse 策略 2 提取 {} 子串
 5. **审查被取消**：TurnAborted → None → abort() → exit(None)
 6. **channel 关闭**：recv() Err → None → exit(None)

 ---

 ## 3.2 场景二：`/review <base-branch>` diff 审查

 ### 触发条件
 用户输入 `/review main`，当前分支有相对 main 的改动。

 ### 关键差异
 `review_prompt(BaseBranch { "main" }, cwd)` → `merge_base_with_head(cwd, "main")` → 有结果用精确模板（含 SHA），无结果用 backup 模板（含 git 命令示例）。子代理执行 `git diff <merge-base>` 查看改动。

 ### 边界
 1. merge-base 失败 → resolve Err → emit Error
 2. 无 merge-base → backup 模板（子代理自行找）
 3. cwd 覆盖生效（测试 `review_uses_overridden_cwd_for_base_branch_merge_base` 验证）

 ---

 ## 3.3 场景三：`/review <commit>` 单 commit 审查

 ### 触发条件
 用户输入 `/review abc1234`，审查该 commit 引入的改动。

 ### 关键差异
 `review_prompt(Commit { sha, title }, cwd)` → 有 title 用 `COMMIT_PROMPT_WITH_TITLE_TEMPLATE`，无 title 用 `COMMIT_PROMPT_TEMPLATE`。不在 resolve 阶段校验 SHA——SHA 无效由子代理执行 git 命令时发现。

 ---

 ## 3.4 场景四：审查被中断/取消

 ### 触发条件
 用户在审查进行中按下 Ctrl+C，`cancellation_token` 被触发。

 ### 时序图
 ```
用户        ReviewTask              子代理            TUI
 |           |                       |                 |
 |--Ctrl+C->| cancel_token.cancel() |                 |
 |           |                       |                 |
 |           |                       |--TurnAborted-->|
 |           |<----------------------|                 |
 |           |                       |                 |
 |           |--process_events--> return None         |
 |           |--is_cancelled()==true--> skip exit      |
 |           |--abort()--> exit_review_mode(None)     |
 |           |                       |                 |
 |           |--emit ExitedReviewMode(None)--------->  |
 |           |                       | exit_review_mode_after_item()
 |           |                       | is_review_mode=false
 |           |                       | restore token_info
 |           |                       | '<< Code review finished >>'
```

 ### 关键流程
 1. `TurnAborted` → `process_review_events` 返回 None
 2. `is_cancelled()` 为 true → 跳过 run 中的 `exit_review_mode`
 3. `abort()` → `exit_review_mode(None)` 渲染中断模板
 4. UI 显示 "<< Code review finished >>" 但无审查结果

 ### 中断模板
 ```xml
 <user_action>
   <context>User initiated a review task, but was interrupted. If user asks about this, tell them to re-initiate a review with `/review`...</context>
   <action>review</action>
   <results>None.</results>
 </user_action>
 ```

 ### 边界
 1. channel 关闭无 TurnAborted → recv() Err → None（但 is_cancelled 可能为 false → run 中 exit(None)）
 2. 取消在 exit_review_mode 执行中 → 可能双重 exit（幂等）
 3. start_review_conversation 返回 None（非取消）→ exit(None)，abort 不被调用

 ---

 ## 3.5 场景五：`/autoreview`（审批拒绝后重试）

 ### 与 /review 的区别
 `/autoreview` 与 `/review` 完全不同——它是 Guardian 审批拒绝后的重试机制，从 `RecentAutoReviewDenials`（最多 10 条、去重、LIFO）中取出拒绝记录并批准重试，不产出审查结果也不持久化。

 ### 流程
 1. Guardian AutoReview 拒绝操作 → `GuardianAssessmentEvent { status: Denied }` → `RecentAutoReviewDenials.push(event)`
 2. 用户输入 `/autoreview` → `denials.take(&id)` → 批准重试

 ### RecentAutoReviewDenials 机制
 - `push()`：retain 去重（同 id）→ push_front → truncate(10)
 - `take(&id)`：按 id 取出并移除
 - `action_summary()`：格式化操作摘要（Command/Execve/ApplyPatch/NetworkAccess/McpToolCall/RequestPermissions）

 ---

 ## 3.6 场景六：Guardian 审查流（关联对比）

 ### 与 /review 的核心区别
 | 维度 | /review | Guardian 审查 |
 |------|---------|--------------|
 | 触发 | 用户输入 /review | Agent 请求审批操作 |
 | 目的 | 审查代码改动质量 | 审查操作是否安全 |
 | 执行者 | 隔离子代理 (SubAgentSource::Review) | Guardian 审查会话 |
 | 输出 | ReviewOutputEvent | ReviewDecision |
 | 生命周期事件 | Entered/ExitedReviewMode | GuardianAssessmentEvent |
 | UI 状态 | is_review_mode | pending_guardian_review_status |
 | 权限 | Never | 取决于配置 |
 | 配置 | review_model | approvals_reviewer |
 | 关键文件 | tasks/review.rs | guardian/review.rs |

 ### ReviewDecision 枚举
 `Approved` → Accept | `ApprovedExecpolicyAmendment` → AcceptWithAmendment | `ApprovedForSession` → AcceptForSession | `NetworkPolicyAmendment` → AcceptWithNetworkPolicy | `Abort` → Cancel

 ### TUI 中的区分
 `is_guardian_review()` 通过 header 判断（"Reviewing approval request"）。Guardian 审查有独立的 `pending_guardian_review_status`，与 `/review` 的 `is_review_mode` 是两套独立机制。

 ---

 > **一句话回顾**：端到端生命周期追踪覆盖 6 个场景——正常 /review（7 阶段全链路）、BaseBranch（merge-base 计算）、Commit（SHA 模板）、中断（TurnAborted→abort→exit(None)）、/autoreview（RecentAutoReviewDenials 重试）、Guardian 审查（关联对比），含时序图、伪代码、JSON 样例和错误分支。

 ---
 # 第四部分 · 横切关注点

 本部分聚焦那些跨越多个层的关注点——它们不属于任何单一层，但对理解 `/review` 的完整行为至关重要。

 ---

 ## 4.1 配置体系

 ### 4.1.1 review_model 解析链

 `review_model` 是审查子代理使用的模型配置，解析链如下：

 ```
 config.toml
   └─> review_model: Option<String>     // 配置文件中的值
         └─> Config.review_model          // 加载到 Config 结构体
               └─> spawn_review_thread     // 会话层使用
                     └─> model = config.review_model
                           .unwrap_or(parent.model_info.slug)  // fallback
                         └─> start_review_conversation (task 层)
                               └─> model = config.review_model
                                     .unwrap_or(ctx.model_info.slug)  // 再次 fallback
 ```

 **关键点：**

 1. **两处 fallback 逻辑相同**：`spawn_review_thread` 和 `start_review_conversation` 都做 `review_model.unwrap_or(主模型)`。这意味着即使会话层设了 model，task 层也会独立解析（但结果相同，因为都从同一个 config 读）。

 2. **优先级**：`review_model` > 主会话模型。如果配置了 `review_model = "gpt-5.4"`，审查用 gpt-5.4，主对话可以用别的模型。

 3. **模型信息获取**：`models_manager.get_model_info(model, ...)` 会查询模型的能力（如 reasoning、web search 支持），用于构建 `ToolsConfig`。

 **测试验证：**

 ```
 // core/tests/suite/review.rs

 // 测试 1: review_model = "gpt-5.4", 主模型 = "gpt-4.1"
 //   → 请求 body["model"] == "gpt-5.4"
 review_uses_custom_review_model_from_config()

 // 测试 2: review_model = None, 主模型 = "gpt-4.1"
 //   → 请求 body["model"] == "gpt-4.1"
 review_uses_session_model_when_review_model_unset()
 ```

 ### 4.1.2 Feature 禁用链

 审查子代理的 feature 裁剪发生在**两个层**，禁用的 feature 不同：

 **会话层（spawn_review_thread）：**

 ```rust
 // core/src/session/review.rs:21-25
 review_features.disable(Feature::WebSearchRequest);
 review_features.disable(Feature::WebSearchCached);
 review_web_search_mode = WebSearchMode::Disabled;
 ```

 禁用：`WebSearchRequest`、`WebSearchCached`（web search 相关）
 目的：构建 `ToolsConfig` 时不包含 web search 工具。

 **任务层（start_review_conversation）：**

 ```rust
 // core/src/tasks/review.rs:84-91
 sub_agent_config.web_search_mode.set(WebSearchMode::Disabled);
 sub_agent_config.features.disable(Feature::SpawnCsv);
 sub_agent_config.features.disable(Feature::Collab);
 ```

 禁用：`SpawnCsv`、`Collab`（额外禁用）
 目的：子代理执行时不可 spawn CSV 子代理、不可使用协作工具。

 **完整禁用清单：**

 | Feature | 会话层禁用 | 任务层禁用 | 原因 |
 |---------|-----------|-----------|------|
 | WebSearchRequest | ✅ | ✅ (via web_search_mode) | 审查不需要联网搜索 |
 | WebSearchCached | ✅ | ✅ (via web_search_mode) | 同上 |
 | SpawnCsv | ❌ | ✅ | 审查不应 spawn 批量子代理 |
 | Collab | ❌ | ✅ | 审查不应使用协作工具 |

 ### 4.1.3 Constrained 约束系统

 `WebSearchMode` 被 `Constrained<T>` 包装，`ConfigRequirements` 可以限制允许的值：

 ```rust
 // 简化的 Constrained 机制
 pub struct Constrained<T> {
     value: T,
     allowed: Option<Vec<T>>,  // None = 全部允许
 }

 impl<T> Constrained<T> {
     pub fn set(&mut self, value: T) -> Result<(), T> {
         if self.allowed.map_or(true, |allowed| allowed.contains(&value)) {
             self.value = value;
             Ok(())
         } else {
             Err(value)
         }
     }

     pub fn allow_only(value: T) -> Self {
         Contrained { value, allowed: Some(vec![value]) }
     }
 }
 ```

 **两处 set(Disabled) 的不同行为：**

 **任务层（panic）：**

 ```rust
 // core/src/tasks/review.rs:84-88
 if let Err(err) = sub_agent_config
     .web_search_mode
     .set(WebSearchMode::Disabled)
 {
     panic!("by construction Constrained<WebSearchMode> must always support Disabled: {err}");
 }
 ```

 **会话层（warn + 保持原值）：**

 ```rust
 // core/src/session/review.rs:67-73
 if let Err(err) = per_turn_config.web_search_mode.set(review_web_search_mode) {
     let fallback_value = per_turn_config.web_search_mode.value();
     tracing::warn!(
         error = %err,
         ?review_web_search_mode,
         ?fallback_value,
         "review web_search_mode is disallowed by requirements; keeping constrained value"
     );
 }
 ```

 **为什么行为不同？**

 - 任务层的 `sub_agent_config` 是从 `ctx.config` clone 的，预期 `Constrained` 总是允许 `Disabled`（因为 `Constrained::allow_only` 只在显式约束时限制，而 `Disabled` 是最受限的值）。
 - 会话层的 `per_turn_config` 可能受 `ConfigRequirements` 约束，某些部署可能强制要求 web search（虽然不太可能），所以 warn + 保持原值更安全。

 ### 4.1.4 approvals_reviewer 配置

 `approvals_reviewer` 是 Guardian 审查的配置（与 `/review` 关联但独立）：

 ```rust
 // config/src/config_toml.rs
 pub struct ConfigToml {
     pub approvals_reviewer: Option<ApprovalsReviewer>,
     // ...
 }

 // ApprovalsReviewer 枚举
 pub enum ApprovalsReviewer {
     User,       // 用户手动审批
     AutoReview, // Guardian 自动审查
 }
 ```

 **约束校验：**

 ```rust
 // config/src/config_requirements.rs
 pub struct ConfigRequirements {
     pub allowed_approvals_reviewers: Option<Vec<ApprovalsReviewer>>,
     // ...
 }

 // 校验逻辑
 if let Some(allowed) = requirements.allowed_approvals_reviewers {
     if let Some(reviewer) = config.approvals_reviewer {
         if !allowed.contains(&reviewer) {
             return Err("approvals_reviewer not in allowed list");
         }
     }
 }
 ```

 ---

 > **一句话回顾**：配置体系的核心是 `review_model`（两处 fallback 到主模型）和 feature 禁用链（会话层禁 web search、任务层额外禁 SpawnCsv/Collab）；`Constrained<WebSearchMode>` 的 `set(Disabled)` 在任务层 panic、会话层 warn，反映了不同的容错策略。

 ---

 ## 4.2 子代理隔离

 ### 4.2.1 SubAgentSource::Review 标记

 `SubAgentSource::Review` 是审查子代理的身份标记：

 ```rust
 // protocol/src/protocol.rs:2432, 2552
 pub enum SubAgentSource {
     // ...
     Review,
     // ...
 }

 impl Display for SubAgentSource {
     // ...
     SubAgentSource::Review => f.write_str("review"),
     // ...
 }
 ```

 **用途：**

 1. 在 `run_agere_thread_one_shot` 中作为参数传入，标记这是审查子代理
 2. 下游可以通过 `SubAgentSource` 判断子代理类型，调整行为
 3. 序列化为 `"review"` 字符串，用于日志和遥测

 ### 4.2.2 权限降级

 审查子代理的权限被降级为 `Never`（无需审批）：

 ```rust
 // core/src/tasks/review.rs:94
 sub_agent_config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
 ```

 **含义：**

 - `AskForApproval::Never` 表示所有操作都不需要审批
 - `Constrained::allow_only(Never)` 把约束设为"只允许 Never"
 - 这意味着审查子代理执行 git 命令、读文件等操作时不会触发审批请求

 **为什么审查不需要审批？**

 - 审查子代理主要执行只读操作（git diff、读文件）
 - 审查是用户主动触发的，用户已经知道子代理会分析代码
 - 如果审查也需要审批，会打断审查流程

 ### 4.2.3 历史隔离

 审查子代理**不携带主会话历史**：

 ```rust
 // core/src/tasks/review.rs:101-114
 run_agere_thread_one_shot(
     sub_agent_config,
     // ...
     input,                    // 只有审查 prompt
     // ...
     /*initial_history*/ None, // 不携带主会话历史
 )
 ```

 **测试验证：**

 ```
 // core/tests/suite/review.rs: review_input_isolated_from_parent_history

 1. 创建一个有历史记录的会话（user + assistant 消息）
 2. 提交 Op::Review
 3. 检查子代理请求的 input:
    - 包含 environment context（cwd 等）
    - 包含审查 prompt（作为 user message）
    - 不包含主会话的历史消息
 4. 检查 instructions == REVIEW_PROMPT
 ```

 **隔离的设计意图：**

 1. **审查视角独立**：审查子代理不应受主会话上下文影响，避免"因为之前讨论了 X，所以审查时也关注 X"的偏见。
 2. **token 节约**：不携带历史可以减少 token 消耗。
 3. **可复现性**：相同代码改动的审查结果不应因主会话历史不同而不同。

 ### 4.2.4 工具配置隔离

 审查子代理的 `ToolsConfig` 基于裁剪后的 features 重新构建：

 ```rust
 // core/src/session/review.rs:52-100
 let tools_config = ToolsConfig::new(&ToolsConfigParams {
     model_info: &review_model_info,
     features: &review_features,        // 裁剪后的 features
     web_search_mode: Some(review_web_search_mode),  // Disabled
     // ...
 })
 .with_namespace_tools_capability(...)
 .with_image_generation_capability(...)
 .with_web_search_capability(...)       // Disabled → 不包含 web search 工具
 .with_unified_exec_shell_mode_for_session(...)
 .with_web_search_config(None)
 .with_allow_login_shell(...)
 .with_spawn_agent_usage_hint(...)
 .with_spawn_agent_usage_hint_text(...)
 .with_hide_spawn_agent_metadata(...)
 .with_goal_tools_allowed(false)        // 审查不允许 goal 工具
 .with_max_concurrent_threads_per_session(...)
 .with_wait_agent_min_timeout_ms(...)
 .with_agent_type_description(...);
 ```

 **关键隔离点：**

 - `with_web_search_capability(...)` 传入 provider 的 web search 能力，但因为 `web_search_mode = Disabled`，实际不启用
 - `with_goal_tools_allowed(false)` — 审查子代理不允许使用 goal 工具
 - `with_web_search_config(None)` — 不配置 web search

 ### 4.2.5 指令隔离

 审查子代理不携带 developer/user 指令：

 ```rust
 // core/src/session/review.rs:140-141
 developer_instructions: None,
 user_instructions: None,
 ```

 但设置 `base_instructions` 为 `REVIEW_PROMPT`：

 ```rust
 // core/src/tasks/review.rs:93
 sub_agent_config.base_instructions = Some(crate::REVIEW_PROMPT.to_string());
 ```

 **区别：**

 - `developer_instructions` / `user_instructions`：来自 AGENTS.md 和用户配置的指令，主会话有但审查子代理没有
 - `base_instructions`：审查专用的系统 prompt（`REVIEW_PROMPT`），替代了主会话的系统指令

 ---

 > **一句话回顾**：子代理隔离通过 5 重机制实现——`SubAgentSource::Review` 身份标记、`approval_policy = Never` 权限降级、`initial_history = None` 历史隔离、裁剪后的 `ToolsConfig` 工具隔离、`developer/user_instructions = None` + `base_instructions = REVIEW_PROMPT` 指令隔离。

 ---

 ## 4.3 事件流与抑制策略

 ### 4.3.1 事件抑制的完整清单

 `process_review_events` 中被抑制（不转发给主会话）的事件：

 | 事件类型 | 抑制方式 | 原因 |
 |----------|---------|------|
 | `AgentMessageDelta` | `match => {}` | 流式增量，审查不需要实时流 |
 | `AgentMessageContentDelta` | `match => {}` | 内容增量，同上 |
 | `ItemCompleted(AgentMessage)` | `match => {}` | 转发会触发 legacy AgentMessage |

 **被暂存（延迟转发）的事件：**

 | 事件类型 | 处理方式 | 原因 |
 |----------|---------|------|
 | `AgentMessage` | 暂存到 `prev_agent_message` | 等待下一条或 TurnComplete 再处理 |

 **正常转发的事件：**

 所有其他事件（如工具调用相关事件）正常转发给主会话。

 ### 4.3.2 AgentMessage 暂存机制详解

 ```
 // 暂存逻辑

 Event 1: AgentMessage("partial text 1")
   → prev = Some(event_1)  // 暂存，不转发

 Event 2: AgentMessage("partial text 2")
   → 转发 prev (event_1)    // 先转发上一条
   → prev = Some(event_2)   // 暂存当前

 Event 3: TurnComplete { last_agent_message: Some("final JSON") }
   → 不转发 prev (event_2)  // 丢弃中间消息
   → 解析 last_agent_message
   → 返回 ReviewOutputEvent
 ```

 **为什么暂存而不是直接转发？**

 1. **避免中间消息干扰**：子代理可能在过程中产生多条 AgentMessage（如思考过程），只有最后一条（TurnComplete 时的 `last_agent_message`）才包含完整的 JSON 输出。
 2. **结构化输出优先**：审查流程有意用结构化输出（`ReviewOutputEvent`）替代自由文本 AgentMessage。转发中间消息会让 UI 显示不完整的内容。
 3. **测试验证**：`review_does_not_emit_agent_message_on_structured_output` 验证结构化输出时只 emit 一个 AgentMessage。

 ### 4.3.3 ItemCompleted 抑制的原因

 ```rust
 // core/src/tasks/review.rs:134-140
 EventMsg::ItemCompleted(ItemCompletedEvent {
     item: TurnItem::AgentMessage(_),
     ..
 })
 ```

 注释说：
 > "Suppress ItemCompleted only for assistant messages: forwarding it would trigger legacy AgentMessage via as_legacy_events(), which this review flow intentionally hides in favor of structured output."

 **原因：**

 - `ItemCompleted` 事件在转发时会通过 `as_legacy_events()` 转换为 legacy 事件
 - 对于 `TurnItem::AgentMessage`，这会生成 legacy `AgentMessage` 事件
 - 审查流程有意用结构化输出替代 legacy AgentMessage，所以抑制这个转换
 - 注意：只抑制 `AgentMessage` 的 `ItemCompleted`，其他类型（如工具调用）的 `ItemCompleted` 正常转发

 ### 4.3.4 事件流完整时序

 ```
 子代理产生的原始事件流：
 ┌─────────────────────────────────────────────────────┐
 │ 1. AgentMessageContentDelta("Review")               │ → 抑制
 │ 2. AgentMessageContentDelta("ing...")                │ → 抑制
 │ 3. AgentMessageDelta(...)                            │ → 抑制
 │ 4. ItemCompleted(AgentMessage("partial"))            │ → 抑制
 │ 5. AgentMessage("...partial JSON...")                │ → 暂存
 │ 6. (可能更多 Delta/ItemCompleted)                    │ → 抑制
 │ 7. AgentMessage("...complete JSON...")               │ → 转发上一条，暂存当前
 │ 8. TurnComplete { last_agent_message: Some(json) }   │ → 解析并返回
 └─────────────────────────────────────────────────────┘

 主会话收到的事件：
 ┌─────────────────────────────────────────────────────┐
 │ (可能收到转发的 AgentMessage，取决于时序)              │
 │ ExitedReviewMode(Some(ReviewOutputEvent { ... }))    │
 └─────────────────────────────────────────────────────┘
 ```

 ### 4.3.5 测试验证

 **测试 1：`review_filters_agent_message_related_events`**

 ```
 模拟: SSE 流包含 output_text.delta + output_item.done + response.completed
 期望:
   - 不收到 AgentMessageContentDelta
   - 不收到 AgentMessageDelta
   - 收到 EnteredReviewMode
   - 收到 ExitedReviewMode
   - 收到 TurnComplete
 ```

 **测试 2：`review_does_not_emit_agent_message_on_structured_output`**

 ```
 模拟: SSE 流返回结构化 JSON
 期望:
   - 恰好 1 个 AgentMessage 事件
   - 收到 EnteredReviewMode + ExitedReviewMode + TurnComplete
 ```

 ---

 > **一句话回顾**：事件抑制策略的核心是"用结构化输出替代 legacy 流式 AgentMessage"——抑制 Delta 和 ItemCompleted(AgentMessage) 避免触发 legacy 路径，暂存 AgentMessage 只保留最后一条，从 TurnComplete 的 `last_agent_message` 解析 `ReviewOutputEvent`。

 ---

 ## 4.4 遥测与可观测性

 ### 4.4.1 遥测 counter

 ```rust
 // core/src/tasks/review.rs:48-51
 session.session.services.session_telemetry.counter(
     "agere.task.review",
     /*inc*/ 1,
     &[],
 );
 ```

 **用途：** 记录审查任务执行次数，用于监控和统计。

 ### 4.4.2 Tracing span

 ```rust
 // core/src/tasks/review.rs:59-60
 fn span_name(&self) -> &'static str {
     "session_task.review"
 }
 ```

 **用途：** 在 tracing 系统中标记审查任务的 span，便于在分布式追踪中定位审查相关操作。

 ### 4.4.3 TaskKind 标记

 ```rust
 // core/src/tasks/review.rs:55-57
 fn kind(&self) -> TaskKind {
     TaskKind::Review
 }
 ```

 **用途：** 任务分类，用于任务管理和遥测分组。

 ### 4.4.4 可观测性建议

 在调试审查问题时，可以关注：

 1. **`session_task.review` span** — 审查任务的执行 span
 2. **`agere.task.review` counter** — 审查执行次数
 3. **`SubAgentSource::Review`** — 在子代理日志中过滤审查相关
 4. **`ExitedReviewModeEvent.review_output`** — 审查结果（Some = 成功，None = 中断）
 5. **tracing::warn** — `web_search_mode` 约束冲突时的警告
 6. **tracing::error** — UI 层 "Reviewer failed to output a response" 错误

 ---

 > **一句话回顾**：遥测通过 `agere.task.review` counter（计数）、`session_task.review` span（追踪）、`TaskKind::Review`（分类）三重机制提供可观测性；调试时关注 span、counter、SubAgentSource 和 ExitedReviewMode 事件。

 ---

 ## 4.5 错误处理与降级

 ### 4.5.1 错误处理清单

 | 错误场景 | 处理方式 | 用户体验 |
 |----------|---------|---------|
 | resolve_review_request 失败 | emit ErrorEvent | 显示错误消息，不进入审查 |
 | start_review_conversation 返回 None | output = None → exit_review_mode(None) | 显示中断模板 |
 | 子代理返回非 JSON | parse 兜底 → overall_explanation | 显示纯文本 |
 | 子代理返回部分 JSON | 提取 {} 子串解析 | 正常显示 findings |
 | 子代理返回空 | ReviewOutputEvent::default() | "Reviewer failed to output a response" |
 | 审查被取消 | TurnAborted → None → abort → exit(None) | 显示中断模板 |
 | channel 异常关闭 | recv() Err → None → exit(None) | 显示中断模板 |
 | web_search_mode set 失败(task层) | panic | 程序崩溃（设计如此） |
 | web_search_mode set 失败(session层) | warn + 保持原值 | 审查继续，web search 可能未禁用 |
 | 模板渲染失败 | panic | 程序崩溃（设计如此） |

 ### 4.5.2 JSON 解析的三级降级策略

 ```
 parse_review_output_event(text):

 策略 1: 整体 JSON 解析
   serde_json::from_str::<ReviewOutputEvent>(text)
   → 成功: 返回解析结果
   → 失败: 继续

 策略 2: 提取子串解析
   start = text.find('{')
   end = text.rfind('}')
   if start < end:
       slice = text[start..=end]
       serde_json::from_str::<ReviewOutputEvent>(slice)
       → 成功: 返回解析结果
       → 失败: 继续

 策略 3: 纯文本兜底
   ReviewOutputEvent {
       overall_explanation: text.to_string(),
       ..Default::default()  // findings=[], correctness="", confidence=0.0
   }
 ```

 **测试验证：**

 - `review_op_emits_lifecycle_and_review_output` — 策略 1（完整 JSON）
 - `review_op_with_plain_text_emits_review_fallback` — 策略 3（纯文本兜底）

 ### 4.5.3 模板渲染的 panic 策略

 ```rust
 // 所有模板的 parse 和 render 都是 panic on error

 static REVIEW_EXIT_SUCCESS_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
     Template::parse(normalized.as_ref())
         .unwrap_or_else(|err| panic!("review exit success template must parse: {err}"))
 });

 fn render_review_exit_success(results: &str) -> String {
     REVIEW_EXIT_SUCCESS_TEMPLATE
         .render([("results", results)])
         .unwrap_or_else(|err| panic!("review exit success template must render: {err}"))
 }
 ```

 **设计理由：**

 - 模板是编译时常量（`include_str!`），语法错误应在开发时发现
 - 如果模板有语法错误，这是 bug 而非运行时错误，panic 比静默失败更好
 - 模板变量（如 `{{results}}`）由代码控制，不会出现未知变量

 ### 4.5.4 "Reviewer failed to output a response" 场景

 当 `ReviewOutputEvent` 为全空（`Default::default()`）时：

 ```
 exit_review_mode:
   findings_str = "".trim() = ""  (空)
   findings.is_empty() = true → 不追加 findings block
   user_message = render_review_exit_success("")  // 空结果
   assistant_message = render_review_output_text(default)
     → explanation = "" (空)
     → findings.is_empty() = true
     → sections.is_empty() = true
     → return REVIEW_FALLBACK_MESSAGE = "Reviewer failed to output a response."

 TUI on_exited_review_mode:
   review_output = Some(default)
   findings.is_empty() = true
   explanation = "".trim() = "" (空)
   → tracing::error!("Reviewer failed to output a response.")
   → add_to_history(new_error_event("Reviewer failed to output a response."))
 ```

 ---

 > **一句话回顾**：错误处理采用分级降级——resolve 失败 emit Error、启动失败/取消/channel 关闭走中断模板、JSON 解析三级降级（整体→子串→纯文本兜底）、模板渲染 panic（设计为编译时保证）；全空输出触发 "Reviewer failed to output a response" 错误提示。

 ---

 ## 4.6 持久化

 ### 4.6.1 ensure_rollout_materialized

 ```rust
 // core/src/tasks/review.rs:240-241
 // Review turns can run before any regular user turn, so explicitly
 // materialize rollout persistence. Do this after emitting review output so
 // file creation + git metadata collection cannot delay client-facing items.
 session.ensure_rollout_materialized().await;
 ```

 **为什么需要显式调用？**

 1. **审查 turn 可能在常规 turn 之前运行**：如果用户第一次操作就是 `/review`，rollout 文件可能尚未创建。
 2. **确保审查结果被持久化**：`record_conversation_items` 和 `record_response_item_and_emit_turn_item` 记录到内存，`ensure_rollout_materialized` 把它们写入磁盘。
 3. **延迟到 emit 之后**：注释明确说"Do this after emitting review output so file creation + git metadata collection cannot delay client-facing items"。即先让用户看到结果（emit），再做磁盘 I/O。

 ### 4.6.2 Conversation Items 记录

 `exit_review_mode` 记录两条 conversation items：

 **User Message（id: "review_rollout_user"）：**

 ```rust
 ResponseItem::Message {
     id: Some("review_rollout_user".to_string()),
     role: "user".to_string(),
     content: vec![ContentItem::InputText { text: user_message }],
     phase: None,
 }
 ```

 内容：成功时为 XML 模板（`<user_action>` 包裹审查结果），中断时为中断模板。

 **Assistant Message（id: "review_rollout_assistant"）：**

 ```rust
 ResponseItem::Message {
     id: Some("review_rollout_assistant".to_string()),
     role: "assistant".to_string(),
     content: vec![ContentItem::OutputText { text: assistant_message }],
     phase: None,
 }
 ```

 内容：成功时为 `render_review_output_text` 渲染的纯文本，中断时为 "Review was interrupted..."。

 ### 4.6.3 后续 turn 的历史引用

 审查结果记录在 rollout 后，后续的常规 turn 可以引用：

 ```
 // 测试: review_history_surfaces_in_parent_session

 1. 运行 /review → 产出审查结果 → 记录到 rollout
 2. 提交常规 Op::UserInput("back to parent")
 3. 检查第二个请求的 input:
    - 包含 "User initiated a review task." (user message)
    - 包含 "review assistant output" (assistant message)
    - 最后一条是 "back to parent" (当前 user input)
 ```

 **关键点：**

 - 审查的 user message（XML 模板）和 assistant message（纯文本）都出现在后续 turn 的 input 中
 - 这让 Agent 在后续对话中可以引用审查结果
 - XML 模板格式（`<user_action>`）让 Agent 理解这是审查结果而非普通用户输入

 ### 4.6.4 rollout 文件格式

 rollout 文件是 JSONL 格式，每行一个 JSON 对象：

 ```json
 {"timestamp":"2024-01-01T00:00:00.000Z","type":"session_meta","payload":{...}}
 {"timestamp":"...","type":"response_item","payload":{"type":"message","role":"user","id":"review_rollout_user","content":[{"type":"input_text","text":"<user_action>..."}]}}
 {"timestamp":"...","type":"response_item","payload":{"type":"message","role":"assistant","id":"review_rollout_assistant","content":[{"type":"output_text","text":"..."}]}}
 ```

 **类型：**

 - `session_meta`：会话元数据（id、timestamp、cwd、model 等）
 - `response_item`：会话项（消息、工具调用等）

 ---

 > **一句话回顾**：持久化通过 `ensure_rollout_materialized`（延迟到 emit 后执行，确保审查结果写入磁盘）和两条 conversation items（user message 用 XML 模板、assistant message 用纯文本）实现；后续 turn 可以引用审查结果，让 Agent 理解 XML 格式的审查输出。

 ---

 ## 4.7 测试体系

 ### 4.7.1 测试文件清单

 | 文件 | 类型 | 覆盖内容 |
 |------|------|---------|
 | `core/tests/suite/review.rs` | 集成测试 | 8 个端到端审查测试 |
 | `tui/src/chatwidget/tests/review_mode.rs` | TUI 测试 | 审查模式 UI 渲染 |
 | `app-server/tests/suite/v2/review.rs` | App-Server 测试 | v2 审查协议 |
 | `core/src/tasks/review.rs` (内联) | 单元测试 | 模板渲染、CRLF 规范化 |
 | `core/src/review_prompts.rs` (内联) | 单元测试 | prompt 模板渲染 |
 | `core/src/review_format.rs` | (如有) | 格式化 |
 | `tui/src/auto_review_denials.rs` (内联) | 单元测试 | AutoReview 拒绝记录 |

 ### 4.7.2 核心集成测试详解

 **测试 1：`review_op_emits_lifecycle_and_review_output`**

 ```
 目的: 验证完整审查生命周期 + 结构化输出
 设置:
   - mock SSE 返回 JSON ReviewOutputEvent
   - 提交 Op::Review { Custom { instructions } }
 验证:
   1. 收到 EnteredReviewMode
   2. 收到 ExitedReviewMode(Some(review_output))
   3. review_output == 预期的 ReviewOutputEvent（deep compare）
   4. 收到 TurnComplete
   5. rollout 中有:
      - user header "full review output from reviewer model"
      - finding line "- Prefer Stylize helpers — /tmp/file.rs:10-20"
      - assistant plain text (render_review_output_text)
      - 不含 <user_action> markup（assistant 消息中不应有 XML）
 ```

 **测试 2：`review_op_with_plain_text_emits_review_fallback`**

 ```
 目的: 验证纯文本输出的兜底处理
 设置:
   - mock SSE 返回 "just plain text"（非 JSON）
 验证:
   1. 收到 ExitedReviewMode(Some(review_output))
   2. review_output.overall_explanation == "just plain text"
   3. review_output.findings == [] (空)
   4. review_output == ReviewOutputEvent { overall_explanation: "...", ..Default }
 ```

 **测试 3：`review_filters_agent_message_related_events`**

 ```
 目的: 验证事件抑制
 设置:
   - mock SSE 包含 output_text.delta + output_item.done
 验证:
   1. 不收到 AgentMessageContentDelta（panic if surfaced）
   2. 不收到 AgentMessageDelta（panic if surfaced）
   3. 收到 EnteredReviewMode + ExitedReviewMode + TurnComplete
 ```

 **测试 4：`review_does_not_emit_agent_message_on_structured_output`**

 ```
 目的: 验证结构化输出时 AgentMessage 数量
 设置:
   - mock SSE 返回结构化 JSON
 验证:
   1. 恰好 1 个 AgentMessage 事件
   2. 收到 EnteredReviewMode + ExitedReviewMode + TurnComplete
 ```

 **测试 5：`review_uses_custom_review_model_from_config`**

 ```
 目的: 验证 review_model 配置生效
 设置:
   - config.model = "gpt-4.1"
   - config.review_model = "gpt-5.4"
 验证:
   - 请求 body["model"] == "gpt-5.4"
 ```

 **测试 6：`review_uses_session_model_when_review_model_unset`**

 ```
 目的: 验证 review_model 未设置时 fallback
 设置:
   - config.model = "gpt-4.1"
   - config.review_model = None
 验证:
   - 请求 body["model"] == "gpt-4.1"
 ```

 **测试 7：`review_input_isolated_from_parent_history`**

 ```
 目的: 验证历史隔离
 设置:
   - 创建有历史记录的会话（resume from rollout file）
   - 提交 Op::Review
 验证:
   1. 请求 input 包含 environment context（cwd）
   2. 请求 input 包含审查 prompt
   3. 请求 input 不包含主会话历史
   4. instructions == REVIEW_PROMPT
   5. rollout 中有中断消息（因为 mock 只返回 response.completed）
 ```

 **测试 8：`review_history_surfaces_in_parent_session`**

 ```
 目的: 验证审查结果在后续 turn 中可见
 设置:
   - 运行 /review（产出审查结果）
   - 提交 Op::UserInput("back to parent")
 验证:
   1. 第二个请求的 input 包含审查 user message
   2. 第二个请求的 input 包含审查 assistant message
   3. 最后一条是 "back to parent"
 ```

 **测试 9：`review_uses_overridden_cwd_for_base_branch_merge_base`**

 ```
 目的: 验证 cwd 覆盖对 merge-base 计算的影响
 设置:
   - 创建临时 git 仓库
   - 通过 Op::OverrideTurnContext 覆盖 cwd
   - 提交 Op::Review { BaseBranch { "main" } }
 验证:
   - 请求 input 包含 merge-base SHA
 ```

 ### 4.7.3 单元测试

 **模板渲染测试（core/src/tasks/review.rs）：**

 ```rust
 #[test]
 fn render_review_exit_success_replaces_results_placeholder() {
     assert_eq!(
         render_review_exit_success("Finding A\nFinding B"),
         "<user_action>\n  <context>...\n  <results>\n  Finding A\nFinding B\n  </results>\n  </user_action>\n"
     );
 }

 #[test]
 fn normalize_review_template_line_endings_rewrites_crlf() {
     assert_eq!(
         normalize_review_template_line_endings("<user_action>\r\n  <results>\r\n  None.\r\n"),
         "<user_action>\n  <results>\n  None.\n"
     );
 }
 ```

 **Prompt 模板测试（core/src/review_prompts.rs）：**

 ```rust
 #[test]
 fn review_prompt_template_renders_base_branch_backup_variant() { ... }

 #[test]
 fn review_prompt_template_renders_base_branch_variant() { ... }

 #[test]
 fn review_prompt_template_renders_commit_variant() { ... }

 #[test]
 fn review_prompt_template_renders_commit_variant_with_title() { ... }
 ```

 **AutoReviewDenials 测试（tui/src/auto_review_denials.rs）：**

 ```rust
 #[test]
 fn keeps_only_ten_most_recent_denials() {
     // push 12 个，验证只保留最新 10 个
 }
 ```

 ### 4.7.4 测试辅助函数

 ```rust
 // core/tests/suite/review.rs 中的辅助函数

 async fn start_responses_server_with_sse(sse_raw, expected_requests) -> (MockServer, ResponseMock)
 // 启动 mock SSE 服务器

 async fn new_conversation_for_server(server, agere_home, mutator) -> Arc<AgereThread>
 // 创建连接到 mock 服务器的会话

 async fn resume_conversation_for_server(server, agere_home, resume_path, mutator) -> Arc<AgereThread>
 // 从 rollout 文件恢复会话
 ```

 ### 4.7.5 测试覆盖矩阵

 | 场景 | 集成测试 | 单元测试 |
 |------|---------|---------|
 | 结构化输出 | ✅ test 1 | - |
 | 纯文本兜底 | ✅ test 2 | - |
 | 事件抑制 | ✅ test 3 | - |
 | AgentMessage 数量 | ✅ test 4 | - |
 | review_model 配置 | ✅ test 5 | - |
 | review_model fallback | ✅ test 6 | - |
 | 历史隔离 | ✅ test 7 | - |
 | 后续 turn 引用 | ✅ test 8 | - |
 | cwd 覆盖 | ✅ test 9 | - |
 | 模板渲染 | - | ✅ |
 | CRLF 规范化 | - | ✅ |
 | Prompt 模板 | - | ✅ (4 个) |
 | AutoReview denials | - | ✅ |

 ---

 > **一句话回顾**：测试体系包含 9 个集成测试（覆盖结构化输出、纯文本兜底、事件抑制、model 配置、历史隔离、后续引用、cwd 覆盖）和多个单元测试（模板渲染、CRLF、prompt 模板、denials），使用 mock SSE 服务器和 `core_test_support` 工具链。

 ---

  > **一句话回顾**：测试体系包含 9 个集成测试（覆盖结构化输出、纯文本兜底、事件抑制、model 配置、历史隔离、后续引用、cwd 覆盖）和多个单元测试（模板渲染、CRLF、prompt 模板、denials），使用 mock SSE 服务器和 `core_test_support` 工具链。
 ---

  ---

 # 第五部分 · 附录

 本部分提供快速查阅的参考资料和扩展指南。

 ---

 ## 5.1 完整类型目录

 以下列出审查相关的所有 Rust 类型，含字段表和 serde 注解。

 ### ReviewDelivery

 ```rust
 // protocol/src/protocol.rs:2798-2805
 #[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
 #[serde(rename_all = "snake_case")]
 pub enum ReviewDelivery {
     Inline,
     Detached,
 }
 ```

 | 变体 | serde 值 | 说明 |
 |------|---------|------|
 | Inline | `"inline"` | 内联交付（当前主流程） |
 | Detached | `"detached"` | 分离交付 |

 **serde 注解：** `rename_all = "snake_case"`

 ### ReviewTarget

 ```rust
 // protocol/src/protocol.rs:2806-2830
 #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, TS)]
 #[serde(tag = "type", rename_all = "camelCase")]
 #[ts(tag = "type")]
 pub enum ReviewTarget {
     UncommittedChanges,
     #[serde(rename_all = "camelCase")]
     #[ts(rename_all = "camelCase")]
     BaseBranch { branch: String },
     #[serde(rename_all = "camelCase")]
     #[ts(rename_all = "camelCase")]
     Commit { sha: String, title: Option<String> },
     #[serde(rename_all = "camelCase")]
     #[ts(rename_all = "camelCase")]
     Custom { instructions: String },
 }
 ```

 | 变体 | 字段 | serde JSON | 说明 |
 |------|------|-----------|------|
 | UncommittedChanges | (无) | `{"type":"uncommittedChanges"}` | 审查 working tree |
 | BaseBranch | branch: String | `{"type":"baseBranch","branch":"main"}` | 审查相对 base 分支 |
 | Commit | sha: String, title: Option\<String\> | `{"type":"commit","sha":"abc","title":"Fix"}` | 审查单 commit |
 | Custom | instructions: String | `{"type":"custom","instructions":"..."}` | 自定义指令 |

 **serde 注解：** `tag = "type"`（内部标签），`rename_all = "camelCase"`

 ### ReviewRequest

 ```rust
 // protocol/src/protocol.rs:2831-2838
 #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
 pub struct ReviewRequest {
     pub target: ReviewTarget,
     #[serde(skip_serializing_if = "Option::is_none")]
     #[ts(optional)]
     pub user_facing_hint: Option<String>,
 }
 ```

 | 字段 | 类型 | serde 注解 | 说明 |
 |------|------|-----------|------|
 | target | ReviewTarget | - | 审查目标 |
 | user_facing_hint | Option\<String\> | `skip_serializing_if = "Option::is_none"`, `ts(optional)` | UI 提示文本 |

 ### ReviewOutputEvent

 ```rust
 // protocol/src/protocol.rs:2839-2857
 #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
 pub struct ReviewOutputEvent {
     pub findings: Vec<ReviewFinding>,
     pub overall_correctness: String,
     pub overall_explanation: String,
     pub overall_confidence_score: f32,
 }

 impl Default for ReviewOutputEvent {
     fn default() -> Self {
         Self {
             findings: Vec::new(),
             overall_correctness: String::default(),
             overall_explanation: String::default(),
             overall_confidence_score: 0.0,
         }
     }
 }
 ```

 | 字段 | 类型 | Default | 说明 |
 |------|------|---------|------|
 | findings | Vec\<ReviewFinding\> | `vec![]` | 审查发现列表 |
 | overall_correctness | String | `""` | 正确性裁决 |
 | overall_explanation | String | `""` | 解释说明 |
 | overall_confidence_score | f32 | `0.0` | 整体置信度 |

 ### ReviewFinding

 ```rust
 // protocol/src/protocol.rs:2859-2867
 #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
 pub struct ReviewFinding {
     pub title: String,
     pub body: String,
     pub confidence_score: f32,
     pub priority: i32,
     pub code_location: ReviewCodeLocation,
 }
 ```

 | 字段 | 类型 | 说明 |
 |------|------|------|
 | title | String | 标题（≤80 字符，含优先级标记） |
 | body | String | Markdown 正文 |
 | confidence_score | f32 | 置信度 0.0-1.0 |
 | priority | i32 | 优先级 0=P0, 1=P1, 2=P2, 3=P3 |
 | code_location | ReviewCodeLocation | 代码位置 |

 ### ReviewCodeLocation

 ```rust
 // protocol/src/protocol.rs:2869-2876
 #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
 pub struct ReviewCodeLocation {
     pub absolute_file_path: PathBuf,
     pub line_range: ReviewLineRange,
 }
 ```

 | 字段 | 类型 | 说明 |
 |------|------|------|
 | absolute_file_path | PathBuf | 文件绝对路径 |
 | line_range | ReviewLineRange | 行范围 |

 ### ReviewLineRange

 ```rust
 // protocol/src/protocol.rs:2878-2882
 #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
 pub struct ReviewLineRange {
     pub start: u32,
     pub end: u32,
 }
 ```

 | 字段 | 类型 | 说明 |
 |------|------|------|
 | start | u32 | 起始行（inclusive） |
 | end | u32 | 结束行（inclusive） |

 ### ExitedReviewModeEvent

 ```rust
 // protocol/src/protocol.rs:1801-1803
 #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
 pub struct ExitedReviewModeEvent {
     pub review_output: Option<ReviewOutputEvent>,
 }
 ```

 | 字段 | 类型 | 说明 |
 |------|------|------|
 | review_output | Option\<ReviewOutputEvent\> | 审查结果（None = 中断） |

 ### ResolvedReviewRequest

 ```rust
 // core/src/review_prompts.rs:11-15
 #[derive(Clone, Debug, PartialEq)]
 pub struct ResolvedReviewRequest {
     pub target: ReviewTarget,
     pub prompt: String,
     pub user_facing_hint: String,
 }
 ```

 | 字段 | 类型 | 说明 |
 |------|------|------|
 | target | ReviewTarget | 审查目标 |
 | prompt | String | 生成的审查 user prompt |
 | user_facing_hint | String | UI 提示文本 |

 **注意：** 无 serde derive（内部类型，不跨层序列化）

 ### ReviewTask

 ```rust
 // core/src/tasks/review.rs:34-36
 #[derive(Clone, Copy)]
 pub(crate) struct ReviewTask;
 ```

 零大小类型，仅作为 `SessionTask` trait 的实现载体。

 ### SubAgentSource::Review

 ```rust
 // protocol/src/protocol.rs:2432, 2552
 pub enum SubAgentSource {
     // ...
     Review,
     // ...
 }

 // Display: "review"
 ```

 ### ReviewDecision (Guardian)

 ```rust
 // protocol/src/protocol.rs:3517-3559
 pub enum ReviewDecision {
     Approved,
     ApprovedExecpolicyAmendment { .. },
     ApprovedForSession,
     NetworkPolicyAmendment { .. },
     Abort,
 }
 ```

 | 变体 | as_str | v2 映射 | 说明 |
 |------|--------|---------|------|
 | Approved | `"approved"` | Accept | 批准 |
 | ApprovedExecpolicyAmendment | `"approved_with_amendment"` | AcceptWithAmendment | 批准+修改 exec policy |
 | ApprovedForSession | `"approved_for_session"` | AcceptForSession | 会话内批准 |
 | NetworkPolicyAmendment | - | AcceptWithNetworkPolicy | 批准+修改网络策略 |
 | Abort | - | Cancel | 拒绝 |

 ### ApprovalsReviewer

 ```rust
 // protocol/src/protocol.rs (core) + app-server-protocol/src/protocol/v2.rs:328-332
 pub enum ApprovalsReviewer {
     User,
     AutoReview,
 }
 ```

 | 变体 | 说明 |
 |------|------|
 | User | 用户手动审批 |
 | AutoReview | Guardian 自动审查 |

 ---

 ## 5.2 伪代码库汇总

 前文所有伪代码的集中索引，按功能分组。

 ### 命令解析

 - `on_user_input(text)` — 2.1.4 斜杠命令解析入口
 - `build_review_request(args)` — 2.1.4 参数到 ReviewRequest 映射

 ### 请求解析

 - `resolve_review_request(request, cwd)` — 2.5.4 审查请求解析
 - `review_prompt(target, cwd)` — 2.5.4 按 target 生成 prompt
 - `user_facing_hint(target)` — 2.5.4 生成 UI 提示

 ### 会话管理

 - `review(sess, sub_id, review_request)` — 2.3.4 审查入口
 - `spawn_review_thread(sess, ctx, sub_id, resolved)` — 2.3.4 spawn 审查线程

 ### 任务执行

 - `ReviewTask::run(session, ctx, input, cancellation_token)` — 2.4.4 主流程
 - `start_review_conversation(session, ctx, input, cancellation_token)` — 2.4.4 起子代理
 - `process_review_events(session, ctx, receiver)` — 2.4.4 事件消费
 - `parse_review_output_event(text)` — 2.4.4 输出解析
 - `exit_review_mode(session, review_output, ctx)` — 2.4.4 退出回灌
 - `ReviewTask::abort(session, ctx)` — 2.4.4 中断处理

 ### 格式化

 - `format_review_findings_block(findings, selection)` — 2.6.4 findings 格式化
 - `render_review_output_text(output)` — 2.6.4 审查摘要渲染
 - `format_location(item)` — 2.6.4 位置格式化

 ### TUI

 - `handle_event(event, from_replay)` — 2.7.4 事件分发
 - `enter_review_mode_with_hint(hint, from_replay)` — 2.7.4 进入审查模式
 - `exit_review_mode_after_item()` — 2.7.4 退出审查模式
 - `on_exited_review_mode(review)` — 2.7.4 退出事件处理
 - `restore_pre_review_token_info()` — 2.7.4 恢复 token info
 - `handle_thread_item(item, from_replay)` — 2.7.4 replay 处理

 ### Guardian (关联)

 - `on_approval_request(action, config)` — 3.6.3 Guardian 审查入口

 ---

 ## 5.3 实战示例集

 ### 示例 1：完整的 UncommittedChanges 审查（带注释）

 ```
 [步骤 1] 用户在 TUI 输入 /review
   → SlashCommand::from_str("review") = Ok(Review)
   → supports_inline_args() = true (无参数)
   → ReviewRequest { target: UncommittedChanges, user_facing_hint: None }
   → session.submit(Op::Review { review_request })

 [步骤 2] handlers::review() 接管
   → turn_context = sess.new_default_turn_with_sub_id(sub_id)
   → resolve_review_request(request, cwd)
     → review_prompt(UncommittedChanges, cwd)
       → UNCOMMITTED_PROMPT = "Review the current code changes..."
     → user_facing_hint = "current changes"
     → ResolvedReviewRequest { target, prompt, hint }

 [步骤 3] spawn_review_thread()
   → model = config.review_model.unwrap_or("gpt-4.1") = "gpt-5.4" (假设配置了)
   → review_features.disable(WebSearchRequest, WebSearchCached)
   → tools_config = ToolsConfig::new(...).with_*()
   → per_turn_config.model = "gpt-5.4"
   → TurnContext { config: per_turn_config, developer_instructions: None, ... }
   → input = [UserInput::Text { text: prompt }]
   → sess.spawn_task(tc, input, ReviewTask::new())
   → emit EnteredReviewMode(ReviewRequest { target, hint: "current changes" })

 [步骤 4] TUI 收到 EnteredReviewMode
   → pre_review_token_info = Some(token_info) (保存)
   → is_review_mode = true
   → banner = ">> Code review started: current changes <<"
   → add_to_history(banner)

 [步骤 5] ReviewTask::run()
   → counter("agere.task.review", 1)
   → start_review_conversation()
     → sub_agent_config.base_instructions = REVIEW_PROMPT
     → approval_policy = Never
     → features.disable(SpawnCsv, Collab)
     → run_agere_thread_one_shot(..., SubAgentSource::Review, None, None)
     → 返回 Some(receiver)

 [步骤 6] 子代理执行
   → 系统指令: REVIEW_PROMPT (审查准则 + 输出 schema)
   → 用户指令: "Review the current code changes..."
   → 子代理执行: git diff, 分析代码, 生成 JSON findings
   → 产生事件流通过 channel 返回

 [步骤 7] process_review_events()
   → 收到 AgentMessageContentDelta → 抑制
   → 收到 AgentMessageDelta → 抑制
   → 收到 ItemCompleted(AgentMessage) → 抑制
   → 收到 AgentMessage(json) → 暂存
   → 收到 TurnComplete { last_agent_message: Some(json) }
     → parse_review_output_event(json)
       → serde_json::from_str → 成功
       → 返回 ReviewOutputEvent { findings: [...], ... }

 [步骤 8] exit_review_mode(Some(output))
   → findings_str = "The unpadding bug..."
   → block = format_review_findings_block(findings, None)
   → findings_str += "\n" + block
   → user_message = render_review_exit_success(findings_str)
     = "<user_action>...<results>...</results></user_action>"
   → assistant_message = render_review_output_text(output)
     = "The unpadding bug...\n\nFull review comments:\n\n- [P1]..."
   → record user message (id: "review_rollout_user")
   → emit ExitedReviewMode(Some(output))
   → record assistant message (id: "review_rollout_assistant")
   → ensure_rollout_materialized()

 [步骤 9] TUI 收到 ExitedReviewMode
   → on_exited_review_mode(review)
   → review_markdown = render_review_output_text(output)
   → record_agent_markdown(review_markdown)
   → flush 三件套
   → findings 非空 → 已在 record_agent_markdown 中处理
   → exit_review_mode_after_item()
     → is_review_mode = false
     → restore_pre_review_token_info() → 恢复 token info
     → banner = "<< Code review finished >>"
     → request_redraw()

 [结果] 用户看到:
   ">> Code review started: current changes <<"
   [审查结果 markdown]
   "<< Code review finished >>"
 ```

 ### 示例 2：BaseBranch 审查被中断（带注释）

 ```
 [步骤 1] 用户输入 /review main
   → ReviewRequest { target: BaseBranch { branch: "main" }, ... }

 [步骤 2] resolve_review_request()
   → merge_base_with_head(cwd, "main") = Ok(Some("abc123"))
   → prompt = "Review the code changes against the base branch 'main'.
       The merge base commit for this comparison is abc123..."
   → hint = "changes against 'main'"

 [步骤 3] spawn_review_thread() → emit EnteredReviewMode
   → TUI: ">> Code review started: changes against 'main' <<"

 [步骤 4] ReviewTask::run() → start_review_conversation()
   → 子代理开始执行 git diff abc123

 [步骤 5] 用户按 Ctrl+C
   → cancellation_token.cancel()
   → 子代理收到取消信号
   → receiver 收到 TurnAborted

 [步骤 6] process_review_events()
   → match TurnAborted => return None

 [步骤 7] run() 检查 is_cancelled()
   → true → 跳过 exit_review_mode

 [步骤 8] abort() 被调用
   → exit_review_mode(session, None, ctx)
   → user_message = REVIEW_EXIT_INTERRUPTED_TMPL
     = "<user_action><context>...interrupted...</context>..."
   → assistant_message = "Review was interrupted. Please re-run /review..."
   → record user message
   → emit ExitedReviewMode(None)
   → record assistant message
   → ensure_rollout_materialized()

 [步骤 9] TUI 收到 ExitedReviewMode(None)
   → on_exited_review_mode: review_output = None → 不渲染结果
   → exit_review_mode_after_item()
   → "<< Code review finished >>"

 [结果] 用户看到:
   ">> Code review started: changes against 'main' <<"
   "<< Code review finished >>"
   (无审查结果，因为被中断)
 ```

 ### 示例 3：纯文本兜底审查（带注释）

 ```
 [步骤 1] 用户输入 /review
   → ReviewRequest { target: UncommittedChanges, ... }

 [步骤 2-5] 同示例 1

 [步骤 6] 子代理返回纯文本（非 JSON）
   → last_agent_message = "The code looks good overall, no major issues found."

 [步骤 7] parse_review_output_event("The code looks good...")
   → 策略 1: serde_json::from_str → 失败（不是 JSON）
   → 策略 2: find('{') = None → 跳过
   → 策略 3: 兜底
   → ReviewOutputEvent {
       findings: [],
       overall_correctness: "",
       overall_explanation: "The code looks good overall...",
       overall_confidence_score: 0.0,
     }

 [步骤 8] exit_review_mode(Some(output))
   → findings_str = "The code looks good overall..." (explanation)
   → findings.is_empty() = true → 不追加 block
   → user_message = render_review_exit_success("The code looks good overall...")
   → assistant_message = render_review_output_text(output)
     = "The code looks good overall..." (只有 explanation)

 [步骤 9] TUI 收到 ExitedReviewMode(Some(output))
   → on_exited_review_mode:
   → review_markdown = "The code looks good overall..."
   → record_agent_markdown(review_markdown)
   → findings.is_empty() = true
   → explanation 非空 → 渲染 explanation 为 AgentMessageCell
   → exit_review_mode_after_item()

 [结果] 用户看到:
   ">> Code review started: current changes <<"
   "The code looks good overall, no major issues found."
   "<< Code review finished >>"
 ```

 ---

 ## 5.4 常见问题 / FAQ

 **Q1: `/review` 和 `/autoreview` 有什么区别？**

 A: `/review` 审查代码改动，产出结构化 findings；`/autoreview` 是 Guardian 审批拒绝后的重试机制，从 `RecentAutoReviewDenials` 取出拒绝记录并批准重试。二者是完全不同的机制，共享的仅是"review"这个名字。

 **Q2: 审查子代理用哪个模型？**

 A: 优先用 `config.review_model`，未设置时用主会话模型（`model_info.slug`）。两处 fallback 逻辑相同（会话层和任务层各做一次）。

 **Q3: 审查子代理能访问主会话历史吗？**

 A: 不能。`run_agere_thread_one_shot` 的 `initial_history` 参数为 `None`，子代理只收到审查 prompt 和 environment context。测试 `review_input_isolated_from_parent_history` 验证了这一点。

 **Q4: 审查结果会保存到 rollout 吗？**

 A: 会。`exit_review_mode` 记录两条 conversation items（user message + assistant message），然后 `ensure_rollout_materialized` 持久化到磁盘。后续 turn 可以引用审查结果。

 **Q5: 审查子代理能联网搜索吗？**

 A: 不能。web search 在两处被禁用（会话层禁 `WebSearchRequest/WebSearchCached`，任务层设 `WebSearchMode::Disabled`）。`web_search_mode.set(Disabled)` 在任务层 panic if fails，在会话层 warn + 保持原值。

 **Q6: 审查子代理需要审批吗？**

 A: 不需要。`approval_policy` 设为 `Constrained::allow_only(AskForApproval::Never)`，所有操作无需审批。

 **Q7: 子代理返回非 JSON 怎么办？**

 A: `parse_review_output_event` 有三级降级策略：整体 JSON 解析 → 提取 {} 子串解析 → 纯文本兜底（放进 `overall_explanation`）。

 **Q8: 审查被中断后会怎样？**

 A: `TurnAborted` → `process_review_events` 返回 None → `is_cancelled()` 为 true 跳过 run 中的 exit → `abort()` 调 `exit_review_mode(None)` 渲染中断模板。UI 显示 "<< Code review finished >>" 但无审查结果。

 **Q9: 为什么有些事件被抑制？**

 A: `AgentMessageDelta`、`AgentMessageContentDelta`、`ItemCompleted(AgentMessage)` 被抑制，因为转发它们会触发 legacy AgentMessage 路径，而审查流程有意用结构化输出替代。`AgentMessage` 被暂存，只保留最后一条。

 **Q10: 如何添加新的 ReviewTarget？**

 A: 见 5.5 扩展指南。

 **Q11: review_prompt.md 可以自定义吗？**

 A: 可以。`REVIEW_PROMPT` 通过 `include_str!("../review_prompt.md")` 编译时包含。修改 `core/review_prompt.md` 后重新编译即可。但注意输出 schema 必须与 `ReviewOutputEvent` 匹配。

 **Q12: 审查结果中的 priority 数字什么意思？**

 A: 0=P0（blocking），1=P1（urgent），2=P2（normal），3=P3（low）。review_prompt.md 要求在 title 开头也用 `[P0]`/`[P1]` 等标记。

 **Q13: Guardian 审查和 /review 共享代码吗？**

 A: 不共享。`/review` 用 `core/src/tasks/review.rs`，Guardian 用 `core/src/guardian/review.rs` + `review_session.rs`。二者是独立的审查机制。

 **Q14: 审查子代理有 developer/user 指令吗？**

 A: 没有。TurnContext 中 `developer_instructions: None` 和 `user_instructions: None`。但设置了 `base_instructions = REVIEW_PROMPT` 作为审查专用系统 prompt。

 **Q15: ensure_rollout_materialized 为什么在 emit 之后？**

 A: 注释说"Do this after emitting review output so file creation + git metadata collection cannot delay client-facing items"。即先让用户看到结果（emit），再做磁盘 I/O（可能较慢）。

 ---

 ## 5.5 扩展指南

 ### 5.5.1 添加新的 ReviewTarget

 假设你想添加 `ReviewTarget::PullRequest { number: u32 }` 来审查 GitHub PR。

 **步骤 1：修改协议层**

 ```rust
 // protocol/src/protocol.rs
 pub enum ReviewTarget {
     // ... 现有变体 ...
     #[serde(rename_all = "camelCase")]
     #[ts(rename_all = "camelCase")]
     PullRequest { number: u32 },
 }
 ```

 **步骤 2：修改 prompt 生成**

 ```rust
 // core/src/review_prompts.rs
 const PR_PROMPT: &str = "Review the code changes in pull request #{{number}}. ...";
 static PR_PROMPT_TEMPLATE: LazyLock<Template> = ...;

 pub fn review_prompt(target: &ReviewTarget, cwd: &AbsolutePathBuf) -> anyhow::Result<String> {
     match target {
         // ... 现有分支 ...
         ReviewTarget::PullRequest { number } => {
             Ok(render_review_prompt(&PR_PROMPT_TEMPLATE, [("number", &number.to_string())]))
         }
     }
 }

 pub fn user_facing_hint(target: &ReviewTarget) -> String {
     match target {
         // ... 现有分支 ...
         ReviewTarget::PullRequest { number } => format!("pull request #{number}"),
     }
 }
 ```

 **步骤 3：修改斜杠命令解析**

 ```rust
 // tui/src/slash_command.rs 或分发逻辑
 // 添加 /review pr <number> 的解析
 ```

 **步骤 4：更新测试**

 ```rust
 // core/src/review_prompts.rs tests
 #[test]
 fn review_prompt_template_renders_pr_variant() {
     assert_eq!(
         review_prompt(&ReviewTarget::PullRequest { number: 42 }, &cwd).unwrap(),
         "Review the code changes in pull request #42. ..."
     );
 }
 ```

 **步骤 5：重新生成 schema**

 ```bash
 just write-config-schema  # 如果影响了 config
 just write-app-server-schema  # 如果影响了 v2 协议
 ```

 ### 5.5.2 添加新的 ReviewFinding 字段

 假设你想给 `ReviewFinding` 添加 `suggestion: Option<String>` 字段。

 **步骤 1：修改协议层**

 ```rust
 // protocol/src/protocol.rs
 pub struct ReviewFinding {
     pub title: String,
     pub body: String,
     pub confidence_score: f32,
     pub priority: i32,
     pub code_location: ReviewCodeLocation,
     pub suggestion: Option<String>,  // 新字段
 }
 ```

 **步骤 2：更新 review_prompt.md**

 在输出 schema 中添加：
 ```json
 {
   "findings": [
     {
       "title": "...",
       "body": "...",
       "confidence_score": 0.9,
       "priority": 1,
       "code_location": { ... },
       "suggestion": "可选的修复建议代码"
     }
   ]
 }
 ```

 **步骤 3：更新格式化层**

 ```rust
 // core/src/review_format.rs
 pub fn format_review_findings_block(findings, selection) -> String {
     // ...
     for (idx, item) in findings.enumerate() {
         // ... 现有逻辑 ...
         if let Some(suggestion) = &item.suggestion {
             lines.push(format!("  Suggestion: {suggestion}"));
         }
     }
 }
 ```

 **步骤 4：更新测试**

 更新 `review_op_emits_lifecycle_and_review_output` 等测试中的 `ReviewFinding` 构造。

 ### 5.5.3 自定义审查 prompt

 修改 `core/review_prompt.md` 即可自定义审查准则。注意事项：

 1. **输出 schema 必须匹配 `ReviewOutputEvent`**：如果改了输出格式，需要同步修改 `ReviewOutputEvent` 和 `parse_review_output_event`。
 2. **`include_str!` 是编译时包含**：修改后需要重新编译。
 3. **模板变量用 `{{var}}`**：退出模板用双花括号，不是 Rust 的 `{}`。
 4. **CRLF 会被规范化**：`normalize_review_template_line_endings` 会把 CRLF 转为 LF。

 ### 5.5.4 修改退出模板

 修改 `core/templates/review/exit_success.xml` 或 `exit_interrupted.xml`：

 1. **保持 `{{results}}` 变量**（exit_success.xml）：`render_review_exit_success` 会渲染这个变量。
 2. **模板语法用 `{{var}}`**：`Template::parse` 使用双花括号。
 3. **parse 和 render 会 panic on error**：模板语法错误会在启动时或渲染时 panic。
 4. **CRLF 会被规范化**。

 ---

 ## 5.6 快速查阅卡

 ### 命令速查

 | 命令 | 作用 | 关键文件 |
 |------|------|---------|
 | `/review` | 审查代码改动 | tasks/review.rs |
 | `/review <branch>` | 审查相对分支的 diff | review_prompts.rs |
 | `/review <sha>` | 审查单 commit | review_prompts.rs |
 | `/autoreview` | 批准 Guardian 拒绝的重试 | auto_review_denials.rs |

 ### 事件速查

 | 事件 | 方向 | 载荷 | 触发位置 |
 |------|------|------|---------|
 | EnteredReviewMode | session → TUI | ReviewRequest | spawn_review_thread |
 | ExitedReviewMode | task → TUI | ExitedReviewModeEvent | exit_review_mode |
 | TurnComplete | 子代理 → task | TaskComplete | 子代理完成 |
 | TurnAborted | 子代理 → task | AbortEvent | 子代理被取消 |
 | AgentMessage | 子代理 → task | AgentMessageEvent | 子代理消息（暂存） |
 | AgentMessageDelta | 子代理 → task | DeltaEvent | 抑制 |
 | AgentMessageContentDelta | 子代理 → task | DeltaEvent | 抑制 |
 | ItemCompleted(AgentMessage) | 子代理 → task | ItemCompletedEvent | 抑制 |
 | Error | handler → TUI | ErrorEvent | resolve 失败 |
 | GuardianAssessmentEvent | Guardian → TUI | GuardianAssessmentEvent | Guardian 审查（关联） |

 ### 函数速查

 | 函数 | 文件 | 用途 |
 |------|------|------|
 | `review()` | handlers.rs:1002 | 审查入口 |
 | `spawn_review_thread()` | session/review.rs | 构建隔离上下文 + spawn |
 | `resolve_review_request()` | review_prompts.rs:47 | 解析请求生成 prompt |
 | `review_prompt()` | review_prompts.rs:74 | 按 target 生成 prompt |
 | `ReviewTask::run()` | tasks/review.rs:44 | 主执行流程 |
 | `start_review_conversation()` | tasks/review.rs:79 | 起子代理 |
 | `process_review_events()` | tasks/review.rs:117 | 消费事件 |
 | `parse_review_output_event()` | tasks/review.rs:155 | 解析输出 |
 | `exit_review_mode()` | tasks/review.rs:177 | 退出回灌 |
 | `format_review_findings_block()` | review_format.rs:33 | 格式化 findings |
 | `render_review_output_text()` | review_format.rs:73 | 渲染摘要 |
 | `enter_review_mode_with_hint()` | chatwidget.rs:8105 | TUI 进入审查 |
 | `on_exited_review_mode()` | chatwidget.rs:8139 | TUI 退出处理 |

 ### 配置速查

 | 配置项 | 类型 | 默认 | 用途 |
 |--------|------|------|------|
 | `review_model` | Option\<String\> | None (fallback 主模型) | 审查子代理模型 |
 | `approvals_reviewer` | Option\<ApprovalsReviewer\> | None | Guardian 审查者 |
 | `allowed_approvals_reviewers` | Option\<Vec\> | None | 允许的审查者约束 |

 ---

 > **一句话回顾**：附录提供了完整类型目录（12 个类型含字段表和 serde 注解）、伪代码库索引（按功能分组）、3 个实战示例（正常/中断/兜底）、15 条 FAQ、扩展指南（新 ReviewTarget/新 finding 字段/自定义 prompt/修改模板）和快速查阅卡。

 ---

 ## 文档统计

 - **总章节数**：5 部分，27 个子章节
 - **架构图**：3 张 Mermaid + 3 张 ASCII
 - **时序图**：5 张 Mermaid sequence diagram
 - **伪代码块**：30+ 个
 - **数据样例**：20+ 个 JSON/文本样例
 - **代码引用**：100+ 个 file:line 引用
 - **测试覆盖**：9 个集成测试 + 多个单元测试
 - **FAQ**：15 条

 ---

 ---

 # 第六部分 · 逐行代码注解

 本部分对 `/review` 流程中最核心的文件进行逐行注解，帮助新手建立从代码到行为的精确映射。每个文件按功能块划分，每块给出：代码原文（简化）+ 行为说明 + 设计意图。

 ---

 ## 6.1 `core/src/tasks/review.rs` 逐行注解

 这是审查流程的核心文件，包含 `ReviewTask` 的全部执行逻辑。

 ### 6.1.1 导入与常量（L1-26）

 ```rust
 use std::borrow::Cow;
 use std::sync::Arc;

 use agere_protocol::config_types::WebSearchMode;
 use agere_protocol::items::TurnItem;
 use agere_protocol::models::ContentItem;
 use agere_protocol::models::ResponseItem;
 use agere_protocol::protocol::AgentMessageContentDeltaEvent;
 use agere_protocol::protocol::AgentMessageDeltaEvent;
 use agere_protocol::protocol::AskForApproval;
 use agere_protocol::protocol::Event;
 use agere_protocol::protocol::EventMsg;
 use agere_protocol::protocol::ExitedReviewModeEvent;
 use agere_protocol::protocol::ItemCompletedEvent;
 use agere_protocol::protocol::ReviewOutputEvent;
 use agere_protocol::protocol::SubAgentSource;
 use agere_protocol::utils_common::Template;
 use tokio_util::sync::CancellationToken;
 ```

 **注解：**

 - `Cow<'_, str>`：用于 `normalize_review_template_line_endings` 的返回类型（可能借用也可能拥有）
 - `WebSearchMode`：web search 模式枚举，审查中强制 `Disabled`
 - `TurnItem`：turn 项类型，用于事件匹配（`ItemCompleted(AgentMessage)`）
 - `ContentItem` / `ResponseItem`：消息内容/响应项，用于记录 conversation items
 - `AgentMessageContentDeltaEvent` / `AgentMessageDeltaEvent`：被抑制的流式事件
 - `AskForApproval`：审批策略枚举，审查中设为 `Never`
 - `Event` / `EventMsg`：事件和事件消息类型
 - `ExitedReviewModeEvent`：退出审查模式事件载荷
 - `ItemCompletedEvent`：item 完成事件
 - `ReviewOutputEvent`：结构化审查结果
 - `SubAgentSource`：子代理来源标记（`Review` 变体）
 - `Template`：模板引擎，用于渲染退出模板
 - `CancellationToken`：取消令牌，用于中断审查

 ```rust
 use crate::agere_delegate::run_agere_thread_one_shot;
 use crate::config::Constrained;
 use crate::review_format::format_review_findings_block;
 use crate::review_format::render_review_output_text;
 use crate::session::session::Session;
 use crate::session::turn_context::TurnContext;
 use crate::state::TaskKind;
 use agere_features::Feature;
 use agere_protocol::user_input::UserInput;
 use std::sync::LazyLock;
 ```

 **注解：**

 - `run_agere_thread_one_shot`：一次性子代理对话启动函数
 - `Constrained`：约束包装器，用于 `web_search_mode` 和 `approval_policy`
 - `format_review_findings_block` / `render_review_output_text`：格式化函数
 - `Session`：会话类型
 - `TurnContext`：turn 上下文
 - `TaskKind`：任务种类枚举
 - `Feature`：feature 标志枚举（`SpawnCsv`、`Collab` 等）
 - `UserInput`：用户输入类型
 - `LazyLock`：延迟初始化（用于模板）

 ```rust
 use super::SessionTask;
 use super::SessionTaskContext;

 static REVIEW_EXIT_SUCCESS_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
     let normalized =
         normalize_review_template_line_endings(crate::client_common::REVIEW_EXIT_SUCCESS_TMPL);
     Template::parse(normalized.as_ref())
         .unwrap_or_else(|err| panic!("review exit success template must parse: {err}"))
 });
 ```

 **注解：**

 - `REVIEW_EXIT_SUCCESS_TEMPLATE`：全局延迟初始化的退出成功模板
 - 初始化时先 `normalize_review_template_line_endings`（CRLF → LF）
 - 然后 `Template::parse` 解析模板
 - parse 失败会 `panic!`（模板是编译时常量，不应出错）
 - `LazyLock` 保证线程安全的延迟初始化（首次访问时初始化）

 ### 6.1.2 ReviewTask 结构体（L34-40）

 ```rust
 #[derive(Clone, Copy)]
 pub(crate) struct ReviewTask;

 impl ReviewTask {
     pub(crate) fn new() -> Self {
         Self
     }
 }
 ```

 **注解：**

 - `Clone, Copy`：零大小类型，可以低成本复制
 - `pub(crate)`：仅在 crate 内可见
 - `new()`：返回 `Self`（零大小，无字段）
 - 设计意图：`ReviewTask` 是一个类型标记，实际状态通过 `SessionTaskContext` 和 `TurnContext` 传入

 ### 6.1.3 SessionTask::run() 实现（L44-71）

 ```rust
 impl SessionTask for ReviewTask {
     fn kind(&self) -> TaskKind {
         TaskKind::Review
     }

     fn span_name(&self) -> &'static str {
         "session_task.review"
     }

     async fn run(
         self: Arc<Self>,
         session: Arc<SessionTaskContext>,
         ctx: Arc<TurnContext>,
         input: Vec<UserInput>,
         cancellation_token: CancellationToken,
     ) -> Option<String> {
 ```

 **注解：**

 - `kind()`：返回 `TaskKind::Review`，用于任务分类和遥测
 - `span_name()`：返回 `"session_task.review"`，用于 tracing span
 - `run()` 签名：
   - `self: Arc<Self>`：通过 Arc 持有自身（async trait 要求）
   - `session: Arc<SessionTaskContext>`：会话任务上下文（含 session、services 等）
   - `ctx: Arc<TurnContext>`：turn 上下文（含 config、model_info 等）
   - `input: Vec<UserInput>`：初始输入（审查 prompt）
   - `cancellation_token: CancellationToken`：取消令牌
   - 返回 `Option<String>`：审查总是返回 `None`

 ```rust
         session.session.services.session_telemetry.counter(
             "agere.task.review",
             /*inc*/ 1,
             &[],
         );
 ```

 **注解：**

 - 记录遥测 counter：`agere.task.review` +1
 - `&[]`：无标签（label）
 - 用途：统计审查任务执行次数

 ```rust
         // Start sub-agere conversation and get the receiver for events.
         let output = match start_review_conversation(
             session.clone(),
             ctx.clone(),
             input,
             cancellation_token.clone(),
         )
         .await
         {
             Some(receiver) => process_review_events(session.clone(), ctx.clone(), receiver).await,
             None => None,
         };
 ```

 **注解：**

 - `start_review_conversation` 返回 `Option<Receiver<Event>>`：
   - `Some(receiver)`：子代理启动成功，进入 `process_review_events`
   - `None`：子代理启动失败，`output = None`
 - `process_review_events` 返回 `Option<ReviewOutputEvent>`
 - `cancellation_token.clone()`：token 可以廉价 clone，clone 的 token 共享取消状态

 ```rust
         if !cancellation_token.is_cancelled() {
             exit_review_mode(session.clone_session(), output.clone(), ctx.clone()).await;
         }
         None
     }
 ```

 **注解：**

 - `is_cancelled()` 检查：只有在**未被取消**时才调 `exit_review_mode`
 - 如果被取消，`abort()` 会负责调 `exit_review_mode(None)`
 - `output.clone()`：`ReviewOutputEvent` 实现了 `Clone`
 - 返回 `None`：审查结果通过事件传递，不通过返回值

 ```rust
     async fn abort(&self, session: Arc<SessionTaskContext>, ctx: Arc<TurnContext>) {
         exit_review_mode(session.clone_session(), /*review_output*/ None, ctx).await;
     }
 }
 ```

 **注解：**

 - `abort()`：被取消时调用
 - `review_output = None`：中断时无审查结果
 - 调用 `exit_review_mode(None)` 渲染中断模板
 - 注意：`abort()` 和 `run()` 中的 `exit_review_mode` 可能都被调用（竞态），但 `exit_review_mode` 是幂等的

 ### 6.1.4 start_review_conversation() 实现（L79-115）

 ```rust
 async fn start_review_conversation(
     session: Arc<SessionTaskContext>,
     ctx: Arc<TurnContext>,
     input: Vec<UserInput>,
     cancellation_token: CancellationToken,
 ) -> Option<async_channel::Receiver<Event>> {
     let config = ctx.config.clone();
     let mut sub_agent_config = config.as_ref().clone();
 ```

 **注解：**

 - `config = ctx.config.clone()`：clone `Arc<Config>`（廉价，只增加引用计数）
 - `sub_agent_config = config.as_ref().clone()`：clone `Config` 本体（深拷贝）
 - 后续修改 `sub_agent_config` 不影响原 config

 ```rust
     // Carry over review-only feature restrictions so the delegate cannot
     // re-enable blocked tools (web search, collab tools, view image).
     if let Err(err) = sub_agent_config
         .web_search_mode
         .set(WebSearchMode::Disabled)
     {
         panic!("by construction Constrained<WebSearchMode> must always support Disabled: {err}");
     }
     let _ = sub_agent_config.features.disable(Feature::SpawnCsv);
     let _ = sub_agent_config.features.disable(Feature::Collab);
 ```

 **注解：**

 - `web_search_mode.set(Disabled)`：强制禁用 web search
   - 失败会 `panic!`：设计假设 `Constrained<WebSearchMode>` 总是允许 `Disabled`
   - 注释说"Carry over review-only feature restrictions so the delegate cannot re-enable blocked tools"
 - `features.disable(SpawnCsv)`：禁用 CSV 批量子代理
 - `features.disable(Collab)`：禁用协作工具
 - `let _ =`：忽略 disable 的返回值（可能已经禁用）

 ```rust
     // Set explicit review rubric for the sub-agent
     sub_agent_config.base_instructions = Some(crate::REVIEW_PROMPT.to_string());
     sub_agent_config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
 ```

 **注解：**

 - `base_instructions = REVIEW_PROMPT`：设置审查专用系统 prompt
   - `REVIEW_PROMPT` 来自 `core/review_prompt.md`（通过 `include_str!`）
   - 替代了主会话的系统指令
 - `approval_policy = allow_only(Never)`：
   - `Constrained::allow_only(Never)` 把约束设为"只允许 Never"
   - 子代理执行操作时不需要审批

 ```rust
     let model = config
         .review_model
         .clone()
         .unwrap_or_else(|| ctx.model_info.slug.clone());
     sub_agent_config.model = Some(model);
 ```

 **注解：**

 - 模型确定：`review_model` > 主会话模型
   - `config.review_model`：用户配置的审查模型
   - `ctx.model_info.slug`：主会话当前模型
 - `sub_agent_config.model = Some(model)`：设置子代理模型

 ```rust
     (run_agere_thread_one_shot(
         sub_agent_config,
         session.auth_manager(),
         session.models_manager(),
         input,
         session.clone_session(),
         ctx.clone(),
         cancellation_token,
         SubAgentSource::Review,
         /*final_output_json_schema*/ None,
         /*initial_history*/ None,
     )
     .await)
         .ok()
         .map(|io| io.rx_event)
 }
 ```

 **注解：**

 - `run_agere_thread_one_shot` 参数：
   - `sub_agent_config`：审查专用配置
   - `auth_manager()`：认证管理器
   - `models_manager()`：模型管理器
   - `input`：初始输入（审查 prompt）
   - `clone_session()`：克隆会话引用
   - `ctx.clone()`：turn 上下文
   - `cancellation_token`：取消令牌
   - `SubAgentSource::Review`：子代理来源标记
   - `None`：无 final_output_json_schema（不用 schema 约束输出，靠 parse 兜底）
   - `None`：无 initial_history（不携带主会话历史）
 - `.ok()`：`Result` → `Option`（Err 变 None）
 - `.map(|io| io.rx_event)`：提取事件 receiver

 ### 6.1.5 process_review_events() 实现（L117-150）

 ```rust
 async fn process_review_events(
     session: Arc<SessionTaskContext>,
     ctx: Arc<TurnContext>,
     receiver: async_channel::Receiver<Event>,
 ) -> Option<ReviewOutputEvent> {
     let mut prev_agent_message: Option<Event> = None;
 ```

 **注解：**

 - `prev_agent_message`：暂存上一条 `AgentMessage` 事件
 - 初始为 `None`

 ```rust
     while let Ok(event) = receiver.recv().await {
         match event.clone().msg {
 ```

 **注解：**

 - `while let Ok(event)`：循环接收事件，`Err`（channel 关闭）时退出
 - `event.clone()`：clone 事件（因为后面要 move `event.msg`，但可能还需要 `event`）

 ```rust
             EventMsg::AgentMessage(_) => {
                 if let Some(prev) = prev_agent_message.take() {
                     session
                         .clone_session()
                         .send_event(ctx.as_ref(), prev.msg)
                         .await;
                 }
                 prev_agent_message = Some(event);
             }
 ```

 **注解：**

 - `AgentMessage` 事件处理：
   1. 如果有暂存的上一条，先 `send_event` 转发它
   2. 把当前事件暂存到 `prev_agent_message`
 - 设计意图：暂存而非立即转发，等待下一条或 `TurnComplete`
   - 只有最后一条 `AgentMessage`（TurnComplete 时的 `last_agent_message`）才包含完整 JSON
   - 中间消息可能是不完整的思考过程

 ```rust
             // Suppress ItemCompleted only for assistant messages: forwarding it
             // would trigger legacy AgentMessage via as_legacy_events(), which this
             // review flow intentionally hides in favor of structured output.
             EventMsg::ItemCompleted(ItemCompletedEvent {
                 item: TurnItem::AgentMessage(_),
                 ..
             })
             | EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { .. })
             | EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent { .. }) => {}
 ```

 **注解：**

 - 三种事件被抑制（`=> {}` 空匹配）：
   1. `ItemCompleted(AgentMessage)`：转发会触发 legacy AgentMessage
   2. `AgentMessageDelta`：流式增量
   3. `AgentMessageContentDelta`：内容增量
 - 使用 `|` 模式组合（或模式）
 - `..`：忽略其他字段
 - 注释解释了 `ItemCompleted` 抑制的原因

 ```rust
             EventMsg::TurnComplete(task_complete) => {
                 // Parse review output from the last agent message (if present).
                 let out = task_complete
                     .last_agent_message
                     .as_deref()
                     .map(parse_review_output_event);
                 return out;
             }
 ```

 **注解：**

 - `TurnComplete` 事件处理：
   - `task_complete.last_agent_message`：`Option<String>`
   - `.as_deref()`：`Option<String>` → `Option<&str>`
   - `.map(parse_review_output_event)`：有值则解析，无值则 `None`
   - `return out`：直接返回，退出循环

 ```rust
             EventMsg::TurnAborted(_) => {
                 // Cancellation or abort: consumer will finalize with None.
                 return None;
             }
 ```

 **注解：**

 - `TurnAborted` 事件处理：
   - 返回 `None`（无审查结果）
   - 注释说"consumer will finalize with None"（run() 或 abort() 会调 exit_review_mode(None)）

 ```rust
             other => {
                 session
                     .clone_session()
                     .send_event(ctx.as_ref(), other)
                     .await;
             }
         }
     }
     // Channel closed without TurnComplete: treat as interrupted.
     None
 }
 ```

 **注解：**

 - `other`：所有其他事件正常转发
   - 包括工具调用事件、错误事件等
 - 循环退出后返回 `None`：
   - `receiver.recv()` 返回 `Err`（channel 关闭）
   - 注释说"Channel closed without TurnComplete: treat as interrupted"

 ### 6.1.6 parse_review_output_event() 实现（L155-172）

 ```rust
 fn parse_review_output_event(text: &str) -> ReviewOutputEvent {
     if let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(text) {
         return ev;
     }
 ```

 **注解：**

 - 策略 1：整体 JSON 解析
   - `serde_json::from_str::<ReviewOutputEvent>(text)` 尝试把整个文本解析为 `ReviewOutputEvent`
   - 成功则直接返回

 ```rust
     if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}'))
         && start < end
         && let Some(slice) = text.get(start..=end)
         && let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(slice)
     {
         return ev;
     }
 ```

 **注解：**

 - 策略 2：提取子串解析
   - `text.find('{')`：找第一个 `{`
   - `text.rfind('}')`：找最后一个 `}`
   - `start < end`：确保 `{` 在 `}` 之前
   - `text.get(start..=end)`：提取子串（inclusive）
   - `serde_json::from_str`：解析子串
   - 使用 `&&` 链式 let（Rust 2024 特性，collapsible if）
   - 成功则返回

 ```rust
     ReviewOutputEvent {
         overall_explanation: text.to_string(),
         ..Default::default()
     }
 }
 ```

 **注解：**

 - 策略 3：纯文本兜底
   - 把整个文本放进 `overall_explanation`
   - `..Default::default()`：其余字段用默认值
     - `findings: vec![]`
     - `overall_correctness: ""`
     - `overall_confidence_score: 0.0`

 ### 6.1.7 exit_review_mode() 实现（L177-241）

 ```rust
 pub(crate) async fn exit_review_mode(
     session: Arc<Session>,
     review_output: Option<ReviewOutputEvent>,
     ctx: Arc<TurnContext>,
 ) {
     const REVIEW_USER_MESSAGE_ID: &str = "review_rollout_user";
     const REVIEW_ASSISTANT_MESSAGE_ID: &str = "review_rollout_assistant";
 ```

 **注解：**

 - `pub(crate)`：crate 内可见（abort 也需要调用）
 - 两个常量 ID：
   - `"review_rollout_user"`：user message 的 ID
   - `"review_rollout_assistant"`：assistant message 的 ID
   - 固定 ID 用于后续 turn 中识别审查记录

 ```rust
     let (user_message, assistant_message) = if let Some(out) = review_output.clone() {
         let mut findings_str = String::new();
         let text = out.overall_explanation.trim();
         if !text.is_empty() {
             findings_str.push_str(text);
         }
 ```

 **注解：**

 - `if let Some(out) = review_output.clone()`：有审查结果时
   - `.clone()`：因为后面还要用 `review_output`（emit 事件时）
 - `findings_str`：拼接 explanation + findings block
 - `text = out.overall_explanation.trim()`：去掉首尾空白
 - 非空则 push 到 `findings_str`

 ```rust
         if !out.findings.is_empty() {
             let block = format_review_findings_block(&out.findings, /*selection*/ None);
             findings_str.push_str(&format!("\n{block}"));
         }
 ```

 **注解：**

 - 有 findings 时：
   - `format_review_findings_block(findings, None)`：格式化为文本块
   - `selection = None`：不显示 checkbox（简单 bullet）
   - `push_str("\n{block}")`：追加到 findings_str（前面加换行）

 ```rust
         let rendered = render_review_exit_success(&findings_str);
         let assistant_message = render_review_output_text(&out);
         (rendered, assistant_message)
 ```

 **注解：**

 - `rendered = render_review_exit_success(findings_str)`：用成功模板包裹
   - 生成 `<user_action>...<results>findings_str</results>...</user_action>`
 - `assistant_message = render_review_output_text(out)`：纯文本渲染
   - 生成 explanation + findings block（不带 XML）
 - 返回元组 `(user_message, assistant_message)`

 ```rust
     } else {
         let rendered = normalize_review_template_line_endings(
             crate::client_common::REVIEW_EXIT_INTERRUPTED_TMPL,
         )
         .into_owned();
         let assistant_message =
             "Review was interrupted. Please re-run /review and wait for it to complete."
                 .to_string();
         (rendered, assistant_message)
     };
 ```

 **注解：**

 - `else`：无审查结果（中断/取消/启动失败）
 - `rendered`：中断模板（CRLF 规范化后）
   - `<user_action>...interrupted...</user_action>`
 - `assistant_message`：固定文本
   - `"Review was interrupted. Please re-run /review..."`
 - `.into_owned()`：`Cow` → `String`

 ```rust
     session
         .record_conversation_items(
             &ctx,
             &[ResponseItem::Message {
                 id: Some(REVIEW_USER_MESSAGE_ID.to_string()),
                 role: "user".to_string(),
                 content: vec![ContentItem::InputText { text: user_message }],
                 phase: None,
             }],
         )
         .await;
 ```

 **注解：**

 - 记录 user message 到会话历史
   - `id: "review_rollout_user"`
   - `role: "user"`
   - `content: [InputText { text: user_message }]`
   - `phase: None`
 - `record_conversation_items`：批量记录（这里只记一条）

 ```rust
     session
         .send_event(
             ctx.as_ref(),
             EventMsg::ExitedReviewMode(ExitedReviewModeEvent { review_output }),
         )
         .await;
 ```

 **注解：**

 - emit `ExitedReviewMode` 事件
   - `ExitedReviewModeEvent { review_output }`：携带审查结果（可能 Some 或 None）
   - 这是 TUI 收到的退出信号
   - 注意：`review_output` 在前面 `.clone()` 了，这里 move 原值

 ```rust
     session
         .record_response_item_and_emit_turn_item(
             ctx.as_ref(),
             ResponseItem::Message {
                 id: Some(REVIEW_ASSISTANT_MESSAGE_ID.to_string()),
                 role: "assistant".to_string(),
                 content: vec![ContentItem::OutputText {
                     text: assistant_message,
                 }],
                 phase: None,
             },
         )
         .await;
 ```

 **注解：**

 - 记录 assistant message 到会话历史
   - `id: "review_rollout_assistant"`
   - `role: "assistant"`
   - `content: [OutputText { text: assistant_message }]`
   - `phase: None`
 - `record_response_item_and_emit_turn_item`：记录并 emit turn item
   - 与 `record_conversation_items` 不同，这个还会 emit turn item 事件

 ```rust
     // Review turns can run before any regular user turn, so explicitly
     // materialize rollout persistence. Do this after emitting review output so
     // file creation + git metadata collection cannot delay client-facing items.
     session.ensure_rollout_materialized().await;
 }
 ```

 **注解：**

 - `ensure_rollout_materialized()`：确保持久化到磁盘
 - 注释解释：
   1. 审查 turn 可能在常规 turn 之前运行（rollout 文件可能未创建）
   2. 延迟到 emit 之后，避免磁盘 I/O 延迟用户看到结果
 - 这是 `exit_review_mode` 的最后一步

 ### 6.1.8 辅助函数（L243-261）

 ```rust
 fn render_review_exit_success(results: &str) -> String {
     REVIEW_EXIT_SUCCESS_TEMPLATE
         .render([("results", results)])
         .unwrap_or_else(|err| panic!("review exit success template must render: {err}"))
 }
 ```

 **注解：**

 - 渲染退出成功模板
   - `REVIEW_EXIT_SUCCESS_TEMPLATE`：全局 LazyLock 模板
   - `.render([("results", results)])`：渲染变量 `{{results}}`
   - 失败会 `panic!`（模板变量由代码控制，不应出错）

 ```rust
 fn normalize_review_template_line_endings(template: &str) -> Cow<'_, str> {
     if template.contains('\r') {
         Cow::Owned(template.replace("\r\n", "\n").replace('\r', "\n"))
     } else {
         Cow::Borrowed(template)
     }
 }
 ```

 **注解：**

 - CRLF 规范化
   - `contains('\r')`：检查是否含 CR
   - 有 CR：`Cow::Owned`（拥有 String）
     - 先替换 `\r\n` → `\n`（Windows 换行）
     - 再替换 `\r` → `\n`（旧 Mac 换行）
   - 无 CR：`Cow::Borrowed`（借用原字符串，零成本）
 - 返回 `Cow<'_, str>`：可能借用也可能拥有
 - 用途：避免不同操作系统的换行符导致测试失败

 ---

 > **一句话回顾**：逐行注解揭示了 `ReviewTask` 的设计精髓——`run()` 的三步流程（起子代理→消费事件→退出回灌）、`process_review_events` 的事件抑制/暂存策略、`parse_review_output_event` 的三级降级、`exit_review_mode` 的双消息记录+延迟持久化，以及辅助函数的 CRLF 规范化与 panic 策略。

 ---

 ## 6.2 `core/src/review_prompts.rs` 逐行注解

 ### 6.2.1 导入与常量（L1-30）

 ```rust
 use agere_git_utils::merge_base_with_head;
 use agere_protocol::protocol::ReviewRequest;
 use agere_protocol::protocol::ReviewTarget;
 use agere_utils_common::Template;
 use agere_utils_fs::AbsolutePathBuf;
 use std::sync::LazyLock;
 ```

 **注解：**

 - `merge_base_with_head`：git merge-base 计算函数
 - `ReviewRequest` / `ReviewTarget`：协议类型
 - `Template`：模板引擎
 - `AbsolutePathBuf`：绝对路径类型（用于 cwd）
 - `LazyLock`：延迟初始化

 ```rust
 #[derive(Clone, Debug, PartialEq)]
 pub struct ResolvedReviewRequest {
     pub target: ReviewTarget,
     pub prompt: String,
     pub user_facing_hint: String,
 }
 ```

 **注解：**

 - 无 serde derive（内部类型，不序列化）
 - `Clone, Debug, PartialEq`：用于测试比较

 ```rust
 const UNCOMMITTED_PROMPT: &str = "Review the current code changes (staged, unstaged, and untracked files) and provide prioritized findings.";
 ```

 **注解：**

 - 固定文本，无需模板渲染
 - 指导子代理审查 staged + unstaged + untracked 文件

 ```rust
 const BASE_BRANCH_PROMPT_BACKUP: &str = "Review the code changes against the base branch '{{branch}}'. Start by finding the merge diff between the current branch and {{branch}}'s upstream e.g. (`git merge-base HEAD \"$(git rev-parse --abbrev-ref \"{{branch}}@{upstream}\")\"`), then run `git diff` against that SHA to see what changes we would merge into the {{branch}} branch. Provide prioritized, actionable findings.";
 const BASE_BRANCH_PROMPT: &str = "Review the code changes against the base branch '{{base_branch}}'. The merge base commit for this comparison is {{merge_base_sha}}. Run `git diff {{merge_base_sha}}` to inspect the changes relative to {{base_branch}}. Provide prioritized, actionable findings.";
 ```

 **注解：**

 - 两个模板：
   - `BASE_BRANCH_PROMPT`：有 merge-base 时用，直接给出 SHA
   - `BASE_BRANCH_PROMPT_BACKUP`：无 merge-base 时用，让子代理自己找
 - 模板变量用 `{{var}}`（双花括号）
 - backup 模板包含 shell 命令示例

 ```rust
 static BASE_BRANCH_PROMPT_BACKUP_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
     Template::parse(BASE_BRANCH_PROMPT_BACKUP)
         .unwrap_or_else(|err| panic!("base branch backup review prompt must parse: {err}"))
 });
 static BASE_BRANCH_PROMPT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
     Template::parse(BASE_BRANCH_PROMPT)
         .unwrap_or_else(|err| panic!("base branch review prompt must parse: {err}"))
 });
 ```

 **注解：**

 - 两个 LazyLock 模板
 - parse 失败会 `panic!`（编译时常量不应出错）

 ```rust
 const COMMIT_PROMPT_WITH_TITLE: &str = "Review the code changes introduced by commit {{sha}} (\"{{title}}\"). Provide prioritized, actionable findings.";
 const COMMIT_PROMPT: &str = "Review the code changes introduced by commit {{sha}}. Provide prioritized, actionable findings.";
 static COMMIT_PROMPT_WITH_TITLE_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
     Template::parse(COMMIT_PROMPT_WITH_TITLE)
         .unwrap_or_else(|err| panic!("commit review prompt with title must parse: {err}"))
 });
 static COMMIT_PROMPT_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
     Template::parse(COMMIT_PROMPT)
         .unwrap_or_else(|err| panic!("commit review prompt must parse: {err}"))
 });
 ```

 **注解：**

 - 两个 commit 模板（有/无 title）
 - 同样 LazyLock + panic on parse error

 ### 6.2.2 resolve_review_request() 实现（L47-72）

 ```rust
 pub fn resolve_review_request(
     request: ReviewRequest,
     cwd: &AbsolutePathBuf,
 ) -> anyhow::Result<ResolvedReviewRequest> {
     let target = request.target;
     let prompt = review_prompt(&target, cwd)?;
     let user_facing_hint = request
         .user_facing_hint
         .unwrap_or_else(|| user_facing_hint(&target));

     Ok(ResolvedReviewRequest {
         target,
         prompt,
         user_facing_hint,
     })
 }
 ```

 **注解：**

 - `request.target`：move 出 target
 - `review_prompt(&target, cwd)?`：生成 prompt（可能失败，如 Custom 为空）
 - `user_facing_hint`：
   - 优先用 request 中的（如果有）
   - 否则用 `user_facing_hint(&target)` 从 target 推导
 - 返回 `ResolvedReviewRequest`

 ### 6.2.3 review_prompt() 实现（L74-103）

 ```rust
 pub fn review_prompt(target: &ReviewTarget, cwd: &AbsolutePathBuf) -> anyhow::Result<String> {
     match target {
         ReviewTarget::UncommittedChanges => Ok(UNCOMMITTED_PROMPT.to_string()),
 ```

 **注解：**

 - `UncommittedChanges`：返回固定文本

 ```rust
         ReviewTarget::BaseBranch { branch } => {
             if let Some(commit) = merge_base_with_head(cwd, branch)? {
                 Ok(render_review_prompt(
                     &BASE_BRANCH_PROMPT_TEMPLATE,
                     [
                         ("base_branch", branch.as_str()),
                         ("merge_base_sha", commit.as_str()),
                     ],
                 ))
             } else {
                 Ok(render_review_prompt(
                     &BASE_BRANCH_PROMPT_BACKUP_TEMPLATE,
                     [("branch", branch.as_str())],
                 ))
             }
         }
 ```

 **注解：**

 - `BaseBranch { branch }`：
   - `merge_base_with_head(cwd, branch)?`：计算 merge-base（可能失败）
   - 有结果（`Some(commit)`）：用精确模板
     - 变量：`base_branch` + `merge_base_sha`
   - 无结果（`None`）：用 backup 模板
     - 变量：`branch`

 ```rust
         ReviewTarget::Commit { sha, title } => {
             if let Some(title) = title {
                 Ok(render_review_prompt(
                     &COMMIT_PROMPT_WITH_TITLE_TEMPLATE,
                     [("sha", sha.as_str()), ("title", title.as_str())],
                 ))
             } else {
                 Ok(render_review_prompt(
                     &COMMIT_PROMPT_TEMPLATE,
                     [("sha", sha.as_str())],
                 ))
             }
         }
 ```

 **注解：**

 - `Commit { sha, title }`：
   - 有 title：用 `COMMIT_PROMPT_WITH_TITLE_TEMPLATE`
     - 变量：`sha` + `title`
   - 无 title：用 `COMMIT_PROMPT_TEMPLATE`
     - 变量：`sha`

 ```rust
         ReviewTarget::Custom { instructions } => {
             let prompt = instructions.trim();
             if prompt.is_empty() {
                 anyhow::bail!("Review prompt cannot be empty");
             }
             Ok(prompt.to_string())
         }
     }
 }
 ```

 **注解：**

 - `Custom { instructions }`：
   - `trim()`：去首尾空白
   - 空则 `bail!`（返回 Err）
   - 非空则直接用作 prompt

 ### 6.2.4 render_review_prompt() 和 user_facing_hint()

 ```rust
 fn render_review_prompt<'a, const N: usize>(
     template: &Template,
     variables: [(&'a str, &'a str); N],
 ) -> String {
     template
         .render(variables)
         .unwrap_or_else(|err| panic!("review prompt template must render: {err}"))
 }
 ```

 **注解：**

 - 泛型函数，`N` 是变量数量（编译时已知）
 - `render(variables)`：渲染模板
 - 失败会 `panic!`

 ```rust
 pub fn user_facing_hint(target: &ReviewTarget) -> String {
     match target {
         ReviewTarget::UncommittedChanges => "current changes".to_string(),
         ReviewTarget::BaseBranch { branch } => format!("changes against '{branch}'"),
         ReviewTarget::Commit { sha, title } => {
             let short_sha: String = sha.chars().take(7).collect();
             if let Some(title) = title {
                 format!("commit {short_sha}: {title}")
             } else {
                 format!("commit {short_sha}")
             }
         }
         ReviewTarget::Custom { instructions } => instructions.trim().to_string(),
     }
 }
 ```

 **注解：**

 - 按 target 生成 UI 提示文本
 - `short_sha`：取前 7 字符（git short SHA 常见长度）
 - `Custom`：用 trim 后的指令作为提示

 ### 6.2.5 From<ResolvedReviewRequest> for ReviewRequest

 ```rust
 impl From<ResolvedReviewRequest> for ReviewRequest {
     fn from(resolved: ResolvedReviewRequest) -> Self {
         ReviewRequest {
             target: resolved.target,
             user_facing_hint: Some(resolved.user_facing_hint),
         }
     }
 }
 ```

 **注解：**

 - 反向转换：`ResolvedReviewRequest` → `ReviewRequest`
 - `user_facing_hint` 总是 `Some`（因为 `ResolvedReviewRequest` 一定有 hint）
 - 用于 `spawn_review_thread` 中构造 `ReviewRequest` 给 `EnteredReviewMode` 事件

 ---

 > **一句话回顾**：`review_prompts.rs` 的逐行注解展示了 prompt 生成的完整逻辑——4 种 target 各有对应的模板（UncommittedChanges 固定文本、BaseBranch 有/无 merge-base 双模板、Commit 有/无 title 双模板、Custom 直接用），`user_facing_hint` 按 target 生成 UI 文本，`From` 转换支持事件构造。

 ---

 ## 6.3 `core/src/review_format.rs` 逐行注解

 ### 6.3.1 导入与常量

 ```rust
 use agere_protocol::protocol::ReviewFinding;
 use agere_protocol::protocol::ReviewOutputEvent;

 // Note: We keep this module UI-agnostic. It returns plain strings that
 // higher layers (e.g., TUI) may style as needed.
 ```

 **注解：**

 - 注释强调 UI 无关设计：返回纯字符串，由上层（TUI）处理样式

 ```rust
 fn format_location(item: &ReviewFinding) -> String {
     let path = item.code_location.absolute_file_path.display();
     let start = item.code_location.line_range.start;
     let end = item.code_location.line_range.end;
     format!("{path}:{start}-{end}")
 }
 ```

 **注解：**

 - 格式化位置：`"path:start-end"`
   - `display()`：`PathBuf` → `Display`
   - `start` 和 `end` 是 `u32`
   - 例如：`"/tmp/file.rs:10-20"`

 ```rust
 const REVIEW_FALLBACK_MESSAGE: &str = "Reviewer failed to output a response.";
 ```

 **注解：**

 - 兜底消息：当 explanation 和 findings 都为空时使用

 ### 6.3.2 format_review_findings_block() 实现

 ```rust
 pub fn format_review_findings_block(
     findings: &[ReviewFinding],
     selection: Option<&[bool]>,
 ) -> String {
     let mut lines: Vec<String> = Vec::new();
     lines.push(String::new());  // 空行开头
 ```

 **注解：**

 - `findings: &[ReviewFinding]`：findings 切片
 - `selection: Option<&[bool]>`：可选的选中状态数组
   - `Some`：显示 checkbox（`[x]` / `[ ]`）
   - `None`：显示简单 bullet（`-`）
 - `lines`：行向量
 - 第一行是空行（视觉间隔）

 ```rust
     // Header
     if findings.len() > 1 {
         lines.push("Full review comments:".to_string());
     } else {
         lines.push("Review comment:".to_string());
     }
 ```

 **注解：**

 - 单数 vs 复数 header
   - `len() > 1`：`"Full review comments:"`
   - `len() <= 1`：`"Review comment:"`
   - 注意：`len() == 0` 也会显示 `"Review comment:"`（但通常不调用）

 ```rust
     for (idx, item) in findings.iter().enumerate() {
         lines.push(String::new());  // 每条前空行
 ```

 **注解：**

 - 遍历 findings，每条前加空行

 ```rust
         let title = &item.title;
         let location = format_location(item);
 ```

 **注解：**

 - `title`：finding 标题
 - `location`：格式化位置（`"path:start-end"`）

 ```rust
         if let Some(flags) = selection {
             // Default to selected if index is out of bounds.
             let checked = flags.get(idx).copied().unwrap_or(true);
             let marker = if checked { "[x]" } else { "[ ]" };
             lines.push(format!("- {marker} {title} — {location}"));
         } else {
             lines.push(format!("- {title} — {location}"));
         }
 ```

 **注解：**

 - checkbox 模式（`selection = Some`）：
   - `flags.get(idx).copied().unwrap_or(true)`：越界默认选中
   - `marker`：`"[x]"`（选中）或 `"[ ]"`（未选中）
   - 格式：`"- [x] Title — path:start-end"`
 - 简单 bullet 模式（`selection = None`）：
   - 格式：`"- Title — path:start-end"`
   - 注意：用 `—`（em dash）而非 `-`

 ```rust
         for body_line in item.body.lines() {
             lines.push(format!("  {body_line}"));
         }
     }
 ```

 **注解：**

 - body 每行缩进 2 空格
   - `item.body.lines()`：按行分割
   - `format!("  {body_line}")`：2 空格缩进

 ```rust
     lines.join("\n")
 }
 ```

 **注解：**

 - 用 `\n` 连接所有行

 ### 6.3.3 render_review_output_text() 实现

 ```rust
 pub fn render_review_output_text(output: &ReviewOutputEvent) -> String {
     let mut sections = Vec::new();
     let explanation = output.overall_explanation.trim();
     if !explanation.is_empty() {
         sections.push(explanation.to_string());
     }
 ```

 **注解：**

 - `sections`：各部分文本（explanation + findings block）
 - `explanation`：trim 后的总体解释
   - 非空则加入 sections

 ```rust
     if !output.findings.is_empty() {
         let findings = format_review_findings_block(&output.findings, /*selection*/ None);
         let trimmed = findings.trim();
         if !trimmed.is_empty() {
             sections.push(trimmed.to_string());
         }
     }
 ```

 **注解：**

 - 有 findings 时：
   - `format_review_findings_block(findings, None)`：格式化
   - `trim()`：去首尾空白
   - 非空则加入 sections

 ```rust
     if sections.is_empty() {
         REVIEW_FALLBACK_MESSAGE.to_string()
     } else {
         sections.join("\n\n")
     }
 }
 ```

 **注解：**

 - 都为空：返回 `"Reviewer failed to output a response."`
 - 否则：用 `\n\n`（双换行）连接各部分
   - 双换行在视觉上有明显分隔

 ---

 > **一句话回顾**：`review_format.rs` 的逐行注解揭示了 UI 无关的格式化逻辑——`format_location` 生成 `path:start-end`，`format_review_findings_block` 支持 checkbox/bullet 两种模式（越界默认选中），`render_review_output_text` 用双换行拼接 explanation + findings 或返回兜底消息。

 ---
 ---

 # 第七部分 · 状态机与数据流图

 本部分用状态机和数据流图补充前面的时序图，帮助理解审查流程的状态转换和数据变换。

 ---

 ## 7.1 审查流程状态机

 ### 7.1.1 主状态机（ASCII）

 ```
                         ┌─────────────────┐
                         │     IDLE        │
                         │ (非审查模式)     │
                         └────────┬────────┘
                                  │
                          Op::Review 提交
                                  │
                                  ▼
                         ┌─────────────────┐
                         │  RESOLVING      │
                         │ resolve_review_  │
                         │ request()       │
                         └────────┬────────┘
                                  │
                     ┌────────────┼────────────┐
                     │            │            │
                  Ok(resolved)  Err(err)    (其他)
                     │            │            │
                     ▼            ▼            ▼
           ┌─────────────┐  ┌─────────┐  ┌─────────┐
           │  SPAWNING   │  │  ERROR  │  │  IDLE   │
           │ spawn_review│  │ emit    │  │ (不变)   │
           │ _thread()   │  │ ErrorEv │  └─────────┘
           └──────┬──────┘  └─────────┘
                  │
           emit EnteredReviewMode
                  │
                  ▼
           ┌─────────────────┐
           │  REVIEWING      │  ◄─────── User Ctrl+C
           │ ReviewTask::run │           (cancellation_token)
           │ ├ start_review_ │ │
           │ │  conversation │ │
           │ ├ process_review│ │
           │ │  _events      │ │
           │ └ (循环接收事件) │ │
           └────────┬────────┘ │
                    │          │
          ┌─────────┼──────────┘
          │         │
   TurnComplete  TurnAborted
   /channel close │
          │         │
          ▼         ▼
     ┌────────┐  ┌────────────┐
     │PARSING │  │  ABORTING  │
     │parse_  │  │ abort()    │
     │review_ │  │ exit(None) │
     │output_ │  └─────┬──────┘
     │event() │        │
     └───┬────┘        │
         │             │
         ▼             │
     ┌─────────────────┘
     │
     ▼
  ┌──────────────┐
  │  EXITING     │
  │ exit_review_ │
  │ mode()       │
  │ ├ record user│
  │ ├ emit Exited│
  │ ├ record asst│
  │ └ materialize│
  └──────┬───────┘
         │
         ▼
  ┌──────────────┐
  │  IDLE        │
  │ (非审查模式)  │
  └──────────────┘
 ```

 ### 7.1.2 状态说明

 | 状态 | 描述 | 持续时间 | 可中断 |
 |------|------|---------|--------|
 | IDLE | 非审查模式，正常对话 | - | - |
 | RESOLVING | 解析审查请求，生成 prompt | 短（同步） | 否 |
 | ERROR | resolve 失败，emit Error | 瞬时 | 否 |
 | SPAWNING | 构建隔离上下文，spawn 任务 | 短（同步） | 否 |
 | REVIEWING | 子代理执行审查 | 长（异步） | 是（Ctrl+C） |
 | PARSING | 解析子代理输出 | 短（同步） | 否 |
 | ABORTING | 中断处理，exit(None) | 短（同步） | 否 |
 | EXITING | 退出审查模式，回灌结果 | 短（同步） | 否 |

 ### 7.1.3 TUI 状态机（ASCII）

 ```
   ┌──────────────┐
   │  NORMAL      │
   │ is_review_   │
   │ mode = false │
   └──────┬───────┘
          │
   EnteredReviewMode
          │
          ▼
   ┌──────────────┐
   │  REVIEW_MODE │
   │ is_review_   │  ◄─── 用户消息被抑制
   │ mode = true  │  ◄─── token info 已保存
   │ banner 显示  │
   └──────┬───────┘
          │
   ExitedReviewMode
          │
          ▼
   ┌──────────────┐
   │  RENDERING   │
   │ 渲染 findings│
   │ / explanation│
   │ / error      │
   └──────┬───────┘
          │
          ▼
   ┌──────────────┐
   │  NORMAL      │
   │ is_review_   │
   │ mode = false │
   │ token info   │
   │ 已恢复       │
   └──────────────┘
 ```

 ### 7.1.4 Mermaid 状态机

 ```
                  +-----------------+
                  |     IDLE        |
                  +--------+--------+
                           | Op::Review
                           v
                  +-----------------+
                  |  RESOLVING      |
                  +--------+--------+
                 +---------+---------+
                 |         |         |
              Ok        Err      (other)
                 |         |         |
                 v         v         v
           +---------+ +-------+ +-------+
           |SPAWNING | | ERROR | | IDLE  |
           +----+----+ +-------+ +-------+
                | emit EnteredReviewMode
                v
           +-----------------+
           |  REVIEWING      |<--- Ctrl+C
           +--------+--------+
                    |
           +--------+--------+
           |        |        |
     TurnComplete  Abort  ch.close
           |        |        |
           v        v        v
      +--------+ +--------+ +--------+
      |PARSING | |ABORTING| | (None) |
      +---+----+ +---+----+ +---+----+
          +----------+----------+
                   |
                   v
            +--------------+
            |  EXITING     |
            +------+-------+
                   |
                   v
            +--------------+
            |  IDLE        |
            +--------------+
```

 ---

 ## 7.2 数据流变换图

 ### 7.2.1 审查请求的数据变换链

 ```
 用户输入 "/review main"
       │
       ▼
 ┌─────────────────┐
 │ 文本字符串       │ "/review main"
 └────────┬────────┘
          │ SlashCommand::from_str
          ▼
 ┌─────────────────┐
 │ SlashCommand    │ Review, args="main"
 └────────┬────────┘
          │ 构造 ReviewRequest
          ▼
 ┌─────────────────┐
 │ ReviewRequest   │ { target: BaseBranch { branch: "main" }, hint: None }
 └────────┬────────┘
          │ Op::Review
          ▼
 ┌─────────────────┐
 │ Op::Review      │ { review_request }
 └────────┬────────┘
          │ resolve_review_request
          ▼
 ┌─────────────────┐
 │ ResolvedReview  │ { target, prompt: "Review...merge_base...", hint: "changes against 'main'" }
 │ Request         │
 └────────┬────────┘
          │ spawn_review_thread
          ▼
 ┌─────────────────┐
 │ Vec<UserInput>  │ [Text { text: prompt, text_elements: [] }]
 └────────┬────────┘
          │ run_agere_thread_one_shot
          ▼
 ┌─────────────────┐
 │ API 请求 JSON   │ { model, instructions: REVIEW_PROMPT, input: [...], tools: [...] }
 └────────┬────────┘
          │ 子代理执行
          ▼
 ┌─────────────────┐
 │ Event 流        │ AgentMessage, Delta, TurnComplete, ...
 └────────┬────────┘
          │ process_review_events
          ▼
 ┌─────────────────┐
 │ Option<String>  │ last_agent_message = Some("...JSON...")
 └────────┬────────┘
          │ parse_review_output_event
          ▼
 ┌─────────────────┐
 │ ReviewOutputEvent│ { findings: [...], overall_correctness, ... }
 └────────┬────────┘
          │ exit_review_mode
          ▼
 ┌─────────────────┐
 │ (user_msg,      │ user_msg = XML 模板
 │  asst_msg)      │ asst_msg = 纯文本
 └────────┬────────┘
          │ record + emit
          ▼
 ┌─────────────────┐
 │ Conversation    │ ResponseItem::Message × 2
 │ Items           │ + ExitedReviewMode 事件
 └────────┬────────┘
          │ ensure_rollout_materialized
          ▼
 ┌─────────────────┐
 │ Rollout 文件    │ JSONL 持久化
 └─────────────────┘
 ```

 ### 7.2.2 JSON 数据变换示例

 **输入（用户文本）：**
 ```
 /review
 ```

 **ReviewRequest JSON：**
 ```json
 {
   "target": { "type": "uncommittedChanges" }
 }
 ```

 **ResolvedReviewRequest（内部）：**
 ```rust
 ResolvedReviewRequest {
     target: UncommittedChanges,
     prompt: "Review the current code changes (staged, unstaged, and untracked files) and provide prioritized findings.",
     user_facing_hint: "current changes",
 }
 ```

 **API 请求 JSON（简化）：**
 ```json
 {
   "model": "gpt-5.4",
   "instructions": "# Review guidelines:\n\nYou are acting as a reviewer...",
   "input": [
     {
       "type": "message",
       "role": "user",
       "content": [{ "type": "input_text", "text": "Review the current code changes..." }]
     }
   ]
 }
 ```

 **子代理返回 JSON（ReviewOutputEvent）：**
 ```json
 {
   "findings": [
     {
       "title": "[P1] Buffer overflow in parse function",
       "body": "The `parse` function doesn't check buffer bounds...",
       "confidence_score": 0.95,
       "priority": 1,
       "code_location": {
         "absolute_file_path": "/home/user/repo/src/parser.rs",
         "line_range": { "start": 42, "end": 48 }
       }
     }
   ],
   "overall_correctness": "patch is incorrect",
   "overall_explanation": "The buffer overflow bug can cause memory corruption.",
   "overall_confidence_score": 0.9
 }
 ```

 **user_message（XML 模板）：**
 ```xml
 <user_action>
   <context>User initiated a review task. Here's the full review output from reviewer model. User may select one or more comments to resolve.</context>
   <action>review</action>
   <results>
   The buffer overflow bug can cause memory corruption.

 Full review comments:

 - [P1] Buffer overflow in parse function — /home/user/repo/src/parser.rs:42-48
   The `parse` function doesn't check buffer bounds...
   </results>
 </user_action>
 ```

 **assistant_message（纯文本）：**
 ```
 The buffer overflow bug can cause memory corruption.

 Full review comments:

 - [P1] Buffer overflow in parse function — /home/user/repo/src/parser.rs:42-48
   The `parse` function doesn't check buffer bounds...
 ```

 **Rollout JSONL（两条记录）：**
 ```json
 {"timestamp":"...","type":"response_item","payload":{"type":"message","role":"user","id":"review_rollout_user","content":[{"type":"input_text","text":"<user_action>..."}]}}
 {"timestamp":"...","type":"response_item","payload":{"type":"message","role":"assistant","id":"review_rollout_assistant","content":[{"type":"output_text","text":"The buffer overflow..."}]}}
 ```

 ---

 ## 7.3 事件流过滤图

 ### 7.3.1 事件过滤管道（ASCII）

 ```
 子代理产生的事件流
 ═══════════════════════════════════════════════════════
 │ 1. AgentMessageContentDelta                    ──┐
 │ 2. AgentMessageDelta                           ──┤
 │ 3. ItemCompleted(AgentMessage)                 ──┤
 │ 4. AgentMessage("partial")                     ──┤
 │ 5. AgentMessageContentDelta                    ──┤
 │ 6. AgentMessage("final JSON")                  ──┤
 │ 7. ItemCompleted(AgentMessage)                 ──┤
 │ 8. TurnComplete { last_agent_message }         ──┤
 ═══════════════════════════════════════════════════════│
                                                      │
                    process_review_events              │
                    ┌─────────────────┐                │
                    │  事件过滤器      │◄───────────────┘
                    └────────┬────────┘
                             │
           ┌─────────────────┼─────────────────┐
           │                 │                 │
      抑制(Drop)         暂存(Buffer)      转发(Forward)
           │                 │                 │
           ▼                 ▼                 ▼
 ┌──────────────────┐ ┌────────────┐  ┌──────────────┐
 │ AgentMessageDelta│ │ AgentMessage│  │ 其他事件     │
 │ AgentMessageCont │ │ (暂存,      │  │ (工具调用等) │
 │ Delta            │ │  只保留最后 │  │              │
 │ ItemCompleted    │ │  一条)      │  │              │
 │ (AgentMessage)   │ └──────┬─────┘  └──────┬───────┘
 └──────────────────┘        │               │
                              │               │
                    TurnComplete              │
                              │               │
                              ▼               │
                    ┌────────────┐            │
                    │ parse_     │            │
                    │ review_    │            │
                    │ output_    │            │
                    │ event()    │            │
                    └──────┬─────┘            │
                           │                  │
                           ▼                  │
                    ┌────────────┐            │
                    │ ReviewOutput│           │
                    │ Event      │            │
                    └──────┬─────┘            │
                           │                  │
                           ▼                  ▼
                    ┌─────────────────────────────┐
                    │       主会话事件流           │
                    │  (ExitedReviewMode + 其他)   │
                    └─────────────────────────────┘
 ```

 ### 7.3.2 事件过滤规则表

 | 事件类型 | 过滤规则 | 转发到主会话 | 原因 |
 |----------|---------|-------------|------|
 | AgentMessageContentDelta | Drop | 否 | 流式增量，审查不需要 |
 | AgentMessageDelta | Drop | 否 | legacy 流式增量 |
 | ItemCompleted(AgentMessage) | Drop | 否 | 避免触发 legacy AgentMessage |
 | AgentMessage | Buffer | 最终丢弃 | 只用 TurnComplete 的 last_agent_message |
 | TurnComplete | Terminal | 否（触发解析） | 从 last_agent_message 解析输出 |
 | TurnAborted | Terminal | 否（返回 None） | 中断信号 |
 | 其他（工具调用等） | Forward | 是 | 审查过程中的工具调用可见 |

 ---

 > **一句话回顾**：状态机图展示了审查从 IDLE→RESOLVING→SPAWNING→REVIEWING→PARSING→EXITING→IDLE 的完整生命周期（含 ERROR 和 ABORTING 分支），数据流图展示了从用户文本到 rollout JSONL 的 10 步变换链，事件过滤图展示了 7 类事件如何被 Drop/Buffer/Forward/Terminal 处理。

 ---

 # 第八部分 · 测试代码走读

 本部分对 `core/tests/suite/review.rs` 中的关键测试进行逐行走读，帮助新手理解测试设计。

 ---

 ## 8.1 测试基础设施

 ### 8.1.1 mock SSE 服务器

 ```rust
 async fn start_responses_server_with_sse(
     sse_raw: &str,
     expected_requests: usize,
 ) -> (MockServer, ResponseMock) {
     let server = start_mock_server().await;
     let sse = load_sse_fixture_with_id_from_str(sse_raw, &Uuid::new_v4().to_string());
     let responses = vec![sse; expected_requests];
     let request_log = mount_sse_sequence(&server, responses).await;
     (server, request_log)
 }
 ```

 **走读：**

 1. `start_mock_server()`：启动 wiremock 服务器
 2. `load_sse_fixture_with_id_from_str(sse_raw, &uuid)`：把 SSE JSON 模板加载为 fixture
    - `sse_raw`：SSE 事件的 JSON 数组字符串
    - `&Uuid::new_v4().to_string()`：生成唯一 ID 替换 `__ID__` 占位符
 3. `vec![sse; expected_requests]`：复制 fixture（每个请求一个响应）
 4. `mount_sse_sequence(&server, responses)`：挂载 SSE 序列到 mock 服务器
    - 返回 `ResponseMock`，用于检查请求日志

 ### 8.1.2 会话创建

 ```rust
 async fn new_conversation_for_server<F>(
     server: &MockServer,
     agere_home: Arc<TempDir>,
     mutator: F,
 ) -> Arc<AgereThread>
 where
     F: FnOnce(&mut Config) + Send + 'static,
 {
     let base_url = format!("{}/v1", server.uri());
     let mut builder = test_agere()
         .with_home(agere_home)
         .with_config(move |config| {
             config.model_provider.base_url = Some(base_url.clone());
             mutator(config);
         });
     builder
         .build(server)
         .await
         .expect("create conversation")
         .agere
 }
 ```

 **走读：**

 1. `base_url`：mock 服务器的 `/v1` 端点
 2. `test_agere()`：创建测试用 Agere 构建器
 3. `.with_home(agere_home)`：设置临时 home 目录
 4. `.with_config(mutator)`：配置回调
   - `base_url`：指向 mock 服务器
   - `mutator(config)`：调用者传入的配置修改函数
 5. `.build(server).await`：构建会话
 6. `.agere`：提取 `AgereThread`

 ### 8.1.3 resume 会话创建

 ```rust
 async fn resume_conversation_for_server<F>(
     server: &MockServer,
     agere_home: Arc<TempDir>,
     resume_path: std::path::PathBuf,
     mutator: F,
 ) -> Arc<AgereThread>
 where
     F: FnOnce(&mut Config) + Send + 'static,
 {
     // 同 new_conversation_for_server，但从 resume_path 恢复
     let mut builder = test_agere()
         .with_home(agere_home.clone())
         .with_config(move |config| { ... });
     builder
         .resume(server, agere_home, resume_path)
         .await
         .expect("resume conversation")
         .agere
 }
 ```

 **走读：**

 - 与 `new_conversation_for_server` 类似，但从 `resume_path`（rollout 文件）恢复会话
 - 用于测试历史隔离（`review_input_isolated_from_parent_history`）

 ---

 ## 8.2 关键测试逐行走读

 ### 8.2.1 `review_op_emits_lifecycle_and_review_output` 走读

 ```rust
 #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
 async fn review_op_emits_lifecycle_and_review_output() {
     skip_if_no_network!();
 ```

 **走读：**

 - `tokio::test(flavor = "multi_thread", worker_threads = 2)`：多线程异步测试
 - `skip_if_no_network!()`：网络禁用时跳过（CI 限制）

 ```rust
     let review_json = serde_json::json!({
         "findings": [
             {
                 "title": "Prefer Stylize helpers",
                 "body": "Use .dim()/.bold() chaining instead of manual Style where possible.",
                 "confidence_score": 0.9,
                 "priority": 1,
                 "code_location": {
                     "absolute_file_path": "/tmp/file.rs",
                     "line_range": {"start": 10, "end": 20}
                 }
             }
         ],
         "overall_correctness": "good",
         "overall_explanation": "All good with some improvements suggested.",
         "overall_confidence_score": 0.8
     })
     .to_string();
 ```

 **走读：**

 - 构造 `ReviewOutputEvent` 的 JSON 字符串
 - 这是子代理应该返回的结构化审查结果
 - 包含 1 个 finding（title、body、confidence_score、priority、code_location）

 ```rust
     let sse_template = r#"[{"type":"response.output_item.done", "item":{
         "type":"message", "role":"assistant",
         "content":[{"type":"output_text","text":__REVIEW__}]}},
         {"type":"response.completed", "response": {"id": "__ID__"}}]"#;
     let review_json_escaped = serde_json::to_string(&review_json).unwrap();
     let sse_raw = sse_template.replace("__REVIEW__", &review_json_escaped);
 ```

 **走读：**

 - `sse_template`：SSE 事件模板
   - `response.output_item.done`：助手消息完成
   - `response.completed`：响应完成
   - `__REVIEW__`：审查 JSON 占位符
   - `__ID__`：响应 ID 占位符
 - `review_json_escaped`：二次 JSON 编码（因为 JSON 嵌套在字符串中）
 - `sse_raw`：替换占位符后的完整 SSE JSON

 ```rust
     let (server, _request_log) =
         start_responses_server_with_sse(&sse_raw, /*expected_requests*/ 1).await;
     let agere_home = Arc::new(TempDir::new().unwrap());
     let agere = new_conversation_for_server(&server, agere_home.clone(), |_| {}).await;
 ```

 **走读：**

 - 启动 mock 服务器（期望 1 个请求）
 - 创建会话（无配置修改）

 ```rust
     agere
         .submit(Op::Review {
             review_request: ReviewRequest {
                 target: ReviewTarget::Custom {
                     instructions: "Please review my changes".to_string(),
                 },
                 user_facing_hint: None,
             },
         })
         .await
         .unwrap();
 ```

 **走读：**

 - 提交 `Op::Review`
   - `target: Custom { instructions: "Please review my changes" }`
   - `user_facing_hint: None`
 - 使用 `Custom` 而非 `UncommittedChanges`，因为测试不需要真实 git 仓库

 ```rust
     let _entered = wait_for_event(&agere, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
     let closed = wait_for_event(&agere, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
     let review = match closed {
         EventMsg::ExitedReviewMode(ev) => ev
             .review_output
             .expect("expected ExitedReviewMode with Some(review_output)"),
         other => panic!("expected ExitedReviewMode(..), got {other:?}"),
     };
 ```

 **走读：**

 - `wait_for_event`：等待特定事件
   - 先等 `EnteredReviewMode`（确认进入审查模式）
   - 再等 `ExitedReviewMode`（确认退出审查模式）
 - 从 `ExitedReviewMode` 提取 `review_output`
   - `expect("Some")`：断言有审查结果

 ```rust
     let expected = ReviewOutputEvent {
         findings: vec![ReviewFinding {
             title: "Prefer Stylize helpers".to_string(),
             body: "Use .dim()/.bold() chaining instead of manual Style where possible.".to_string(),
             confidence_score: 0.9,
             priority: 1,
             code_location: ReviewCodeLocation {
                 absolute_file_path: PathBuf::from("/tmp/file.rs"),
                 line_range: ReviewLineRange { start: 10, end: 20 },
             },
         }],
         overall_correctness: "good".to_string(),
         overall_explanation: "All good with some improvements suggested.".to_string(),
         overall_confidence_score: 0.8,
     };
     assert_eq!(expected, review);
 ```

 **走读：**

 - 构造预期的 `ReviewOutputEvent`
 - `assert_eq!(expected, review)`：深度比较
   - 使用 `pretty_assertions::assert_eq`（清晰的 diff）
   - `PartialEq` 对 `f32` 在精确值上可用
   - 验证所有字段：findings（含 title、body、confidence_score、priority、code_location）、overall_correctness、overall_explanation、overall_confidence_score

 ```rust
     let _complete = wait_for_event(&agere, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
 ```

 **走读：**

 - 等待 `TurnComplete` 确认 turn 结束

 ```rust
     // 验证 rollout 中的记录
     let path = agere.rollout_path().expect("rollout path");
     let text = std::fs::read_to_string(&path).expect("read rollout file");
     // ... 逐行解析 JSONL，检查:
     //   saw_header: user message 含 "full review output from reviewer model"
     //   saw_finding_line: 含 "- Prefer Stylize helpers — /tmp/file.rs:10-20"
     //   saw_assistant_plain: assistant message == render_review_output_text(expected)
     //   saw_assistant_xml: assistant message 不含 <user_action>（应为 false）
 ```

 **走读：**

 - 读取 rollout 文件（JSONL）
 - 逐行解析为 `RolloutLine`
 - 检查 4 个条件：
   1. `saw_header`：user message 含成功模板的 context 文本
   2. `saw_finding_line`：含格式化后的 finding 行
   3. `saw_assistant_plain`：assistant message 是纯文本渲染结果
   4. `!saw_assistant_xml`：assistant message 不含 XML 标记（XML 只在 user message 中）

 ```rust
     server.verify().await;
 }
 ```

 **走读：**

 - `server.verify()`：验证 mock 服务器收到了期望数量的请求

 ### 8.2.2 `review_input_isolated_from_parent_history` 走读

 这个测试验证审查子代理不携带主会话历史。

 ```rust
     // 1. 创建有历史记录的 resume 文件
     let session_file = agere_home.path().join("resume.jsonl");
     {
         let mut f = tokio::fs::File::create(&session_file).await.unwrap();
         let convo_id = Uuid::new_v4();

         // session_meta 行
         let meta_line = serde_json::json!({
             "timestamp": "2024-01-01T00:00:00.000Z",
             "type": "session_meta",
             "payload": { "id": convo_id, ... }
         });
         f.write_all(format!("{meta_line}\n").as_bytes()).await.unwrap();

         // 先前的 user message
         let user = ResponseItem::Message { role: "user", content: [InputText { text: "parent: earlier user message" }], ... };
         // ... 写入 JSONL

         // 先前的 assistant message
         let assistant = ResponseItem::Message { role: "assistant", content: [OutputText { text: "parent: assistant reply" }], ... };
         // ... 写入 JSONL
     }
 ```

 **走读：**

 - 手动构造一个有 2 条历史消息的 rollout 文件
   - 1 条 user message: "parent: earlier user message"
   - 1 条 assistant message: "parent: assistant reply"

 ```rust
     // 2. 从 resume 文件恢复会话
     let agere = resume_conversation_for_server(&server, agere_home.clone(), session_file.clone(), |_| {}).await;

     // 3. 提交审查请求
     let review_prompt = "Please review only this".to_string();
     agere.submit(Op::Review {
         review_request: ReviewRequest {
             target: ReviewTarget::Custom { instructions: review_prompt.clone() },
             user_facing_hint: None,
         },
     }).await.unwrap();
 ```

 **走读：**

 - 从有历史的 rollout 恢复会话
 - 提交审查请求（Custom 指令 = "Please review only this"）

 ```rust
     // 4. 检查请求 input
     let request = request_log.single_request();
     let body = request.body_json();
     let input = body["input"].as_array().expect("input array");

     // input 应包含:
     //   - environment context（含 <cwd>）
     //   - review prompt（"Please review only this"）
     // 但不应包含:
     //   - "parent: earlier user message"
     //   - "parent: assistant reply"
 ```

 **走读：**

 - 检查发送给 mock 服务器的请求体
 - `input` 数组应包含 environment context 和 review prompt
 - 不应包含主会话的历史消息（验证隔离）

 ```rust
     // 5. 验证 instructions == REVIEW_PROMPT
     let instructions = body["instructions"].as_str().expect("instructions string");
     assert_eq!(instructions, REVIEW_PROMPT);
 ```

 **走读：**

 - 验证系统指令是 `REVIEW_PROMPT`（审查专用系统 prompt）

 ```rust
     // 6. 验证 rollout 中有中断消息
     // (因为 mock 只返回 response.completed，无 last_agent_message)
     // → exit_review_mode(None) → 中断模板
     // → rollout 中应有 "User initiated a review task, but was interrupted."
 ```

 **走读：**

 - mock 返回 `response.completed` 但无 `output_item.done`
 - 子代理的 `last_agent_message` 为 `None`
 - `parse_review_output_event(None)` → `None`
 - `exit_review_mode(None)` → 中断模板
 - rollout 中应有中断消息

 ---

 ## 8.3 测试设计模式总结

 ### 8.3.1 测试模式分类

 | 模式 | 描述 | 示例测试 |
 |------|------|---------|
 | 生命周期验证 | 验证事件序列 | review_op_emits_lifecycle |
 | 兜底降级 | 验证非 JSON 输出 | review_op_with_plain_text |
 | 事件抑制 | 验证不该出现的事件 | review_filters_agent_message |
 | 配置生效 | 验证配置项 | review_uses_custom_review_model |
 | 隔离验证 | 验证不携带历史 | review_input_isolated |
 | 持久化验证 | 验证 rollout 记录 | review_history_surfaces |
 | 端到端集成 | 验证完整流程 | review_uses_overridden_cwd |

 ### 8.3.2 测试工具链

 | 工具 | 用途 | 来源 |
 |------|------|------|
 | `wiremock::MockServer` | mock HTTP 服务器 | wiremock crate |
 | `core_test_support::responses::*` | SSE mock 工具 | 项目内部 |
 | `wait_for_event` | 等待特定事件 | core_test_support |
 | `skip_if_no_network!` | 网络禁用时跳过 | core_test_support |
 | `test_agere()` | 测试 Agere 构建器 | core_test_support |
 | `TempDir` | 临时目录 | tempfile crate |
 | `pretty_assertions::assert_eq` | 清晰的 diff | pretty_assertions crate |

 ### 8.3.3 测试最佳实践

 1. **使用 `skip_if_no_network!`**：CI 可能禁用网络，跳过而非失败
 2. **使用 `wait_for_event` 而非 `sleep`**：事件驱动而非时间驱动
 3. **深度比较整个对象**：`assert_eq!(expected, actual)` 而非逐字段比较
 4. **验证 rollout 持久化**：不只检查内存状态，还检查磁盘记录
 5. **Windows CI 特殊处理**：`#[cfg_attr(windows, tokio::test(..., worker_threads = 4))]`
 6. **mock 请求验证**：`server.verify().await` 确保期望的请求被发送
 7. **使用 `ResponseMock` 检查请求体**：`request.body_json()` 验证 API 请求内容

 ---

 > **一句话回顾**：测试代码走读展示了 7 种测试模式（生命周期/兜底/抑制/配置/隔离/持久化/集成），使用 wiremock + core_test_support 工具链，通过 `wait_for_event` 事件驱动验证、深度比较整个对象、检查 rollout 磁盘记录来确保审查流程的正确性。

 ---

 # 第九部分 · 调试与排障指南

 本部分提供调试 `/review` 问题的实用指南。

 ---

 ## 9.1 常见问题排查

 ### 9.1.1 审查不启动

 **症状：** 输入 `/review` 后没有任何反应。

 **排查步骤：**

 1. **检查 resolve 是否失败**：
    - 如果 target 是 `BaseBranch`，检查 `merge_base_with_head` 是否失败
    - 如果 target 是 `Custom`，检查指令是否为空
    - 查看 TUI 是否显示了 `ErrorEvent`

 2. **检查模型配置**：
    - `review_model` 是否指向一个有效的模型
    - 如果 `review_model = None`，主会话模型是否可用

 3. **检查认证**：
    - `start_review_conversation` 调用 `run_agere_thread_one_shot`
    - 如果认证失败，返回 `Err` → `None` → 走中断流程
    - 查看日志中是否有认证错误

 4. **检查 Constrained 约束**：
    - 如果 `ConfigRequirements` 不允许 `WebSearchMode::Disabled`
    - task 层会 `panic!`
    - session 层会 `warn!` + 保持原值

 ### 9.1.2 审查结果为空

 **症状：** 审查完成但显示 "Reviewer failed to output a response."

 **排查步骤：**

 1. **检查子代理是否返回了内容**：
    - `TurnComplete.last_agent_message` 是否为 `Some`
    - 如果为 `None`，说明子代理没有产出消息

 2. **检查 JSON 解析**：
    - 子代理返回的内容是否是有效的 JSON
    - `parse_review_output_event` 的三级降级是否都失败了
    - 如果返回纯文本，应该走兜底（`overall_explanation`），不会显示 "failed"

 3. **检查 ReviewOutputEvent 是否全空**：
    - `findings = []` 且 `overall_explanation = ""`
    - 这会导致 `render_review_output_text` 返回 fallback 消息
    - 可能原因：子代理返回了 `{}` 空对象

 ### 9.1.3 审查被意外中断

 **症状：** 审查过程中显示 "Review was interrupted."

 **排查步骤：**

 1. **检查是否用户取消**：
    - `cancellation_token` 是否被触发
    - 用户是否按了 Ctrl+C

 2. **检查 channel 是否异常关闭**：
    - 子代理是否崩溃
    - `receiver.recv()` 是否返回 `Err`

 3. **检查 TurnAborted 事件**：
    - 子代理是否主动发送了 `TurnAborted`
    - 可能是子代理遇到了不可恢复的错误

 ### 9.1.4 审查结果不正确

 **症状：** 审查结果不符合预期（findings 缺失或错误）。

 **排查步骤：**

 1. **检查 review_prompt.md**：
    - 系统 prompt 是否被修改
    - 输出 schema 是否与 `ReviewOutputEvent` 匹配

 2. **检查审查模型**：
    - `review_model` 是否指向一个能力足够的模型
    - 模型是否支持 JSON 结构化输出

 3. **检查代码改动**：
    - 子代理是否正确读取了 git diff
    - `code_location` 是否与实际改动重叠

 4. **检查 JSON 提取**：
    - 如果子代理返回的 JSON 嵌在文本中
    - `parse_review_output_event` 策略 2（提取 {} 子串）是否正确提取

 ---

 ## 9.2 日志与追踪

 ### 9.2.1 关键日志点

 | 日志点 | 级别 | 位置 | 含义 |
 |--------|------|------|------|
 | `agere.task.review` counter | info | tasks/review.rs:48 | 审查任务启动 |
 | `session_task.review` span | trace | tasks/review.rs:60 | 审查任务 span |
 | web_search_mode warn | warn | session/review.rs:70 | web search 禁用失败 |
 | "Reviewer failed to output" | error | chatwidget.rs:8163 | 审查结果为空 |
 | ExitedReviewMode(None) | info | tasks/review.rs:222 | 审查中断 |
 | ExitedReviewMode(Some) | info | tasks/review.rs:222 | 审查完成 |

 ### 9.2.2 追踪审查流程

 ```
 1. 搜索 "agere.task.review" → 找到审查任务启动
 2. 搜索 "session_task.review" → 找到审查 span
 3. 搜索 "EnteredReviewMode" → 确认进入审查模式
 4. 搜索 "ExitedReviewMode" → 确认退出审查模式
    - review_output: Some → 成功
    - review_output: None → 中断
 5. 搜索 "SubAgentSource::Review" 或 "review" → 找到子代理相关日志
 6. 搜索 "parse_review_output" → 找到输出解析
 7. 搜索 "exit_review_mode" → 找到退出流程
 ```

 ### 9.2.3 断点调试建议

 在以下位置设置断点：

 1. `handlers::review()` — 审查入口
 2. `resolve_review_request()` — 请求解析
 3. `spawn_review_thread()` — 上下文构建
 4. `ReviewTask::run()` — 任务执行
 5. `start_review_conversation()` — 子代理启动
 6. `process_review_events()` — 事件消费
 7. `parse_review_output_event()` — 输出解析
 8. `exit_review_mode()` — 退出回灌
 9. `on_exited_review_mode()` — UI 渲染

 ---

 ## 9.3 性能分析

 ### 9.3.1 审查耗时分析

 审查的主要耗时在子代理执行（阶段 4）：

 ```
 阶段 1-3（命令解析+请求解析+上下文构建）：~1-2 秒（同步）
 阶段 4（子代理执行）：~10-50 秒（异步，取决于改动大小和模型速度）
 阶段 5-7（事件消费+退出+UI）：~1-2 秒（同步）
 ```

 **优化建议：**

 1. **使用更强的审查模型**：配置 `review_model` 为推理能力强的模型
 2. **减少改动范围**：审查特定 commit 而非整个 working tree
 3. **避免大 diff**：大 diff 会导致子代理处理时间过长

 ### 9.3.2 Token 消耗分析

 审查的 token 消耗：

 - **系统 prompt**（`REVIEW_PROMPT`）：~2000-3000 tokens
 - **用户 prompt**（审查指令）：~50-200 tokens
 - **子代理执行**（git diff + 分析）：~5000-50000 tokens（取决于改动大小）
 - **审查输出**（JSON findings）：~500-5000 tokens

 **注意：** 审查的 token 消耗不计入主会话的 token 统计（通过 `pre_review_token_info` 保存/恢复机制隔离）。

 ---

 > **一句话回顾**：调试指南覆盖 4 类常见问题（不启动/结果为空/意外中断/结果不正确），提供 6 个关键日志点和 9 个断点位置，性能分析指出子代理执行是主要耗时（10-50 秒），token 消耗通过 `pre_review_token_info` 机制与主会话隔离。

 ---
 ---

 # 第十部分 · session/review.rs 逐行注解

 `core/src/session/review.rs` 是审查上下文构建的核心，包含 `spawn_review_thread` 的完整实现。这个函数构建了一个高度隔离的 `TurnContext`，是理解审查子代理如何与主会话隔离的关键。

 ---

 ## 10.1 导入与函数签名

 ```rust
 use super::turn_context::image_generation_tool_auth_allowed;
 use super::*;
 use std::sync::atomic::AtomicBool;

 /// Spawn a review thread using the given prompt.
 pub(super) async fn spawn_review_thread(
     sess: Arc<Session>,
     parent_turn_context: Arc<TurnContext>,
     sub_id: String,
     resolved: crate::review_prompts::ResolvedReviewRequest,
 ) {
 ```

 **注解：**

 - `pub(super)`：仅在 session 模块内可见（handlers.rs 调用）
 - `sess: Arc<Session>`：会话引用（共享）
 - `parent_turn_context: Arc<TurnContext>`：父 turn 上下文（继承配置）
 - `sub_id: String`：子 turn ID
 - `resolved: ResolvedReviewRequest`：解析后的审查请求（含 prompt 和 hint）

 ---

 ## 10.2 模型确定（L14-19）

 ```rust
     let config = parent_turn_context.config.clone();
     let model = config
         .review_model
         .clone()
         .unwrap_or_else(|| parent_turn_context.model_info.slug.clone());
     let review_model_info = sess
         .services
         .models_manager
         .get_model_info(&model, &config.to_models_manager_config())
         .await;
 ```

 **注解：**

 - `config = parent_turn_context.config.clone()`：clone `Arc<Config>`（引用计数+1）
 - `model`：审查模型确定
   - `config.review_model`：用户配置的审查模型
   - `.unwrap_or_else(|| parent_turn_context.model_info.slug.clone())`：fallback 到主会话模型
 - `review_model_info`：获取模型详细信息
   - `models_manager.get_model_info(&model, ...)`：查询模型能力
   - 用于后续构建 `ToolsConfig`（不同模型支持不同工具）
   - `.await`：异步操作（可能需要网络查询）

 ---

 ## 10.3 Feature 裁剪（L21-26）

 ```rust
     // For reviews, disable web_search and view_image regardless of global settings.
     let mut review_features = sess.features.clone();
     let _ = review_features.disable(Feature::WebSearchRequest);
     let _ = review_features.disable(Feature::WebSearchCached);
     let review_web_search_mode = WebSearchMode::Disabled;
 ```

 **注解：**

 - `review_features = sess.features.clone()`：clone 会话级 features
 - `disable(WebSearchRequest)`：禁用 web search 请求 feature
 - `disable(WebSearchCached)`：禁用 web search 缓存 feature
 - `review_web_search_mode = Disabled`：web search 模式设为禁用
 - 注释说"regardless of global settings"：无论全局设置如何，审查都禁用这些
 - `let _ =`：忽略返回值（可能已经禁用，返回 Err 也无所谓）

 ---

 ## 10.4 ToolsConfig 构建（L28-100）

 这是 `spawn_review_thread` 中最长的部分，构建审查子代理的工具配置。

 ```rust
     let provider_capabilities = parent_turn_context.provider.capabilities();
     let tools_config = ToolsConfig::new(&ToolsConfigParams {
         model_info: &review_model_info,
         available_models: &sess
             .services
             .models_manager
             .list_models(RefreshStrategy::OnlineIfUncached)
             .await,
         features: &review_features,
         image_generation_tool_auth_allowed: image_generation_tool_auth_allowed(Some(
             sess.services.auth_manager.as_ref(),
         )),
         web_search_mode: Some(review_web_search_mode),
         session_source: parent_turn_context.session_source.clone(),
         permission_profile: &parent_turn_context.permission_profile,
         windows_execution_restriction_level: parent_turn_context
             .windows_execution_restriction_level,
     })
 ```

 **注解：**

 - `provider_capabilities`：提供商能力（namespace tools、image gen、web search 等）
 - `ToolsConfig::new(ToolsConfigParams { ... })`：构建工具配置
   - `model_info`：审查模型信息（决定工具兼容性）
   - `available_models`：可用模型列表（OnlineIfUncached 策略）
   - `features`：裁剪后的 features（web search 已禁用）
   - `image_generation_tool_auth_allowed`：图片生成工具认证
   - `web_search_mode`：`Some(Disabled)` — 禁用 web search
   - `session_source`：会话来源
   - `permission_profile`：权限配置
   - `windows_execution_restriction_level`：Windows 执行限制

 ```rust
     .with_namespace_tools_capability(provider_capabilities.namespace_tools)
     .with_image_generation_capability(provider_capabilities.image_generation)
     .with_web_search_capability(provider_capabilities.web_search)
     .with_unified_exec_shell_mode_for_session(
         crate::tools::spec::tool_user_shell_type(sess.services.user_shell.as_ref()),
         sess.services.shell_zsh_path.as_ref(),
         sess.services.main_execve_wrapper_exe.as_ref(),
     )
     .with_web_search_config(/*web_search_config*/ None)
     .with_allow_login_shell(config.permissions.allow_login_shell)
     .with_spawn_agent_usage_hint(config.multi_agent_v2.usage_hint_enabled)
     .with_spawn_agent_usage_hint_text(config.multi_agent_v2.usage_hint_text.clone())
     .with_hide_spawn_agent_metadata(config.multi_agent_v2.hide_spawn_agent_metadata)
     .with_goal_tools_allowed(false)
     .with_max_concurrent_threads_per_session(config.agent_max_threads)
     .with_wait_agent_min_timeout_ms(
         review_features
             .enabled(Feature::MultiAgentV2)
             .then_some(config.multi_agent_v2.min_wait_timeout_ms),
     )
     .with_agent_type_description(crate::agent::role::spawn_tool_spec::build(
         &config.agent_roles,
     ));
 ```

 **注解：**

 链式配置方法详解：

 - `with_namespace_tools_capability`：命名空间工具能力
 - `with_image_generation_capability`：图片生成能力
 - `with_web_search_capability`：web search 能力（虽然传入 provider 能力，但因 `web_search_mode = Disabled` 实际不启用）
 - `with_unified_exec_shell_mode_for_session`：统一 shell 执行模式
   - `tool_user_shell_type`：用户 shell 类型
   - `shell_zsh_path`：zsh 路径
   - `main_execve_wrapper_exe`：execve 包装器
 - `with_web_search_config(None)`：不配置 web search
 - `with_allow_login_shell`：是否允许 login shell
 - `with_spawn_agent_usage_hint`：spawn agent 使用提示
 - `with_spawn_agent_usage_hint_text`：提示文本
 - `with_hide_spawn_agent_metadata`：是否隐藏 spawn agent 元数据
 - `with_goal_tools_allowed(false)`：**审查不允许 goal 工具**
 - `with_max_concurrent_threads_per_session`：最大并发线程数
 - `with_wait_agent_min_timeout_ms`：wait agent 最小超时
   - `.then_some(...)`：只有 MultiAgentV2 启用时才设超时
 - `with_agent_type_description`：agent 类型描述

 ---

 ## 10.5 per-turn config 构建（L102-115）

 ```rust
     let review_prompt = resolved.prompt.clone();
     let provider = parent_turn_context.provider.clone();
     let auth_manager = parent_turn_context.auth_manager.clone();
     let model_info = review_model_info.clone();

     // Build per-turn client with the requested model/family.
     let mut per_turn_config = (*config).clone();
     per_turn_config.model = Some(model.clone());
     per_turn_config.features = review_features.clone();
 ```

 **注解：**

 - `review_prompt`：clone 审查 prompt（后续作为 input）
 - `provider`：clone 提供商
 - `auth_manager`：clone 认证管理器
 - `model_info`：clone 模型信息
 - `per_turn_config = (*config).clone()`：clone Config 本体（解引用 Arc 后深拷贝）
 - `per_turn_config.model = Some(model)`：设置审查模型
 - `per_turn_config.features = review_features`：设置裁剪后的 features

 ```rust
     if let Err(err) = per_turn_config.web_search_mode.set(review_web_search_mode) {
         let fallback_value = per_turn_config.web_search_mode.value();
         tracing::warn!(
             error = %err,
             ?review_web_search_mode,
             ?fallback_value,
             "review web_search_mode is disallowed by requirements; keeping constrained value"
         );
     }
 ```

 **注解：**

 - `web_search_mode.set(Disabled)`：
   - 成功：`Ok(())` — web search 被禁用
   - 失败：`Err(err)` — ConfigRequirements 不允许 Disabled
     - `warn!` 记录警告
     - `fallback_value = per_turn_config.web_search_mode.value()`：获取当前值
     - 保持原值（可能仍为 Enabled）
 - **与 task 层的区别**：task 层 panic，session 层 warn + 保持原值

 ```rust
     let session_telemetry = parent_turn_context
         .session_telemetry
         .clone()
         .with_model(model.as_str(), review_model_info.slug.as_str());
 ```

 **注解：**

 - `session_telemetry`：clone 遥测并设置模型信息
   - `.with_model(model, slug)`：记录审查使用的模型

 ---

 ## 10.6 TurnContext 构建（L117-160）

 ```rust
     let auth_manager_for_context = auth_manager.clone();
     let provider_for_context = provider.clone();
     let session_telemetry_for_context = session_telemetry.clone();
     let reasoning_effort = per_turn_config.model_reasoning_effort;
     let reasoning_summary = per_turn_config
         .model_reasoning_summary
         .unwrap_or(model_info.default_reasoning_summary);
     let session_source = parent_turn_context.session_source.clone();

     let per_turn_config = Arc::new(per_turn_config);
     let review_turn_id = sub_id.to_string();
 ```

 **注解：**

 - 准备 TurnContext 的各字段
 - `reasoning_effort`：推理强度（从 config）
 - `reasoning_summary`：推理摘要（config 或模型默认）
 - `per_turn_config = Arc::new(per_turn_config)`：包装为 Arc（共享）
 - `review_turn_id = sub_id.to_string()`：审查 turn ID

 ```rust
     let turn_metadata_state = Arc::new(TurnMetadataState::new(
         sess.conversation_id.to_string(),
         &session_source,
         review_turn_id.clone(),
         parent_turn_context.cwd.clone(),
         &parent_turn_context.permission_profile,
         parent_turn_context.windows_execution_restriction_level,
         parent_turn_context.network.is_some(),
     ));
 ```

 **注解：**

 - `TurnMetadataState`：turn 元数据状态
   - `conversation_id`：会话 ID
   - `session_source`：会话来源
   - `review_turn_id`：审查 turn ID
   - `cwd`：工作目录
   - `permission_profile`：权限配置
   - `windows_execution_restriction_level`：Windows 限制
   - `network.is_some()`：是否有网络

 ```rust
     let review_turn_context = TurnContext {
         sub_id: review_turn_id,
         trace_id: current_span_trace_id(),
         realtime_active: parent_turn_context.realtime_active,
         config: per_turn_config,
         auth_manager: auth_manager_for_context,
         model_info: model_info.clone(),
         session_telemetry: session_telemetry_for_context,
         provider: provider_for_context,
         reasoning_effort,
         reasoning_summary,
         session_source,
         environment: parent_turn_context.environment.clone(),
         environments: parent_turn_context.environments.clone(),
         tools_config,
         features: parent_turn_context.features.clone(),
         ghost_snapshot: parent_turn_context.ghost_snapshot.clone(),
         current_date: parent_turn_context.current_date.clone(),
         timezone: parent_turn_context.timezone.clone(),
         app_server_client_name: parent_turn_context.app_server_client_name.clone(),
         developer_instructions: None,
         user_instructions: None,
         compact_prompt: parent_turn_context.compact_prompt.clone(),
         collaboration_mode: parent_turn_context.collaboration_mode.clone(),
         personality: parent_turn_context.personality,
         approval_policy: parent_turn_context.approval_policy.clone(),
         permission_profile: parent_turn_context.permission_profile(),
         network: parent_turn_context.network.clone(),
         windows_execution_restriction_level: parent_turn_context
             .windows_execution_restriction_level,
         shell_environment_policy: parent_turn_context.shell_environment_policy.clone(),
         cwd: parent_turn_context.cwd.clone(),
         local_fs: parent_turn_context.local_fs.clone(),
         final_output_json_schema: None,
         agere_self_exe: parent_turn_context.agere_self_exe.clone(),
         agere_linux_exe: parent_turn_context.agere_linux_exe.clone(),
         tool_call_gate: Arc::new(ReadinessFlag::new()),
         dynamic_tools: parent_turn_context.dynamic_tools.clone(),
         truncation_policy: model_info.truncation_policy.into(),
         turn_metadata_state,
         turn_skills: TurnSkillsContext::new(parent_turn_context.turn_skills.outcome.clone()),
         turn_timing_state: Arc::new(TurnTimingState::default()),
         server_model_warning_emitted: AtomicBool::new(false),
         model_verification_emitted: AtomicBool::new(false),
         provider_changed_for_turn: false,
     };
 ```

 **注解：**

 TurnContext 字段详解：

 | 字段 | 来源 | 说明 |
 |------|------|------|
 | sub_id | review_turn_id | 审查 turn ID |
 | trace_id | current_span_trace_id() | 追踪 ID |
 | realtime_active | parent | 实时模式（继承） |
 | config | per_turn_config | 审查专用配置（含 model + features） |
 | auth_manager | parent | 认证管理器（继承） |
 | model_info | review_model_info | 审查模型信息 |
 | session_telemetry | clone | 遥测（含模型信息） |
 | provider | parent | 提供商（继承） |
 | reasoning_effort | config | 推理强度 |
 | reasoning_summary | config/model | 推理摘要 |
 | session_source | parent | 会话来源（继承） |
 | environment | parent | 环境（继承） |
 | environments | parent | 环境列表（继承） |
 | tools_config | 构建 | 审查专用工具配置 |
 | **features** | **parent** | **注意：用父的，非裁剪后的** |
 | ghost_snapshot | parent | ghost 快照（继承） |
 | current_date | parent | 当前日期（继承） |
 | timezone | parent | 时区（继承） |
 | app_server_client_name | parent | app-server 客户端名（继承） |
 | **developer_instructions** | **None** | **审查不带 developer 指令** |
 | **user_instructions** | **None** | **审查不带 user 指令** |
 | compact_prompt | parent | compact prompt（继承） |
 | collaboration_mode | parent | 协作模式（继承） |
 | personality | parent | 人格（继承） |
 | approval_policy | parent | 审批策略（继承） |
 | permission_profile | parent | 权限配置（继承） |
 | network | parent | 网络（继承） |
 | windows_execution_restriction_level | parent | Windows 限制（继承） |
 | shell_environment_policy | parent | shell 环境策略（继承） |
 | cwd | parent | 工作目录（继承，含覆盖） |
 | local_fs | parent | 本地文件系统（继承） |
 | final_output_json_schema | None | 无输出 schema |
 | agere_self_exe | parent | agere 可执行文件（继承） |
 | agere_linux_exe | parent | Linux 可执行文件（继承） |
 | tool_call_gate | new | 新的 ReadinessFlag |
 | dynamic_tools | parent | 动态工具（继承） |
 | truncation_policy | model_info | 截断策略 |
 | turn_metadata_state | new | 新的元数据状态 |
 | turn_skills | new | 新的 skills 上下文 |
 | turn_timing_state | new | 新的计时状态 |
 | server_model_warning_emitted | new(false) | 模型警告标志 |
 | model_verification_emitted | new(false) | 模型验证标志 |
 | provider_changed_for_turn | false | 提供商未改变 |

 **关键隔离点（None 字段）：**
 - `developer_instructions: None` — 不带 AGENTS.md 指令
 - `user_instructions: None` — 不带用户指令
 - `final_output_json_schema: None` — 不用 schema 约束输出

 **关键继承点（从 parent）：**
 - `features` — 用父的 features（非裁剪后的 `review_features`）
 - `cwd` — 继承父的工作目录（含覆盖）
 - `approval_policy` — 继承父的审批策略
   - 注意：task 层会覆盖为 `Never`

 ---

 ## 10.7 spawn_task 与 emit（L162-179）

 ```rust
     // Seed the child task with the review prompt as the initial user message.
     let input: Vec<UserInput> = vec![UserInput::Text {
         text: review_prompt,
         // Review prompt is synthesized; no UI element ranges to preserve.
         text_elements: Vec::new(),
     }];
 ```

 **注解：**

 - 构造初始 input：
   - `UserInput::Text`：文本输入
   - `text: review_prompt`：审查 prompt
   - `text_elements: Vec::new()`：无 UI 元素范围（合成的 prompt）
   - 注释说"Review prompt is synthesized; no UI element ranges to preserve"

 ```rust
     let tc = Arc::new(review_turn_context);
     tc.turn_metadata_state.spawn_git_enrichment_task();
 ```

 **注解：**

 - `tc = Arc::new(review_turn_context)`：包装为 Arc
 - `spawn_git_enrichment_task()`：启动 git 元数据增强任务
   - 异步收集 git 信息（分支、commit 等）

 ```rust
     // TODO(ccunningham): Review turns currently rely on `spawn_task` for TurnComplete but do not
     // emit a parent TurnStarted. Consider giving review a full parent turn lifecycle
     // (TurnStarted + TurnComplete) for consistency with other standalone tasks.
     sess.spawn_task(tc.clone(), input, ReviewTask::new()).await;
 ```

 **注解：**

 - `sess.spawn_task(tc, input, ReviewTask::new())`：spawn 审查任务
   - `tc`：审查 turn 上下文
   - `input`：初始输入（审查 prompt）
   - `ReviewTask::new()`：审查任务实例
 - TODO 注释：审查 turn 目前不 emit TurnStarted，只有 TurnComplete
   - 与其他 standalone task 不一致
   - 未来可能改进为完整生命周期

 ```rust
     // Announce entering review mode so UIs can switch modes.
     let review_request = ReviewRequest {
         target: resolved.target,
         user_facing_hint: Some(resolved.user_facing_hint),
     };
     sess.send_event(&tc, EventMsg::EnteredReviewMode(review_request))
         .await;
 }
 ```

 **注解：**

 - 构造 `ReviewRequest`：
   - `target: resolved.target`：审查目标
   - `user_facing_hint: Some(resolved.user_facing_hint)`：UI 提示（总是 Some）
 - `send_event(EnteredReviewMode)`：emit 进入审查模式事件
   - 注释说"Announce entering review mode so UIs can switch modes"
   - 在 spawn_task 之后 emit，确保任务已启动

 ---

 > **一句话回顾**：`session/review.rs` 的逐行注解揭示了审查隔离上下文的构建——模型确定（review_model fallback）、feature 裁剪（禁 web search）、ToolsConfig 构建（15+ 个 with_ 链式配置，goal_tools_allowed=false）、TurnContext 构建（developer/user_instructions=None 隔离指令，features 用父的），最后 spawn_task + emit EnteredReviewMode。

 ---

 # 第十一部分 · review_prompt.md 审查准则详解

 本部分对 `core/review_prompt.md` 的审查准则进行逐条详解，帮助理解审查子代理的行为逻辑。

 ---

 ## 11.1 角色定义

 ```
 You are acting as a reviewer for a proposed code change made by another engineer.
 ```

 **详解：**

 - 定义子代理身份：审查者，而非执行者
 - "proposed code change"：审查的是提议的代码改动（尚未合并）
 - "another engineer"：假设改动是另一个工程师提交的
 - 设计意图：让子代理以客观第三方视角审查，而非以作者视角

 ---

 ## 11.2 Bug 判定 8 条准则

 ### 准则 1：实质性影响

 ```
 1. It meaningfully impacts the accuracy, performance, security, or maintainability of the code.
 ```

 **详解：**

 - 只标记对以下方面有**实质性影响**的问题：
   - accuracy（准确性）：逻辑错误、计算错误
   - performance（性能）：明显的性能退化
   - security（安全）：安全漏洞
   - maintainability（可维护性）：严重的设计问题
 - 不标记琐碎问题（如风格、格式）

 ### 准则 2：离散且可操作

 ```
 2. The bug is discrete and actionable (i.e. not a general issue with the codebase or a combination of multiple issues).
 ```

 **详解：**

 - bug 必须是**离散的**：可以单独定位和修复
 - bug 必须是**可操作的**：有明确的修复方向
 - 不标记"整个代码库的普遍问题"（太宽泛）
 - 不标记"多个问题的组合"（无法单独修复）

 ### 准则 3：严谨度匹配

 ```
 3. Fixing the bug does not demand a level of rigor that is not present in the rest of the codebase
    (e.g. one doesn't need very detailed comments and input validation in a repository of one-off scripts in personal projects)
 ```

 **详解：**

 - 修复的严谨度应与代码库现有水平**匹配**
 - 如果代码库是个人脚本项目，不应要求"详细的注释和输入验证"
 - 如果代码库是企业级项目，可以要求更高的严谨度
 - 设计意图：避免对简单项目过度要求

 ### 准则 4：本次引入

 ```
 4. The bug was introduced in the commit (pre-existing bugs should not be flagged).
 ```

 **详解：**

 - 只标记**本次改动引入的** bug
 - **预存的 bug 不应标记**（不是本次审查的责任）
 - 设计意图：审查聚焦于当前改动，不扩大范围

 ### 准则 5：作者会修复

 ```
 5. The author of the original PR would likely fix the issue if they were made aware of it.
 ```

 **详解：**

 - 标记的问题应该是**作者知道后会修复的**
 - 如果作者知道后不会修复（如故意的设计选择），不应标记
 - 设计意图：避免标记无意义的"问题"

 ### 准则 6：不依赖未声明假设

 ```
 6. The bug does not rely on unstated assumptions about the codebase or author's intent.
 ```

 **详解：**

 - bug 不应依赖**未声明的假设**
   - 不应假设代码库的某些未文档化的行为
   - 不应假设作者的意图
 - 设计意图：确保 bug 是客观的，而非主观推测

 ### 准则 7：可证明的影响

 ```
 7. It is not enough to speculate that a change may disrupt another part of the codebase,
    to be considered a bug, one must identify the other parts of the code that are provably affected.
 ```

 **详解：**

 - 不能仅凭**推测**说"可能影响其他部分"
 - 必须**识别出受影响的具体代码**
 - "provably affected"：可证明受到影响
 - 设计意图：避免标记没有具体证据的推测性"问题"

 ### 准则 8：非故意改动

 ```
 8. The bug is clearly not just an intentional change by the original author.
 ```

 **详解：**

 - bug 必须**明显不是故意的**
 - 如果是作者有意的改动，不应标记为 bug
 - 设计意图：尊重作者的设计选择

 ---

 ## 11.3 评论撰写 8 条准则

 ### 评论准则 1：说明原因

 ```
 1. The comment should be clear about why the issue is a bug.
 ```

 - 明确说明**为什么**是 bug

 ### 评论准则 2：恰当的严重性

 ```
 2. The comment should appropriately communicate the severity of the issue.
    It should not claim that an issue is more severe than it actually is.
 ```

 - 恰当传达严重性
 - **不要夸大**严重性

 ### 评论准则 3：简洁

 ```
 3. The comment should be brief. The body should be at most 1 paragraph.
    It should not introduce line breaks within the natural language flow unless it is necessary for the code fragment.
 ```

 - **简洁**：body 最多 1 段
 - 不引入不必要的换行

 ### 评论准则 4：代码块限制

 ```
 4. The comment should not include any chunks of code longer than 3 lines.
    Any code chunks should be wrapped in markdown inline code tags or a code block.
 ```

 - 代码块**不超过 3 行**
 - 用 markdown inline code 或 code block

 ### 评论准则 5：明确触发条件

 ```
 5. The comment should clearly and explicitly communicate the scenarios, environments, or inputs
    that are necessary for the bug to arise.
 ```

 - 明确说明**触发 bug 的场景/环境/输入**
 - 让读者知道 bug 何时会出现

 ### 评论准则 6：语气

 ```
 6. The comment's tone should be matter-of-fact and not accusatory or overly positive.
    It should read as a helpful AI assistant suggestion without sounding too much like a human reviewer.
 ```

 - **客观事实**的语气
 - 不指责，也不过度正面
 - 读起来像 AI 助手建议，而非人类审查者

 ### 评论准则 7：立即可理解

 ```
 7. The comment should be written such that the original author can immediately grasp the idea without close reading.
 ```

 - 让作者**立即抓住要点**
 - 不需要细读

 ### 评论准则 8：避免奉承

 ```
 8. The comment should avoid excessive flattery and comments that are not helpful to the original author.
    The comment should avoid phrasing like "Great job ...", "Thanks for ...".
 ```

 - **避免奉承**（如 "Great job..."）
 - **避免无帮助的评论**

 ---

 ## 11.4 详细指南

 ### 数量指南

 ```
 HOW MANY FINDINGS TO RETURN:
 Output all findings that the original author would fix if they knew about it.
 If there is no finding that a person would definitely love to see and fix, prefer outputting no findings.
 Do not stop at the first qualifying finding. Continue until you've listed every qualifying finding.
 ```

 **详解：**

 - 返回**所有**作者会修复的 findings
 - 如果没有"作者一定会想看到并修复"的 finding，**宁可返回空**
 - **不要停在第一个** qualifying finding
 - 设计意图：宁缺毋滥，但要全面

 ### 格式指南

 ```
 GUIDELINES:
 - Ignore trivial style unless it obscures meaning or violates documented standards.
 - Use one comment per distinct issue (or a multi-line range if necessary).
 - Use ```suggestion blocks ONLY for concrete replacement code (minimal lines; no commentary inside the block).
 - In every ```suggestion block, preserve the exact leading whitespace of the replaced lines.
 - Do NOT introduce or remove outer indentation levels unless that is the actual fix.
 ```

 **详解：**

 - 忽略琐碎风格（除非掩盖含义或违反文档化标准）
 - 每个 distinct issue 一条评论
 - suggestion block 只用于具体替换代码
   - 最小行数
   - 块内无评论
 - 保留精确的前导空格
 - 不引入/移除外层缩进（除非是实际修复）

 ### 行范围指南

 ```
 The comments will be presented in the code review as inline comments.
 You should avoid providing unnecessary location details in the comment body.
 Always keep the line range as short as possible for interpreting the issue.
 Avoid ranges longer than 5–10 lines; instead, choose the most suitable subrange that pinpoints the problem.
 ```

 **详解：**

 - 评论作为 inline comments 呈现
 - 避免在 body 中提供不必要的位置细节（位置在 `code_location` 中）
 - 行范围**尽可能短**
 - 避免**超过 5-10 行**的范围，选择最精确的子范围

 ---

 ## 11.5 优先级系统

 ```
 At the beginning of the finding title, tag the bug with priority level.
 For example "[P1] Un-padding slices along wrong tensor dimensions".

 [P0] – Drop everything to fix. Blocking release, operations, or major usage.
        Only use for universal issues that do not depend on any assumptions about the inputs.
 [P1] – Urgent. Should be addressed in the next cycle
 [P2] – Normal. To be fixed eventually
 [P3] – Low. Nice to have
 ```

 **详解：**

 | 优先级 | 标记 | 含义 | JSON priority |
 |--------|------|------|--------------|
 | P0 | `[P0]` | Drop everything to fix. Blocking release | 0 |
 | P1 | `[P1]` | Urgent. Next cycle | 1 |
 | P2 | `[P2]` | Normal. Eventually | 2 |
 | P3 | `[P3]` | Low. Nice to have | 3 |

 **P0 的特殊要求：**

 - 只用于**不依赖任何输入假设的普遍问题**
 - Blocking release/operations/major usage

 **JSON priority 字段：**

 ```
 Additionally, include a numeric priority field in the JSON output for each finding:
 set "priority" to 0 for P0, 1 for P1, 2 for P2, or 3 for P3.
 If a priority cannot be determined, omit the field or use null.
 ```

 - JSON 中 `priority` 字段对应 0-3
 - 无法确定时可以省略或用 null
 - 但 `ReviewFinding.priority` 是 `i32`（非 Option），所以实际不能省略

 ---

 ## 11.6 输出 schema

 ```
 ## Output schema  — MUST MATCH *exactly*

 ```json
 {
   "findings": [
     {
       "title": "<≤ 80 chars, imperative>",
       "body": "<valid Markdown explaining *why* this is a problem; cite files/lines/functions>",
       "confidence_score": <float 0.0-1.0>,
       "priority": <int 0-3, optional>,
       "code_location": {
         "absolute_file_path": "<file path>",
         "line_range": {"start": <int>, "end": <int>}
       }
     }
   ],
   "overall_correctness": "patch is correct" | "patch is incorrect",
   "overall_explanation": "<1-3 sentence explanation justifying the overall_correctness verdict>",
   "overall_confidence_score": <float 0.0-1.0>
 }
 ```
 ```

 **字段详解：**

 | 字段 | 类型 | 约束 | 说明 |
 |------|------|------|------|
 | findings | array | - | 审查发现列表（可为空） |
 | findings[].title | string | ≤80 字符，imperative | 标题（含优先级标记） |
 | findings[].body | string | valid Markdown | 正文（解释为什么是问题） |
 | findings[].confidence_score | float | 0.0-1.0 | 置信度 |
 | findings[].priority | int | 0-3, optional | 优先级 |
 | findings[].code_location | object | required | 代码位置 |
 | code_location.absolute_file_path | string | - | 文件绝对路径 |
 | code_location.line_range | object | - | 行范围 |
 | line_range.start | int | - | 起始行 |
 | line_range.end | int | - | 结束行 |
 | overall_correctness | string | "patch is correct" / "patch is incorrect" | 正确性裁决 |
 | overall_explanation | string | 1-3 句 | 解释 |
 | overall_confidence_score | float | 0.0-1.0 | 整体置信度 |

 **关键约束：**

 ```
 * **Do not** wrap the JSON in markdown fences or extra prose.
 * The code_location field is required and must include absolute_file_path and line_range.
 * Line ranges must be as short as possible for interpreting the issue (avoid ranges over 5–10 lines).
 * The code_location should overlap with the diff.
 * Do not generate a PR fix.
 ```

 - **不要**用 markdown fence 包裹 JSON
 - `code_location` **必填**
 - 行范围**尽可能短**
 - `code_location` 应与 **diff 重叠**
 - **不要**生成 PR fix

 ---

 > **一句话回顾**：`review_prompt.md` 详解揭示了审查准则的完整体系——8 条 bug 判定准则（实质性影响/离散可操作/严谨度匹配/本次引入/作者会修复/不依赖假设/可证明影响/非故意）+ 8 条评论准则（说明原因/恰当严重性/简洁/代码块限制/明确触发/语气/立即可理解/避免奉承）+ 优先级 P0-P3 + 严格 JSON 输出 schema。

 ---

 # 第十二部分 · 完整索引与交叉引用

 本部分提供文档的完整索引，方便快速定位。

 ---

 ## 12.1 按主题索引

 ### 命令解析
 - 斜杠命令解析：§2.1, §3.1.3阶段1, §6.1.3
 - SlashCommand::Review：§1.4, §2.1.2
 - supports_inline_args：§2.1.3, §2.1.5
 - /autoreview：§2.1.7, §3.5

 ### 协议类型
 - ReviewRequest：§1.4, §2.2.2, §5.1
 - ReviewTarget：§1.4, §2.2.2, §5.1
 - ReviewOutputEvent：§1.4, §2.2.2, §5.1
 - ReviewFinding：§1.4, §2.2.2, §5.1
 - ReviewCodeLocation：§1.4, §2.2.2, §5.1
 - ReviewLineRange：§1.4, §2.2.2, §5.1
 - ExitedReviewModeEvent：§1.4, §2.2.2, §5.1
 - SubAgentSource::Review：§1.4, §2.2.2, §5.1
 - ReviewDecision：§2.2.2, §3.6.4, §5.1
 - ApprovalsReviewer：§2.2.2, §2.9, §5.1

 ### 核心函数
 - review()：§2.3.3, §2.3.4, §6.1.3
 - spawn_review_thread()：§2.3.4, §6.2, §10
 - resolve_review_request()：§2.5.3, §2.5.4, §6.2.2
 - review_prompt()：§2.5.4, §6.2.3
 - ReviewTask::run()：§2.4.4, §6.1.3
 - start_review_conversation()：§2.4.4, §6.1.4
 - process_review_events()：§2.4.4, §6.1.5
 - parse_review_output_event()：§2.4.4, §6.1.6
 - exit_review_mode()：§2.4.4, §6.1.7
 - format_review_findings_block()：§2.6.4, §6.3.2
 - render_review_output_text()：§2.6.4, §6.3.3

 ### 生命周期场景
 - 正常 /review：§3.1
 - /review <branch>：§3.2
 - /review <commit>：§3.3
 - 中断/取消：§3.4
 - /autoreview：§3.5
 - Guardian 审查：§3.6

 ### 横切关注点
 - 配置体系：§4.1
 - 子代理隔离：§4.2
 - 事件抑制：§4.3
 - 遥测：§4.4
 - 错误处理：§4.5
 - 持久化：§4.6
 - 测试：§4.7, §8

 ### 图表
 - 全局架构图：§1.2
 - 主状态机：§7.1.1
 - TUI 状态机：§7.1.3
 - 数据流变换图：§7.2.1
 - 事件过滤管道：§7.3.1
 - 时序图（场景1-6）：§3.1.2, §3.2.2, §3.3.2, §3.4.2, §3.5.2, §3.6.2

 ### 逐行注解
 - tasks/review.rs：§6.1
 - review_prompts.rs：§6.2
 - review_format.rs：§6.3
 - session/review.rs：§10
 - review_prompt.md：§11

 ### 调试
 - 常见问题排查：§9.1
 - 日志与追踪：§9.2
 - 性能分析：§9.3

 ---

 ## 12.2 按文件索引

 | 文件 | 文档章节 | 说明 |
 |------|---------|------|
 | tui/src/slash_command.rs | §2.1 | 斜杠命令解析 |
 | protocol/src/protocol.rs | §2.2 | 协议类型定义 |
 | core/src/session/handlers.rs | §2.3 | 审查入口 |
 | core/src/session/review.rs | §2.3, §10 | spawn 审查线程 |
 | core/src/tasks/review.rs | §2.4, §6.1 | 审查任务执行 |
 | core/review_prompt.md | §2.5, §11 | 审查者系统 prompt |
 | core/src/review_prompts.rs | §2.5, §6.2 | prompt 生成 |
 | core/src/client_common.rs | §2.5 | REVIEW_PROMPT 常量 |
 | core/templates/review/exit_success.xml | §2.5 | 成功模板 |
 | core/templates/review/exit_interrupted.xml | §2.5 | 中断模板 |
 | core/src/review_format.rs | §2.6, §6.3 | 格式化 |
 | tui/src/chatwidget.rs | §2.7 | TUI 审查模式 |
 | tui/src/auto_review_denials.rs | §2.7, §3.5 | AutoReview 拒绝记录 |
 | app-server-protocol/src/protocol/v2.rs | §2.8 | v2 协议 |
 | config/src/config_toml.rs | §2.9 | 配置定义 |
 | config/src/config_requirements.rs | §2.9, §4.1 | 配置约束 |
 | core/tests/suite/review.rs | §4.7, §8 | 集成测试 |
 | tui/src/chatwidget/tests/review_mode.rs | §4.7 | TUI 测试 |
 | core/src/guardian/review.rs | §3.6 | Guardian 审查 |
 | core/src/guardian/review_session.rs | §3.6 | Guardian 审查会话 |

 ---

 ## 12.3 文档统计（最终）

 | 统计项 | 数量 |
 |--------|------|
 | 总部分数 | 12 |
 | 总章节数 | 60+ |
 | 架构图（Mermaid + ASCII） | 15+ |
 | 时序图 | 6 |
 | 状态机图 | 3 |
 | 数据流图 | 3 |
 | 伪代码块 | 40+ |
 | 数据样例（JSON/文本） | 30+ |
 | 代码引用（file:line） | 150+ |
 | 逐行注解函数 | 8 |
 | 测试走读 | 9 |
 | FAQ | 15 |
 | 排障场景 | 4 |

 ---

 ---

 # 第十三部分 · TUI 审查模式代码注解

 本部分对 `tui/src/chatwidget.rs` 中审查相关的关键函数进行逐行注解。

 ---

 ## 13.1 ChatWidget 审查相关字段

 ```rust
 // tui/src/chatwidget.rs:1022-1025
 pub struct ChatWidget {
     // ...
     /// Simple review mode flag; used to adjust layout and banners.
     is_review_mode: bool,

     /// Snapshot of token usage to restore after review mode exits.
     pre_review_token_info: Option<Option<TokenUsageInfo>>,
     // ...
 }
 ```

 **注解：**

 - `is_review_mode: bool`：审查模式标志
   - `true`：当前在审查模式中
   - 用于调整布局、banner、抑制用户消息渲染
   - 初始值 `false`（L5791）

 - `pre_review_token_info: Option<Option<TokenUsageInfo>>`：审查前 token 快照
   - 外层 `Option`：是否在审查模式中（`Some` = 在审查模式，`None` = 不在）
   - 内层 `Option`：审查前的 token info（可能为 `None` 如果审查前没有 token info）
   - 初始值 `None`（L5792）
   - 用途：审查子代理的 token 消耗不应混入主会话统计

 ---

 ## 13.2 enter_review_mode_with_hint() 注解

 ```rust
 // tui/src/chatwidget.rs:8105-8117
 fn enter_review_mode_with_hint(&mut self, hint: String, from_replay: bool) {
     if self.pre_review_token_info.is_none() {
         self.pre_review_token_info = Some(self.token_info.clone());
     }
 ```

 **注解：**

 - `if self.pre_review_token_info.is_none()`：仅在首次进入时保存
   - 防止嵌套审查时覆盖原始快照
   - `Some(self.token_info.clone())`：保存当前 token info
   - 内层是 `self.token_info.clone()`（可能是 `Some` 或 `None`）

 ```rust
     if !from_replay && !self.bottom_pane.is_task_running() {
         self.bottom_pane.set_task_running(/*running*/ true);
     }
 ```

 **注解：**

 - `!from_replay`：非 replay 时才设 task running
   - replay 是历史回放，不是实时执行
 - `!self.bottom_pane.is_task_running()`：如果还没在 running 状态才设
   - 避免重复设置
 - `set_task_running(true)`：标记底部面板为"任务运行中"

 ```rust
     self.is_review_mode = true;
     let banner = format!(">> Code review started: {hint} <<");
     self.add_to_history(history_cell::new_review_status_line(banner));
     self.request_redraw();
 }
 ```

 **注解：**

 - `is_review_mode = true`：设置审查模式标志
 - `banner = ">> Code review started: {hint} <<"`：格式化 banner
   - `hint` 来自 `ResolvedReviewRequest.user_facing_hint`
   - 例如：">> Code review started: current changes <<"
 - `add_to_history(new_review_status_line(banner))`：添加到历史
   - `new_review_status_line`：创建审查状态行 cell
 - `request_redraw()`：请求 UI 重绘

 ---

 ## 13.3 exit_review_mode_after_item() 注解

 ```rust
 // tui/src/chatwidget.rs:8118-8127
 fn exit_review_mode_after_item(&mut self) {
     self.flush_answer_stream_with_separator();
     self.flush_interrupt_queue();
     self.flush_active_cell();
 ```

 **注解：**

 - flush 三件套：
   1. `flush_answer_stream_with_separator()`：刷新答案流（带分隔符）
      - 把缓冲的流式输出刷新到 UI
   2. `flush_interrupt_queue()`：刷新中断队列
      - 处理待处理的中断事件
   3. `flush_active_cell()`：刷新活跃 cell
      - 把当前正在构建的 cell 刷新到历史

 ```rust
     self.is_review_mode = false;
     self.restore_pre_review_token_info();
     self.add_to_history(history_cell::new_review_status_line(
         "<< Code review finished >>".to_string(),
     ));
     self.request_redraw();
 }
 ```

 **注解：**

 - `is_review_mode = false`：退出审查模式
 - `restore_pre_review_token_info()`：恢复审查前的 token info
 - `add_to_history(new_review_status_line("<< Code review finished >>"))`：添加完成 banner
 - `request_redraw()`：请求重绘

 ---

 ## 13.4 on_entered_review_mode() 注解

 ```rust
 // tui/src/chatwidget.rs:8131-8135
 #[cfg(test)]
 fn on_entered_review_mode(&mut self, review: ReviewRequest, from_replay: bool) {
     let hint = review.user_facing_hint.unwrap_or_else(|| {
         crate::legacy_core::review_prompts::user_facing_hint(&review.target)
     });
     self.enter_review_mode_with_hint(hint, from_replay);
 }
 ```

 **注解：**

 - `#[cfg(test)]`：仅在测试中直接调用（生产代码通过事件分发）
 - `review.user_facing_hint.unwrap_or_else(...)`：
   - 如果 `ReviewRequest` 有 hint，用它
   - 否则从 `review.target` 推导（`user_facing_hint(&target)`）
 - `enter_review_mode_with_hint(hint, from_replay)`：调用进入函数
   - 注意：`crate::legacy_core::review_prompts` 是 TUI 中引用 core 的路径

 ---

 ## 13.5 on_exited_review_mode() 注解

 ```rust
 // tui/src/chatwidget.rs:8139-8171
 #[cfg(test)]
 fn on_exited_review_mode(&mut self, review: ExitedReviewModeEvent) {
     if let Some(output) = review.review_output {
         let review_markdown =
             crate::legacy_core::review_format::render_review_output_text(&output);
         self.record_agent_markdown(&review_markdown);
 ```

 **注解：**

 - `#[cfg(test)]`：仅在测试中直接调用
 - `if let Some(output) = review.review_output`：有审查结果时
 - `render_review_output_text(&output)`：渲染为纯文本 markdown
 - `record_agent_markdown(&review_markdown)`：记录为 agent markdown
   - 这会把审查结果加入历史显示

 ```rust
         self.flush_answer_stream_with_separator();
         self.flush_interrupt_queue();
         self.flush_active_cell();
 ```

 **注解：**

 - flush 三件套（同 exit_review_mode_after_item）

 ```rust
         if output.findings.is_empty() {
             let explanation = output.overall_explanation.trim().to_string();
             if explanation.is_empty() {
                 tracing::error!("Reviewer failed to output a response.");
                 self.add_to_history(history_cell::new_error_event(
                     "Reviewer failed to output a response.".to_owned(),
                 ));
 ```

 **注解：**

 - `output.findings.is_empty()`：无 findings 时
 - `explanation = output.overall_explanation.trim()`：取 explanation
 - `explanation.is_empty()`：explanation 也为空时
   - `tracing::error!(...)`：记录错误日志
   - `add_to_history(new_error_event(...))`：添加错误事件到历史
   - 显示 "Reviewer failed to output a response."

 ```rust
             } else {
                 // Show explanation when there are no structured findings.
                 let mut rendered: Vec<ratatui::text::Line<'static>> = vec!["".into()];
                 crate::markdown::append_markdown(
                     &explanation,
                     /*width*/ None,
                     Some(self.config.cwd.as_path()),
                     &mut rendered,
                 );
                 let body_cell = AgentMessageCell::new(rendered, /*is_first_line*/ false);
                 self.app_event_tx
                     .send(AppEvent::InsertHistoryCell(Box::new(body_cell)));
             }
 ```

 **注解：**

 - 有 explanation 但无 findings 时：
   - `rendered = vec!["".into()]`：初始空行
   - `append_markdown(...)`：把 explanation 渲染为 ratatui Lines
     - `width = None`：不限制宽度
     - `cwd`：用于相对路径解析
   - `AgentMessageCell::new(rendered, false)`：创建 agent 消息 cell
     - `is_first_line = false`：不是第一行
   - `app_event_tx.send(InsertHistoryCell(...))`：发送插入事件

 ```rust
         }
         // Final message is rendered as part of the AgentMessage.
     }
 ```

 **注解：**

 - 有 findings 时：已在 `record_agent_markdown` 中处理
 - 注释说"Final message is rendered as part of the AgentMessage"

 ```rust
     self.exit_review_mode_after_item();
 }
 ```

 **注解：**

 - 无论是否有审查结果，都调 `exit_review_mode_after_item()`
 - 完成退出流程（flush + is_review_mode = false + 恢复 token info + banner）

 ---

 ## 13.6 restore_pre_review_token_info() 注解

 ```rust
 // tui/src/chatwidget.rs:3253-3256
 fn restore_pre_review_token_info(&mut self) {
     if let Some(saved) = self.pre_review_token_info.take() {
         self.token_info = saved
     }
 }
 ```

 **注解：**

 - `pre_review_token_info.take()`：取出保存的 token info
   - `take()` 把 `Option` 中的值取出，留下 `None`
   - 如果 `pre_review_token_info` 是 `Some(saved)`，取出 `saved`（内层 Option）
   - 如果是 `None`（不在审查模式），什么都不做
 - `self.token_info = saved`：恢复审查前的 token info
   - `saved` 是 `Option<TokenUsageInfo>`

 ---

 ## 13.7 事件分发注解

 ```rust
 // tui/src/chatwidget.rs:8013-8016
 // 在事件处理主循环中
 EventMsg::EnteredReviewMode(review_request) => {
     self.on_entered_review_mode(review_request, from_replay)
 }
 EventMsg::ExitedReviewMode(review) => self.on_exited_review_mode(review),
 ```

 **注解：**

 - `EnteredReviewMode` 事件 → `on_entered_review_mode`
   - 传入 `review_request` 和 `from_replay`
 - `ExitedReviewMode` 事件 → `on_exited_review_mode`
   - 传入 `ExitedReviewModeEvent`

 ---

 ## 13.8 replay 处理注解

 ```rust
 // tui/src/chatwidget.rs:7092-7098
 // replay 时处理历史中的审查 items
 ThreadItem::EnteredReviewMode { review, .. } => {
     self.enter_review_mode_with_hint(review, /*from_replay*/ true);
 }
 ThreadItem::ExitedReviewMode { .. } => {
     self.exit_review_mode_after_item();
 }
 ```

 **注解：**

 - replay 时从历史 items 恢复审查模式状态
 - `EnteredReviewMode { review, .. }`：
   - `review` 是 hint 字符串
   - `from_replay = true`：标记为 replay（不设 task running）
 - `ExitedReviewMode { .. }`：
   - 直接调 `exit_review_mode_after_item`
   - 注意：replay 时不调 `on_exited_review_mode`（不重新渲染结果）
     - 因为结果已在历史中

 ```rust
 // tui/src/chatwidget.rs:7634-7635
 // 非 replay 时处理
 ThreadItem::EnteredReviewMode { review, .. } if !from_replay => {
     self.enter_review_mode_with_hint(review, /*from_replay*/ false);
 }
 ```

 **注解：**

 - 非 replay 时 `from_replay = false`
 - 会设 task running

 ---

 ## 13.9 is_review_mode 的影响注解

 ### 13.9.1 用户消息渲染抑制

 ```rust
 // tui/src/chatwidget.rs:7827-7834
 // on_committed_user_message 中
 if from_replay || self.is_review_mode =>
     // 审查模式或 replay 时跳过用户消息渲染
     ...
 ```

 **注解：**

 - `is_review_mode = true` 时跳过用户消息渲染
 - 原因：审查期间记录的 user message（XML 模板）不应显示为普通用户输入

 ### 13.9.2 task running 判断

 ```rust
 // tui/src/chatwidget.rs:11787
 self.bottom_pane.is_task_running() || self.is_review_mode
 ```

 **注解：**

 - `is_review_mode` 为 true 时，即使 bottom_pane 没有标记 task running，也视为"任务运行中"
 - 影响 UI 状态显示

 ---

 > **一句话回顾**：TUI 审查模式代码注解揭示了 5 个核心函数——`enter_review_mode_with_hint`（保存 token + 设标志 + banner）、`exit_review_mode_after_item`（flush 三件套 + 恢复 token + 完成 banner）、`on_exited_review_mode`（渲染 findings/explanation/error）、`restore_pre_review_token_info`（take + 恢复），以及 `is_review_mode` 对用户消息渲染和 task running 判断的影响。

 ---

 # 第十四部分 · 完整测试代码与注解

 本部分对剩余的关键测试进行完整代码展示和注解。

 ---

 ## 14.1 `review_filters_agent_message_related_events` 完整注解

 ```rust
 // Windows CI only: bump to 4 workers
 #[cfg_attr(windows, tokio::test(flavor = "multi_thread", worker_threads = 4))]
 #[cfg_attr(not(windows), tokio::test(flavor = "multi_thread", worker_threads = 2))]
 async fn review_filters_agent_message_related_events() {
     skip_if_no_network!();
 ```

 **注解：**

 - Windows CI 用 4 个 worker threads（防止 SSE/event 饥饿和超时）
 - 非 Windows 用 2 个 worker threads

 ```rust
     // 模拟流式 assistant message
     let sse_raw = r#"[
         {"type":"response.output_item.added", "item":{
             "type":"message", "role":"assistant", "id":"msg-1",
             "content":[{"type":"output_text","text":""}]
         }},
         {"type":"response.output_text.delta", "delta":"Hi"},
         {"type":"response.output_text.delta", "delta":" there"},
         {"type":"response.output_item.done", "item":{
             "type":"message", "role":"assistant", "id":"msg-1",
             "content":[{"type":"output_text","text":"Hi there"}]
         }},
         {"type":"response.completed", "response": {"id": "__ID__"}}
     ]"#;
 ```

 **注解：**

 - SSE 事件序列：
   1. `response.output_item.added`：消息开始（空文本）
   2. `response.output_text.delta` "Hi"：流式增量
   3. `response.output_text.delta` " there"：流式增量
   4. `response.output_item.done`：消息完成（完整文本 "Hi there"）
   5. `response.completed`：响应完成
 - 这模拟了一个流式 assistant message

 ```rust
     let (server, _request_log) =
         start_responses_server_with_sse(sse_raw, /*expected_requests*/ 1).await;
     let agere_home = Arc::new(TempDir::new().unwrap());
     let agere = new_conversation_for_server(&server, agere_home.clone(), |_| {}).await;

     agere
         .submit(Op::Review {
             review_request: ReviewRequest {
                 target: ReviewTarget::Custom {
                     instructions: "Filter streaming events".to_string(),
                 },
                 user_facing_hint: None,
             },
         })
         .await
         .unwrap();
 ```

 **注解：**

 - 启动 mock 服务器，创建会话
 - 提交审查请求（Custom 指令 = "Filter streaming events"）

 ```rust
     let mut saw_entered = false;
     let mut saw_exited = false;

     // 排水直到 TurnComplete；断言流式相关事件永远不会出现
     wait_for_event(&agere, |event| match event {
         EventMsg::TurnComplete(_) => true,
         EventMsg::EnteredReviewMode(_) => {
             saw_entered = true;
             false
         }
         EventMsg::ExitedReviewMode(_) => {
             saw_exited = true;
             false
         }
         // 以下必须被审查流过滤
         EventMsg::AgentMessageContentDelta(_) => {
             panic!("unexpected AgentMessageContentDelta surfaced during review")
         }
         EventMsg::AgentMessageDelta(_) => {
             panic!("unexpected AgentMessageDelta surfaced during review")
         }
         _ => false,
     })
     .await;
 ```

 **注解：**

 - `wait_for_event`：循环接收事件直到匹配
 - `TurnComplete` → 返回 true（结束等待）
 - `EnteredReviewMode` → 设 `saw_entered = true`，继续等待
 - `ExitedReviewMode` → 设 `saw_exited = true`，继续等待
 - `AgentMessageContentDelta` → **panic!**（不应出现）
 - `AgentMessageDelta` → **panic!**（不应出现）
 - 其他 → 继续等待

 ```rust
     assert!(saw_entered && saw_exited, "missing review lifecycle events");

     let _agere_home_guard = agere_home;
     server.verify().await;
 }
 ```

 **注解：**

 - 断言看到了 `EnteredReviewMode` 和 `ExitedReviewMode`
 - `_agere_home_guard`：保持 TempDir 直到测试结束
 - `server.verify()`：验证 mock 收到了期望请求

 ---

 ## 14.2 `review_history_surfaces_in_parent_session` 完整注解

 ```rust
 async fn review_history_surfaces_in_parent_session() {
     skip_if_no_network!();

     // 响应审查请求和后续父请求
     let sse_raw = r#"[
         {"type":"response.output_item.done", "item":{
             "type":"message", "role":"assistant",
             "content":[{"type":"output_text","text":"review assistant output"}]
         }},
         {"type":"response.completed", "response": {"id": "__ID__"}}
     ]"#;
     let (server, request_log) =
         start_responses_server_with_sse(sse_raw, /*expected_requests*/ 2).await;
 ```

 **注解：**

 - SSE 返回 "review assistant output"（纯文本，非 JSON）
 - `expected_requests = 2`：期望 2 个请求（审查 + 后续父 turn）

 ```rust
     // 1) 运行审查 turn
     agere
         .submit(Op::Review {
             review_request: ReviewRequest {
                 target: ReviewTarget::Custom {
                     instructions: "Start a review".to_string(),
                 },
                 user_facing_hint: None,
             },
         })
         .await
         .unwrap();

     let _entered = wait_for_event(&agere, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
     let _closed = wait_for_event(&agere, |ev| {
         matches!(
             ev,
             EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
                 review_output: Some(_)
             })
         )
     })
     .await;
     let _complete = wait_for_event(&agere, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
 ```

 **注解：**

 - 提交审查请求
 - 等待 `EnteredReviewMode` → `ExitedReviewMode(Some)` → `TurnComplete`
   - `Some(_)`：有审查结果（纯文本兜底 → `overall_explanation = "review assistant output"`）

 ```rust
     // 2) 在父会话继续；请求 input 不应包含任何审查 items
     let followup = "back to parent".to_string();
     agere
         .submit(Op::UserInput {
             environments: None,
             items: vec![UserInput::Text {
                 text: followup.clone(),
                 text_elements: Vec::new(),
             }],
             final_output_json_schema: None,
             responsesapi_client_metadata: None,
         })
         .await
         .unwrap();
     let _complete = wait_for_event(&agere, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
 ```

 **注解：**

 - 提交常规 `Op::UserInput`（"back to parent"）
 - 等待 `TurnComplete`

 ```rust
     // 检查第二个请求（父 turn）的 input 内容
     let requests = request_log.requests();
     assert_eq!(requests.len(), 2);
     let body = requests[1].body_json();
     let input = body["input"].as_array().expect("input array");
 ```

 **注解：**

 - `request_log.requests()`：获取所有捕获的请求
 - `requests.len() == 2`：审查请求 + 父 turn 请求
 - `requests[1]`：第二个请求（父 turn）
 - `body["input"]`：请求的 input 数组

 ```rust
     // 最后一条必须是 followup
     let last = input.last().expect("at least one item in input");
     assert_eq!(last["role"].as_str().unwrap(), "user");
     let last_text = last["content"][0]["text"].as_str().unwrap();
     assert_eq!(last_text, followup);
 ```

 **注解：**

 - `input.last()`：最后一条 input
 - 断言 `role == "user"` 和 `text == "back to parent"`

 ```rust
     // 确保审查 thread 内容存在于后续 turn 中
     let contains_review_rollout_user = input.iter().any(|msg| {
         msg["content"][0]["text"]
             .as_str()
             .unwrap_or_default()
             .contains("User initiated a review task.")
     });
     let contains_review_assistant = input.iter().any(|msg| {
         msg["content"][0]["text"]
             .as_str()
             .unwrap_or_default()
             .contains("review assistant output")
     });
     assert!(contains_review_rollout_user, "review rollout user message missing");
     assert!(contains_review_assistant, "review assistant output missing");
 ```

 **注解：**

 - 遍历所有 input items，检查是否包含：
   1. `"User initiated a review task."` — 审查 user message（XML 模板）
   2. `"review assistant output"` — 审查 assistant message
 - 断言两者都存在（验证审查结果在后续 turn 中可见）

 ---

 ## 14.3 `review_uses_overridden_cwd_for_base_branch_merge_base` 完整注解

 这个测试验证 cwd 覆盖对 merge-base 计算的影响，是唯一需要真实 git 仓库的测试。

 ```rust
 async fn review_uses_overridden_cwd_for_base_branch_merge_base() {
     skip_if_no_network!();

     let sse_raw = r#"[{"type":"response.completed", "response": {"id": "__ID__"}}]"#;
     let (server, request_log) =
         start_responses_server_with_sse(sse_raw, /*expected_requests*/ 1).await;

     let initial_cwd = TempDir::new().unwrap();

     // 创建临时 git 仓库
     let repo_dir = TempDir::new().unwrap();
     let repo_path = repo_dir.path();
 ```

 **注解：**

 - `initial_cwd`：初始 cwd（非 git 仓库）
 - `repo_dir`：临时 git 仓库目录

 ```rust
     fn run_git(repo_path: &std::path::Path, args: &[&str]) {
         let output = std::process::Command::new("git")
             .arg("-C")
             .arg(repo_path)
             .args(args)
             .output()
             .expect("spawn git");
         assert!(
             output.status.success(),
             "git {:?} failed: stdout={:?} stderr={:?}",
             args,
             String::from_utf8_lossy(&output.stdout),
             String::from_utf8_lossy(&output.stderr)
         );
     }

     // 初始化 git 仓库
     run_git(repo_path, &["init", "-b", "main"]);
     run_git(repo_path, &["config", "user.email", "test@example.com"]);
     run_git(repo_path, &["config", "user.name", "Test User"]);
     std::fs::write(repo_path.join("file.txt"), "hello\n").unwrap();
     run_git(repo_path, &["add", "."]);
     run_git(repo_path, &["commit", "-m", "initial"]);
 ```

 **注解：**

 - `run_git` 辅助函数：执行 git 命令并断言成功
 - 初始化 git 仓库：
   1. `git init -b main`：创建仓库，默认分支 main
   2. `git config`：设置用户信息
   3. 写入 `file.txt`
   4. `git add .` + `git commit -m "initial"`：初始 commit

 ```rust
     // 获取 HEAD SHA
     let head_sha = std::process::Command::new("git")
         .arg("-C")
         .arg(repo_path)
         .args(["rev-parse", "HEAD"])
         .output()
         .expect("rev-parse HEAD");
     let head_sha = String::from_utf8(head_sha.stdout)
         .expect("utf8 sha")
         .trim()
         .to_string();
 ```

 **注解：**

 - `git rev-parse HEAD`：获取 HEAD commit SHA
 - 用于后续断言审查 prompt 包含这个 SHA

 ```rust
     // 创建会话，初始 cwd 为 initial_cwd（非 git 仓库）
     let agere = new_conversation_for_server(&server, agere_home.clone(), move |config| {
         config.cwd = initial_cwd_path.abs();
     })
     .await;
 ```

 **注解：**

 - 初始 cwd 设为 `initial_cwd`（非 git 仓库）

 ```rust
     // 覆盖 cwd 为 repo_path（git 仓库）
     agere
         .submit(Op::OverrideTurnContext {
             cwd: Some(repo_path.to_path_buf()),
             approval_policy: None,
             approvals_reviewer: None,
             permission_profile: None,
             windows_execution_restriction_level: None,
             model: None,
             effort: None,
             summary: None,
             service_tier: None,
             collaboration_mode: None,
             personality: None,
         })
         .await
         .unwrap();
 ```

 **注解：**

 - `Op::OverrideTurnContext`：覆盖 turn 上下文
   - `cwd: Some(repo_path)`：覆盖为 git 仓库路径
   - 其他字段为 `None`（不覆盖）

 ```rust
     // 提交审查请求
     agere
         .submit(Op::Review {
             review_request: ReviewRequest {
                 target: ReviewTarget::BaseBranch {
                     branch: "main".to_string(),
                 },
                 user_facing_hint: None,
             },
         })
         .await
         .unwrap();
 ```

 **注解：**

 - 提交 `BaseBranch { branch: "main" }` 审查请求
 - `resolve_review_request` 会使用覆盖后的 cwd 计算 merge-base

 ```rust
     let _entered = wait_for_event(&agere, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
     let _complete = wait_for_event(&agere, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

     // 检查请求
     let requests = request_log.requests();
     assert_eq!(requests.len(), 1);
     let body = requests[0].body_json();
     let input = body["input"].as_array().expect("input array");

     // 验证 prompt 包含 merge-base SHA
     let saw_merge_base_sha = input
         .iter()
         .filter_map(|msg| msg["content"][0]["text"].as_str())
         .any(|text| text.contains(&head_sha));
     assert!(
         saw_merge_base_sha,
         "expected review prompt to include merge-base sha {head_sha}"
     );
 ```

 **注解：**

 - 检查审查请求的 input
 - 遍历所有 input items 的 text 内容
 - 断言某个 text 包含 `head_sha`（merge-base SHA）
 - 验证 `resolve_review_request` 使用了覆盖后的 cwd 计算 merge-base

 ---

 > **一句话回顾**：完整测试注解展示了 3 个关键测试的完整代码——`review_filters_agent_message_related_events`（验证 Delta 事件不出现，panic if surfaced）、`review_history_surfaces_in_parent_session`（验证审查结果在后续 turn 的 input 中可见）、`review_uses_overridden_cwd_for_base_branch_merge_base`（创建真实 git 仓库，验证 cwd 覆盖影响 merge-base 计算）。

 ---

 # 第十五部分 · 速查手册

 本部分是浓缩的速查手册，方便快速回忆关键信息。

 ---

 ## 15.1 一页纸总览

 ```
 /review 流程：
   用户输入 → SlashCommand::Review → Op::Review
   → handlers::review() → resolve_review_request() → spawn_review_thread()
   → 构建 TurnContext (隔离) → spawn ReviewTask → emit EnteredReviewMode
   → ReviewTask::run() → start_review_conversation() → run_agere_thread_one_shot()
   → 子代理执行 (REVIEW_PROMPT + 审查 prompt)
   → process_review_events() (抑制 Delta, 暂存 AgentMessage)
   → TurnComplete → parse_review_output_event() (三级降级)
   → exit_review_mode() (record + emit + persist)
   → TUI on_exited_review_mode() (渲染 + 恢复)
 ```

 ## 15.2 关键文件一行速查

 ```
 tui/src/slash_command.rs          → SlashCommand::Review 解析
 protocol/src/protocol.rs          → 审查类型定义 (ReviewRequest/Output/Event)
 core/src/session/handlers.rs      → review() 入口 + Op::Review 分发
 core/src/session/review.rs        → spawn_review_thread() 隔离上下文构建
 core/src/tasks/review.rs          → ReviewTask 执行体
 core/src/review_prompts.rs        → resolve_review_request() + prompt 模板
 core/review_prompt.md             → 审查者系统 prompt
 core/src/review_format.rs         → findings 格式化
 core/src/client_common.rs         → REVIEW_PROMPT 常量
 core/templates/review/*.xml       → 退出模板
 tui/src/chatwidget.rs             → TUI 审查模式
 tui/src/auto_review_denials.rs    → AutoReview 拒绝记录
 config/src/config_toml.rs         → review_model 配置
 ```

 ## 15.3 事件速查

 ```
 EnteredReviewMode(ReviewRequest)  → spawn_review_thread emit, TUI 进入审查模式
 ExitedReviewMode(ExitedReviewModeEvent) → exit_review_mode emit, TUI 退出+渲染
 TurnComplete { last_agent_message } → process_review_events 收到, 解析输出
 TurnAborted → process_review_events 返回 None, 走中断流程
 AgentMessage → 暂存 (不立即转发)
 AgentMessageDelta → 抑制
 AgentMessageContentDelta → 抑制
 ItemCompleted(AgentMessage) → 抑制
 ```

 ## 15.4 配置速查

 ```toml
 # config.toml
 review_model = "gpt-5.4"           # 审查模型 (可选, fallback 主模型)
 approvals_reviewer = "autoReview"  # Guardian 审查者 (User/AutoReview)
 ```

 ## 15.5 错误速查

 ```
 resolve 失败 → emit ErrorEvent, 不进入审查
 启动失败 → output=None → exit(None) → 中断模板
 非 JSON → parse 兜底 → overall_explanation
 全空 → "Reviewer failed to output a response."
 取消 → TurnAborted → abort() → exit(None) → 中断模板
 channel 关闭 → recv() Err → None → exit(None) → 中断模板
 web_search set 失败(task) → panic
 web_search set 失败(session) → warn + 保持原值
 模板渲染失败 → panic
 ```

 ## 15.6 /review vs /autoreview vs Guardian 速查

 ```
 /review     → 审查代码改动 → ReviewOutputEvent → tasks/review.rs
 /autoreview → 批准 Guardian 拒绝重试 → 从 RecentAutoReviewDenials 取出
 Guardian    → 审查操作是否安全 → ReviewDecision → guardian/review.rs
 ```

 ---

 ---

 # 第十六部分 · 设计决策与原理

 本部分解释 `/review` 流程中关键设计决策背后的原理，帮助新手理解"为什么这样设计"而不仅仅是"怎么工作"。

 ---

 ## 16.1 为什么审查用子代理而不是主会话？

 **决策：** 审查在独立的子代理中执行，而非在主会话中直接执行。

 **原因：**

 1. **上下文隔离**：主会话可能已有大量对话历史，审查不应受其影响。子代理只收到审查 prompt 和 REVIEW_PROMPT，确保审查视角独立。

 2. **权限隔离**：审查子代理的 `approval_policy = Never`，不需要审批。如果用主会话，审批策略可能与用户期望冲突。

 3. **工具隔离**：审查禁用 web search、SpawnCsv、Collab 等。如果用主会话，需要临时修改工具配置，容易出错。

 4. **模型隔离**：审查可以用不同的模型（`review_model`）。如果用主会话，切换模型会影响主对话。

 5. **token 隔离**：审查的 token 消耗不应混入主会话统计。子代理的 token 消耗通过 `pre_review_token_info` 机制隔离。

 6. **可取消性**：子代理有独立的 `CancellationToken`，用户可以中断审查而不影响主会话。

 ---

 ## 16.2 为什么抑制流式事件？

 **决策：** `process_review_events` 抑制 `AgentMessageDelta`、`AgentMessageContentDelta`、`ItemCompleted(AgentMessage)`。

 **原因：**

 1. **结构化输出优先**：审查流程有意用结构化 `ReviewOutputEvent` 替代自由文本 `AgentMessage`。流式增量属于自由文本范畴，不应展示。

 2. **避免 legacy 路径**：`ItemCompleted(AgentMessage)` 转发时会通过 `as_legacy_events()` 触发 legacy `AgentMessage`，这与结构化输出策略冲突。

 3. **用户体验**：审查过程中的中间推理（如"让我看看 git diff..."）对用户没有价值。用户关心的是最终的结构化结论。

 4. **避免不完整输出**：流式增量可能包含不完整的 JSON 片段，显示它们会让用户困惑。

 ---

 ## 16.3 为什么暂存 AgentMessage？

 **决策：** `process_review_events` 收到 `AgentMessage` 时不立即转发，而是暂存到 `prev_agent_message`。

 **原因：**

 1. **只保留最后一条**：子代理可能产生多条 `AgentMessage`（如思考过程 + 最终输出），只有最后一条（`TurnComplete.last_agent_message`）才包含完整的 JSON。

 2. **避免中间消息干扰**：中间消息可能是不完整的思考或部分输出，转发它们会让 UI 显示不完整内容。

 3. **简化解析**：从 `TurnComplete.last_agent_message` 解析比从多条 `AgentMessage` 中拼接更可靠。

 4. **时序保证**：`TurnComplete` 是子代理完成的信号，此时 `last_agent_message` 一定是完整的最终输出。

 ---

 ## 16.4 为什么用三级降级解析？

 **决策：** `parse_review_output_event` 有三级降级策略（整体 JSON → 子串 JSON → 纯文本兜底）。

 **原因：**

 1. **模型不总是按 schema 输出**：即使 REVIEW_PROMPT 明确要求输出 JSON，模型可能：
    - 在 JSON 前后加解释文字（如"Here's my review: {...}"）
    - 输出纯文本而非 JSON
    - 输出不完整的 JSON

 2. **整体 JSON 解析（策略 1）**：处理最理想情况——模型直接输出完整 JSON。

 3. **子串 JSON 解析（策略 2）**：处理"JSON 嵌在文本中"的情况——提取第一个 `{` 到最后一个 `}` 的子串。这能处理大多数"前后有文字"的情况。

 4. **纯文本兜底（策略 3）**：处理"完全不是 JSON"的情况——把纯文本放进 `overall_explanation`。这确保审查总有输出，不会因为解析失败而崩溃。

 5. **设计哲学**：优雅降级（graceful degradation）。宁可给出不完美的结果（纯文本而非结构化 findings），也不要让审查失败。

 ---

 ## 16.5 为什么用固定 ID 记录消息？

 **决策：** `exit_review_mode` 用固定 ID `"review_rollout_user"` 和 `"review_rollout_assistant"` 记录消息。

 **原因：**

 1. **可识别性**：固定 ID 让后续逻辑能识别审查记录（如 replay 时识别审查 items）。

 2. **幂等性**：如果 `exit_review_mode` 被多次调用（如竞态条件），固定 ID 可以用于去重。

 3. **调试便利**：在 rollout 文件中搜索固定 ID 可以快速定位审查记录。

 4. **与动态 ID 的区别**：普通消息用动态生成的 ID（如 UUID），审查消息用固定 ID 因为它们是系统生成的、有明确语义的。

 ---

 ## 16.6 为什么延迟持久化？

 **决策：** `ensure_rollout_materialized` 在 emit + record 之后执行。

 **原因：**

 1. **用户体验优先**：注释说"Do this after emitting review output so file creation + git metadata collection cannot delay client-facing items"。先让用户看到结果（emit），再做磁盘 I/O。

 2. **磁盘 I/O 可能较慢**：`ensure_rollout_materialized` 涉及文件创建和 git 元数据收集，可能耗时。延迟到 emit 后避免阻塞用户看到结果。

 3. **审查可在常规 turn 前运行**：如果用户第一次操作就是 `/review`，rollout 文件可能尚未创建。`ensure_rollout_materialized` 确保文件被创建。

 ---

 ## 16.7 为什么 web_search_mode.set 行为不同？

 **决策：** `web_search_mode.set(Disabled)` 在 task 层 panic，在 session 层 warn + 保持原值。

 **原因：**

 1. **task 层（panic）**：
    - `sub_agent_config` 是从 `ctx.config` clone 的
    - 设计假设 `Constrained<WebSearchMode>` 总是允许 `Disabled`
    - 如果不允许，这是配置系统的 bug，应该 panic 暴露
    - 注释说"by construction Constrained<WebSearchMode> must always support Disabled"

 2. **session 层（warn）**：
    - `per_turn_config` 可能受 `ConfigRequirements` 约束
    - 某些部署可能强制要求 web search（虽然不太可能）
    - warn + 保持原值更安全，不会因配置约束导致审查失败
    - 注释说"review web_search_mode is disallowed by requirements; keeping constrained value"

 3. **设计哲学**：task 层是"最后防线"——如果到这里还不允许 Disabled，说明有系统性问题；session 层是"配置层"——应该尊重配置约束。

 ---

 ## 16.8 为什么审查不携带 developer/user 指令？

 **决策：** 审查子代理的 `developer_instructions = None` 和 `user_instructions = None`。

 **原因：**

 1. **审查视角独立**：AGENTS.md 中的 developer 指令和用户配置的 user 指令是为主会话设计的。审查子代理不应受其影响。

 2. **避免偏见**：如果审查子代理携带主会话的 developer 指令（如"always use method references"），审查时可能对不符合该指令的代码过度批评。

 3. **可复现性**：相同代码改动的审查结果不应因主会话配置不同而不同。不携带指令确保审查行为一致。

 4. **替代方案**：审查用 `base_instructions = REVIEW_PROMPT` 替代主会话的系统指令，确保审查有专门的、一致的审查准则。

 ---

 ## 16.9 为什么用 XML 模板包裹审查结果？

 **决策：** 审查结果用 `<user_action>` XML 模板包裹后记录为 user message。

 **原因：**

 1. **语义标记**：XML 标签让后续 turn 的 Agent 理解这是审查结果而非普通用户输入。`<context>` 说明背景，`<action>review</action>` 标记动作，`<results>` 包裹结果。

 2. **可选择性**：注释说"User may select one or more comments to resolve"。XML 格式让 Agent 能解析出每条 finding，支持后续的选择性处理。

 3. **与普通消息区分**：普通用户消息是自由文本，审查结果是结构化的 XML。这让 Agent 在后续对话中能区分"用户说的"和"审查结果"。

 4. **context 注入**：成功模板的 `<context>` 说明了审查结果的来源（"reviewer model"）和用途（"User may select one or more comments to resolve"），帮助 Agent 正确理解。

 ---

 ## 16.10 为什么有两个 feature 裁剪点？

 **决策：** feature 裁剪在 `spawn_review_thread`（session 层）和 `start_review_conversation`（task 层）各做一次，但禁用的 feature 不同。

 **原因：**

 1. **不同关注点**：
    - session 层裁剪影响 `ToolsConfig` 构建（决定哪些工具可用）
    - task 层裁剪影响子代理执行时的 feature 检查

 2. **不同 feature**：
    - session 层：禁 `WebSearchRequest`、`WebSearchCached`（影响工具配置）
    - task 层：禁 `SpawnCsv`、`Collab`（影响子代理行为）

 3. **冗余但必要**：web search 在两处都禁（`features.disable` + `web_search_mode.set(Disabled)`），确保即使一处遗漏另一处也能拦截。

 4. **历史原因**：session 层的 `spawn_review_thread` 和 task 层的 `start_review_conversation` 是不同时期的代码，各自独立做了裁剪。

 ---

 ## 16.11 为什么 TurnContext.features 用父的？

 **决策：** `review_turn_context.features = parent_turn_context.features.clone()`，而非裁剪后的 `review_features`。

 **原因：**

 1. **不同用途**：
    - `TurnContext.features`：用于 turn 执行期间的 feature 检查
    - `per_turn_config.features`：用于子代理配置（`start_review_conversation` 中再次裁剪）
    - `review_features`：仅用于 `ToolsConfig` 构建

 2. **task 层再裁剪**：`start_review_conversation` 会 clone `sub_agent_config.features` 并再次裁剪（禁 SpawnCsv、Collab）。所以 `TurnContext.features` 用父的不影响最终行为。

 3. **潜在问题**：如果 `TurnContext.features` 被其他逻辑使用（非 task 层），可能未禁用 web search。但当前代码中审查的 feature 检查主要在 task 层。

 4. **设计意图**：`TurnContext` 的 `features` 字段更多是"继承"而非"配置"，实际裁剪在 `per_turn_config` 和 `start_review_conversation` 中完成。

 ---

 ## 16.12 为什么不用 final_output_json_schema？

 **决策：** `run_agere_thread_one_shot` 的 `final_output_json_schema` 参数为 `None`。

 **原因：**

 1. **靠 prompt 约束**：REVIEW_PROMPT 中明确定义了输出 schema，模型被要求按 schema 输出。

 2. **靠解析兜底**：`parse_review_output_event` 的三级降级确保即使模型不完全按 schema 输出，也能得到结果。

 3. **schema 约束的局限**：如果用 `final_output_json_schema`，模型可能更严格地按 schema 输出，但也可能在无法满足 schema 时返回空或错误。兜底策略更灵活。

 4. **兼容性**：不是所有模型都支持 JSON schema 约束输出。靠 prompt + 解析兜底兼容性更好。

 ---

 > **一句话回顾**：设计决策揭示了 12 个关键选择的原理——子代理隔离（6 重隔离）、事件抑制（结构化输出优先）、暂存 AgentMessage（只留最后一条）、三级降级解析（优雅降级）、固定 ID（可识别+幂等）、延迟持久化（用户体验优先）、web_search 双行为（task panic/session warn）、不携带指令（审查独立）、XML 模板（语义标记）、双 feature 裁剪点（不同关注点）、features 用父的（task 层再裁剪）、不用 schema（靠 prompt+兜底）。

 ---

 # 第十七部分 · 常见模式与惯用法

 本部分总结 `/review` 代码中的常见模式和 Rust 惯用法，帮助新手举一反三。

 ---

 ## 17.1 LazyLock 延迟初始化模式

 ```rust
 static REVIEW_EXIT_SUCCESS_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
     let normalized = normalize_review_template_line_endings(
         crate::client_common::REVIEW_EXIT_SUCCESS_TMPL
     );
     Template::parse(normalized.as_ref())
         .unwrap_or_else(|err| panic!("review exit success template must parse: {err}"))
 });
 ```

 **模式：**

 - `LazyLock<T>`：全局延迟初始化
 - 首次访问时执行闭包
 - 线程安全（无需 `Mutex`）
 - 适合编译时常量的运行时解析（如模板解析）

 **审查代码中的使用：**

 - `REVIEW_EXIT_SUCCESS_TEMPLATE` — 退出成功模板
 - `BASE_BRANCH_PROMPT_TEMPLATE` — base branch prompt 模板
 - `BASE_BRANCH_PROMPT_BACKUP_TEMPLATE` — backup 模板
 - `COMMIT_PROMPT_TEMPLATE` — commit prompt 模板
 - `COMMIT_PROMPT_WITH_TITLE_TEMPLATE` — commit with title 模板

 ---

 ## 17.2 Constrained 约束模式

 ```rust
 // 设置约束值
 if let Err(err) = sub_agent_config.web_search_mode.set(WebSearchMode::Disabled) {
     panic!("...must always support Disabled: {err}");
 }

 // 创建只允许特定值的约束
 sub_agent_config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
 ```

 **模式：**

 - `Constrained<T>`：带约束的包装器
 - `set(value)`：设置值（如果允许）
 - `allow_only(value)`：创建只允许一个值的约束
 - `value()`：获取当前值

 **两种错误处理策略：**

 1. `panic!`：用于"设计上必须成功"的场景（如 task 层的 web search）
 2. `warn! + 保持原值`：用于"可能被配置约束"的场景（如 session 层的 web search）

 ---

 ## 17.3 事件匹配与抑制模式

 ```rust
 match event.clone().msg {
     EventMsg::AgentMessage(_) => {
         // 暂存逻辑
     }
     // 抑制多种事件（或模式）
     EventMsg::ItemCompleted(ItemCompletedEvent {
         item: TurnItem::AgentMessage(_),
         ..
     })
     | EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { .. })
     | EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent { .. }) => {}
     // 终止事件
     EventMsg::TurnComplete(task_complete) => {
         return task_complete.last_agent_message.as_deref().map(parse_review_output_event);
     }
     EventMsg::TurnAborted(_) => return None,
     // 转发其他
     other => {
         session.clone_session().send_event(ctx.as_ref(), other).await;
     }
 }
 ```

 **模式：**

 - `|` 模式组合：多个事件类型共享同一处理（抑制）
 - `..`：忽略结构体的其他字段
 - `return`：终止事件直接返回
 - `other`：通配符转发

 ---

 ## 17.4 三级降级模式

 ```rust
 fn parse_review_output_event(text: &str) -> ReviewOutputEvent {
     // 策略 1：整体解析
     if let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(text) {
         return ev;
     }
     // 策略 2：子串解析
     if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}'))
         && start < end
         && let Some(slice) = text.get(start..=end)
         && let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(slice)
     {
         return ev;
     }
     // 策略 3：兜底
     ReviewOutputEvent {
         overall_explanation: text.to_string(),
         ..Default::default()
     }
 }
 ```

 **模式：**

 - 逐级降级：从最严格到最宽松
 - `if let Ok` 链：成功则返回，失败则继续
 - `&&` 链式 let（Rust 2024）：多个条件组合
 - `..Default::default()`：部分初始化 + 默认值

 ---

 ## 17.5 Arc + clone 共享模式

 ```rust
 let config = ctx.config.clone();        // Arc<Config> clone（廉价）
 let mut sub_agent_config = config.as_ref().clone();  // Config clone（深拷贝）

 // 传递 Arc 共享
 session.clone_session().send_event(ctx.as_ref(), other).await;

 // cancellation_token clone（共享取消状态）
 start_review_conversation(session.clone(), ctx.clone(), input, cancellation_token.clone())
 ```

 **模式：**

 - `Arc<T>::clone()`：引用计数+1（廉价）
 - `Arc<T>::as_ref().clone()`：先解引用再深拷贝
 - `CancellationToken::clone()`：共享取消状态
 - `session.clone_session()`：clone session 引用

 ---

 ## 17.6 include_str! 编译时包含模式

 ```rust
 pub const REVIEW_PROMPT: &str = include_str!("../review_prompt.md");
 pub const REVIEW_EXIT_SUCCESS_TMPL: &str = include_str!("../templates/review/exit_success.xml");
 pub const REVIEW_EXIT_INTERRUPTED_TMPL: &str = include_str!("../templates/review/exit_interrupted.xml");
 ```

 **模式：**

 - `include_str!(path)`：编译时把文件内容包含为 `&'static str`
 - 路径相对于当前源文件
 - 零运行时成本（编译时嵌入）
 - 修改文件后需重新编译

 ---

 ## 17.7 Cow 借用/拥有模式

 ```rust
 fn normalize_review_template_line_endings(template: &str) -> Cow<'_, str> {
     if template.contains('\r') {
         Cow::Owned(template.replace("\r\n", "\n").replace('\r', "\n"))
     } else {
         Cow::Borrowed(template)
     }
 }
 ```

 **模式：**

 - `Cow<'a, str>`：可能借用（`&'a str`）也可能拥有（`String`）
 - 无需修改时：`Cow::Borrowed`（零成本）
 - 需要修改时：`Cow::Owned`（分配新 String）
 - 避免不必要的内存分配

 ---

 ## 17.8 Template 渲染模式

 ```rust
 fn render_review_prompt<'a, const N: usize>(
     template: &Template,
     variables: [(&'a str, &'a str); N],
 ) -> String {
     template
         .render(variables)
         .unwrap_or_else(|err| panic!("review prompt template must render: {err}"))
 }

 // 使用
 render_review_prompt(&BASE_BRANCH_PROMPT_TEMPLATE, [
     ("base_branch", branch.as_str()),
     ("merge_base_sha", commit.as_str()),
 ])
 ```

 **模式：**

 - `const N: usize`：编译时已知的数组大小
 - `[(&str, &str); N]`：变量数组（键值对）
 - `{{var}}` 语法：模板中的双花括号变量
 - `unwrap_or_else(panic!)`：编译时常量不应出错

 ---

 ## 17.9 async_channel 事件流模式

 ```rust
 // 生产者端（子代理）
 run_agere_thread_one_shot(...) -> Ok(io)  // io.rx_event 是 Receiver

 // 消费者端（process_review_events）
 while let Ok(event) = receiver.recv().await {
     match event.msg {
         // ...
     }
 }
 // channel 关闭时 recv() 返回 Err，循环退出
 ```

 **模式：**

 - `async_channel::Receiver<Event>`：异步事件接收器
 - `receiver.recv().await`：异步接收（返回 `Result`）
 - `while let Ok`：channel 关闭时退出循环
 - 生产者-消费者模式：子代理生产事件，`process_review_events` 消费

 ---

 ## 17.10 unwrap_or_else(panic!) 编译时保证模式

 ```rust
 Template::parse(normalized.as_ref())
     .unwrap_or_else(|err| panic!("review exit success template must parse: {err}"))

 REVIEW_EXIT_SUCCESS_TEMPLATE
     .render([("results", results)])
     .unwrap_or_else(|err| panic!("review exit success template must render: {err}"))
 ```

 **模式：**

 - `unwrap_or_else(panic!)`：用于编译时常量的运行时解析
 - 设计假设：模板是编译时包含的（`include_str!`），语法错误应在开发时发现
 - panic 比静默失败更好：暴露问题而非隐藏
 - 变量由代码控制，不应出现未知变量

 ---

 ## 17.11 深度比较测试模式

 ```rust
 let expected = ReviewOutputEvent {
     findings: vec![ReviewFinding {
         title: "Prefer Stylize helpers".to_string(),
         body: "Use .dim()/.bold() chaining...".to_string(),
         confidence_score: 0.9,
         priority: 1,
         code_location: ReviewCodeLocation {
             absolute_file_path: PathBuf::from("/tmp/file.rs"),
             line_range: ReviewLineRange { start: 10, end: 20 },
         },
     }],
     overall_correctness: "good".to_string(),
     overall_explanation: "All good...".to_string(),
     overall_confidence_score: 0.8,
 };
 assert_eq!(expected, review);
 ```

 **模式：**

 - 构造完整的预期对象
 - `assert_eq!` 深度比较（使用 `PartialEq`）
 - `pretty_assertions::assert_eq`：更清晰的 diff
 - 比逐字段比较更好：一次性验证所有字段
 - `f32` 的 `PartialEq` 在精确值上可用

 ---

 ## 17.12 wait_for_event 事件驱动测试模式

 ```rust
 let _entered = wait_for_event(&agere, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
 let closed = wait_for_event(&agere, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
 let _complete = wait_for_event(&agere, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
 ```

 **模式：**

 - `wait_for_event(agere, predicate)`：等待特定事件
 - `matches!(ev, pattern)`：模式匹配谓词
 - 事件驱动而非时间驱动（不用 `sleep`）
 - 按顺序等待事件序列
 - 比 `sleep` 更可靠（不依赖时序）

 ---

 > **一句话回顾**：常见模式总结了 12 种 Rust 惯用法——LazyLock（延迟初始化）、Constrained（约束+两种错误策略）、事件匹配抑制（`|` 模式组合）、三级降级（`if let Ok` 链）、Arc clone（共享 vs 深拷贝）、include_str!（编译时包含）、Cow（借用/拥有）、Template 渲染（`{{var}}` + const N）、async_channel（生产者-消费者）、unwrap_or_else(panic!)（编译时保证）、深度比较（assert_eq 整对象）、wait_for_event（事件驱动测试）。

 ---

 # 第十八部分 · 扩展场景与边界案例

 本部分补充更多实际可能遇到的场景和边界案例。

 ---

 ## 18.1 场景：审查空改动

 **触发：** 用户输入 `/review`，但 working tree 没有任何改动（无 staged/unstaged/untracked）。

 **行为：**

 ```
 1. resolve_review_request → UNCOMMITTED_PROMPT（固定文本）
 2. 子代理执行 git diff → 无输出
 3. 子代理可能返回:
    a. 空 findings + "No changes found" explanation
    b. 空 findings + 空 explanation（全空 → "Reviewer failed to output"）
    c. 纯文本 "No changes to review"
 ```

 **关键点：**

 - 代码层不检查是否有改动
 - 子代理自行发现无改动
 - 结果取决于模型如何处理空 diff

 ---

 ## 18.2 场景：审查超大 diff

 **触发：** 用户输入 `/review`，改动涉及大量文件。

 **行为：**

 ```
 1. 子代理执行 git diff → 大量输出
 2. 子代理可能:
    a. 只审查部分文件（token 限制）
    b. 返回较少 findings（信息过载）
    c. 返回截断的 JSON（token 限制）
 3. parse_review_output_event:
    a. 完整 JSON → 正常解析
    b. 截断 JSON → 策略 2 可能失败（不完整的 JSON）
    c. 策略 3 兜底（纯文本）
 ```

 **关键点：**

 - 代码层不限制 diff 大小
 - 子代理的 truncation_policy 可能触发（基于模型信息）
 - 大 diff 可能导致审查不完整
 - 建议：审查特定 commit 而非整个 working tree

 ---

 ## 18.3 场景：审查二进制文件改动

 **触发：** 改动包含二进制文件（如图片、编译产物）。

 **行为：**

 ```
 1. git diff 对二进制文件显示 "Binary files differ"
 2. 子代理无法分析二进制内容
 3. 子代理可能:
    a. 跳过二进制文件
    b. 标记"二进制文件改动无法审查"
    c. 忽略二进制文件
 ```

 **关键点：**

 - 代码层不过滤二进制文件
 - 子代理自行处理
 - REVIEW_PROMPT 未特别提及二进制文件

 ---

 ## 18.4 场景：审查包含删除的改动

 **触发：** 改动包含文件删除。

 **行为：**

 ```
 1. git diff 显示删除的文件（--- /dev/null）
 2. 子代理可能:
    a. 检查删除是否安全（如是否有其他文件引用）
    b. 标记"删除文件可能影响其他部分"（但需要可证明的影响）
    c. 忽略删除
 ```

 **关键点：**

 - REVIEW_PROMPT 准则 7 要求"可证明的影响"
 - 子代理不应仅凭推测标记删除的影响
 - 必须识别出受影响的具体代码

 ---

 ## 18.5 场景：自定义审查指令

 **触发：** 用户输入 `/review check error handling and security`（参数为自定义指令）。

 **行为：**

 ```
 1. ReviewTarget::Custom { instructions: "check error handling and security" }
 2. resolve_review_request:
    → prompt = "check error handling and security".trim()（非空）
    → user_facing_hint = "check error handling and security"
 3. 子代理收到:
    → 系统指令: REVIEW_PROMPT（审查准则）
    → 用户指令: "check error handling and security"（聚焦特定方面）
 4. 子代理聚焦于 error handling 和 security 进行审查
 ```

 **关键点：**

 - Custom 指令与 REVIEW_PROMPT 配合使用
 - REVIEW_PROMPT 定义审查准则和输出格式
 - Custom 指令定义审查重点
 - 如果指令为空（`/review  `）→ `anyhow::bail!` → emit Error

 ---

 ## 18.6 场景：审查时网络不可用

 **触发：** 审查执行时网络断开。

 **行为：**

 ```
 1. 子代理尝试调用 API → 网络错误
 2. run_agere_thread_one_shot 可能:
    a. 返回 Err → start_review_conversation 返回 None
       → exit_review_mode(None) → 中断模板
    b. 超时 → 取消 → TurnAborted → exit_review_mode(None)
 3. UI 显示中断提示
 ```

 **关键点：**

 - 网络错误导致审查失败
 - 用户需要重新运行 /review
 - 中断模板提示 "Please re-run /review and wait for it to complete"

 ---

 ## 18.7 场景：审查模型不支持 JSON 输出

 **触发：** `review_model` 配置为一个不支持结构化输出的模型。

 **行为：**

 ```
 1. 子代理收到 REVIEW_PROMPT（要求 JSON 输出）
 2. 但模型可能无法按 schema 输出
 3. 子代理返回:
    a. 纯文本 → parse 策略 3 兜底（overall_explanation）
    b. 不完整 JSON → parse 策略 2 可能提取子串
    c. 完全不符合 → parse 策略 3 兜底
 4. UI 显示 explanation（无结构化 findings）
 ```

 **关键点：**

 - 三级降级确保总有输出
 - 但可能没有结构化 findings
 - 建议：配置支持 JSON 输出的审查模型

 ---

 ## 18.8 场景：审查被快速取消后立即重新运行

 **触发：** 用户输入 `/review`，立刻按 Ctrl+C，然后再次输入 `/review`。

 **行为：**

 ```
 第一次 /review:
 1. emit EnteredReviewMode → UI 进入审查模式
 2. 取消 → abort() → exit_review_mode(None) → emit ExitedReviewMode(None)
 3. UI 退出审查模式
 4. rollout 记录中断模板

 第二次 /review:
 1. 正常流程（新 turn）
 2. emit EnteredReviewMode → UI 再次进入审查模式
 3. 审查完成 → emit ExitedReviewMode(Some)
 4. rollout 记录成功模板
 ```

 **关键点：**

 - 每次审查是独立的 turn
 - 取消不影响后续审查
 - rollout 中会有两条审查记录（中断 + 成功）
 - `pre_review_token_info` 在第一次退出时已恢复，第二次进入时重新保存

 ---

 ## 18.9 场景：审查结果中 code_location 指向不存在的文件

 **触发：** 子代理返回的 finding 中 `absolute_file_path` 指向一个不存在的文件。

 **行为：**

 ```
 1. parse_review_output_event 正常解析（不校验文件存在性）
 2. format_review_findings_block 正常格式化（显示路径）
 3. UI 显示 finding（路径可能无法点击/跳转）
 4. 用户可能无法定位问题
 ```

 **关键点：**

 - 代码层不校验 code_location 的有效性
 - 这是模型输出的问题，不是代码的 bug
 - REVIEW_PROMPT 要求 code_location 与 diff 重叠，但模型可能不遵守

 ---

 ## 18.10 场景：审查结果中 line_range 超出文件范围

 **触发：** 子代理返回的 finding 中 `line_range.start` 或 `end` 超出文件实际行数。

 **行为：**

 ```
 1. 代码层不校验行范围
 2. format_location 显示 "path:start-end"（即使超出范围）
 3. UI 可能无法正确高亮
 4. 用户可能无法定位问题
 ```

 **关键点：**

 - 代码层不校验行范围
 - REVIEW_PROMPT 要求行范围尽可能短，但模型可能不遵守
 - 这是模型输出质量问题

 ---

 > **一句话回顾**：扩展场景覆盖 10 个边界案例——空改动（子代理自行发现）、超大 diff（可能截断）、二进制文件（子代理自行处理）、文件删除（需可证明影响）、自定义指令（与 REVIEW_PROMPT 配合）、网络不可用（中断模板）、模型不支持 JSON（三级降级兜底）、快速取消重运行（独立 turn）、不存在的文件路径（不校验）、超出范围的行号（不校验）。

 ---

 ---

 # 第十九部分 · 协议层深度走读

 本部分对 `protocol/src/protocol.rs` 中审查相关的所有类型定义进行深度走读，包括完整的 serde/TS 注解和 JSON 示例。

 ---

 ## 19.1 Op::Review 操作定义

 ```rust
 // protocol/src/protocol.rs:827-828

 pub enum Op {
     // ... 其他变体 ...

     /// Request a code review from the agent.
     Review { review_request: ReviewRequest },

     // ... 其他变体 ...
 }
 ```

 **深度走读：**

 - `Op` 是会话操作枚举，`Review` 是其中一个变体
 - `review_request: ReviewRequest`：审查请求载荷
 - doc comment "Request a code review from the agent"
 - L951: `Op::Review { .. } => "review"` — 字符串标识

 **JSON 示例（Op::Review 序列化）：**

 ```json
 {
   "type": "review",
   "review_request": {
     "target": {
       "type": "uncommittedChanges"
     }
   }
 }
 ```

 **使用位置：**

 - TUI 斜杠命令分发：构造 `Op::Review` 并 submit
 - `handlers.rs:1210-1211`：submission loop 中匹配 `Op::Review` 并调用 `review()`
 - 测试中：`agere.submit(Op::Review { review_request })`

 ---

 ## 19.2 ReviewDelivery 枚举

 ```rust
 // protocol/src/protocol.rs:2798-2805

 #[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, JsonSchema, TS)]
 #[serde(rename_all = "snake_case")]
 pub enum ReviewDelivery {
     Inline,
     Detached,
 }
 ```

 **深度走读：**

 - `Clone, Copy`：零大小枚举，可 Copy
 - `Eq`：支持全等比较
 - `#[serde(rename_all = "snake_case")]`：序列化为 snake_case
 - v2 re-export：`app-server-protocol/src/protocol/v2.rs:414`

 **JSON 序列化：**

 ```json
 "inline"
 // 或
 "detached"
 ```

 **当前使用状态：**

 - 定义了但主流程主要走 `Inline`
 - `Detached` 模式预留但未深度使用
 - 在 v2 协议中 re-export

 ---

 ## 19.3 ReviewTarget 枚举（完整注解）

 ```rust
 // protocol/src/protocol.rs:2806-2830

 #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema, TS)]
 #[serde(tag = "type", rename_all = "camelCase")]
 #[ts(tag = "type")]
 pub enum ReviewTarget {
     /// Review the working tree: staged, unstaged, and untracked files.
     UncommittedChanges,

     /// Review changes between the current branch and the given base branch.
     #[serde(rename_all = "camelCase")]
     #[ts(rename_all = "camelCase")]
     BaseBranch { branch: String },

     /// Review the changes introduced by a specific commit.
     #[serde(rename_all = "camelCase")]
     #[ts(rename_all = "camelCase")]
     Commit {
         sha: String,
         /// Optional human-readable label (e.g., commit subject) for UIs.
         title: Option<String>,
     },

     /// Arbitrary instructions provided by the user.
     #[serde(rename_all = "camelCase")]
     #[ts(rename_all = "camelCase")]
     Custom { instructions: String },
 }
 ```

 **深度走读：**

 - `#[serde(tag = "type")]`：内部标签，JSON 中用 `"type"` 字段区分子类
 - `#[serde(rename_all = "camelCase")]`：顶层枚举用 camelCase
 - `#[ts(tag = "type")]`：TypeScript schema 也用内部标签
 - 每个有数据的变体额外标注 `#[serde(rename_all = "camelCase")]` 和 `#[ts(rename_all = "camelCase")]`
   - 确保字段名也是 camelCase

 **各变体 JSON 序列化：**

 ```
 UncommittedChanges:
 {"type": "uncommittedChanges"}

 BaseBranch { branch: "main" }:
 {"type": "baseBranch", "branch": "main"}

 Commit { sha: "abc123", title: Some("Fix") }:
 {"type": "commit", "sha": "abc123", "title": "Fix"}

 Commit { sha: "abc123", title: None }:
 {"type": "commit", "sha": "abc123"}
 // 注意: title 为 None 时，serde 默认序列化为 null
 // 但如果加了 skip_serializing_if，则省略

 Custom { instructions: "check security" }:
 {"type": "custom", "instructions": "check security"}
 ```

 **TypeScript schema 生成：**

 ```typescript
 type ReviewTarget =
   | { type: "uncommittedChanges" }
   | { type: "baseBranch", branch: string }
   | { type: "commit", sha: string, title?: string }
   | { type: "custom", instructions: string };
 ```

 **与 review_prompts.rs 的映射：**

 ```
 UncommittedChanges → UNCOMMITTED_PROMPT (固定文本)
 BaseBranch { branch } → BASE_BRANCH_PROMPT 或 BACKUP (含 merge-base)
 Commit { sha, title } → COMMIT_PROMPT 或 WITH_TITLE
 Custom { instructions } → 直接用 instructions (trim 后非空)
 ```

 ---

 ## 19.4 ReviewRequest 结构体

 ```rust
 // protocol/src/protocol.rs:2831-2838

 #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
 /// Review request sent to the review session.
 pub struct ReviewRequest {
     pub target: ReviewTarget,
     #[serde(skip_serializing_if = "Option::is_none")]
     #[ts(optional)]
     pub user_facing_hint: Option<String>,
 }
 ```

 **深度走读：**

 - doc comment "Review request sent to the review session"
 - `target: ReviewTarget`：审查目标（必填）
 - `user_facing_hint: Option<String>`：UI 提示文本（可选）
 - `#[serde(skip_serializing_if = "Option::is_none")]`：None 时省略字段
 - `#[ts(optional)]`：TypeScript schema 中标记为可选

 **JSON 序列化：**

 ```json
 // 有 hint
 {
   "target": { "type": "uncommittedChanges" },
   "user_facing_hint": "current changes"
 }

 // 无 hint
 {
   "target": { "type": "uncommittedChanges" }
 }
 ```

 **生命周期：**

 ```
 1. TUI 构造 ReviewRequest (hint 通常为 None)
 2. resolve_review_request 消费 ReviewRequest，生成 ResolvedReviewRequest
 3. spawn_review_thread 从 ResolvedReviewRequest 构造新的 ReviewRequest (hint 总是 Some)
 4. 新的 ReviewRequest 被 emit 为 EnteredReviewMode 事件
 5. TUI 从 EnteredReviewMode 收到 ReviewRequest，提取 hint
 ```

 **From<ResolvedReviewRequest> 转换：**

 ```rust
 // core/src/review_prompts.rs
 impl From<ResolvedReviewRequest> for ReviewRequest {
     fn from(resolved: ResolvedReviewRequest) -> Self {
         ReviewRequest {
             target: resolved.target,
             user_facing_hint: Some(resolved.user_facing_hint),
         }
     }
 }
 ```

 - 转换后 `user_facing_hint` 总是 `Some`（因为 `ResolvedReviewRequest` 一定有 hint）

 ---

 ## 19.5 ReviewOutputEvent 结构体

 ```rust
 // protocol/src/protocol.rs:2839-2857

 /// Structured review result produced by a child review session.
 #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
 pub struct ReviewOutputEvent {
     pub findings: Vec<ReviewFinding>,
     pub overall_correctness: String,
     pub overall_explanation: String,
     pub overall_confidence_score: f32,
 }

 impl Default for ReviewOutputEvent {
     fn default() -> Self {
         Self {
             findings: Vec::new(),
             overall_correctness: String::default(),
             overall_explanation: String::default(),
             overall_confidence_score: 0.0,
         }
     }
 }
 ```

 **深度走读：**

 - doc comment "Structured review result produced by a child review session"
 - `findings: Vec<ReviewFinding>`：审查发现列表（可为空 vec）
 - `overall_correctness: String`：正确性裁决
   - 期望值：`"patch is correct"` 或 `"patch is incorrect"`（由 review_prompt.md 约定）
   - 代码层不强制校验（模型可能返回其他值）
 - `overall_explanation: String`：总体解释（1-3 句）
 - `overall_confidence_score: f32`：整体置信度（0.0-1.0）

 **Default 实现：**

 - `findings: Vec::new()` — 空 vec
 - `overall_correctness: String::default()` — 空字符串
 - `overall_explanation: String::default()` — 空字符串
 - `overall_confidence_score: 0.0` — 零
 - 用于 `parse_review_output_event` 的兜底场景

 **JSON 序列化：**

 ```json
 {
   "findings": [
     {
       "title": "[P1] Buffer overflow",
       "body": "The parse function...",
       "confidence_score": 0.9,
       "priority": 1,
       "code_location": {
         "absolute_file_path": "/tmp/file.rs",
         "line_range": { "start": 10, "end": 20 }
       }
     }
   ],
   "overall_correctness": "patch is incorrect",
   "overall_explanation": "The buffer overflow...",
   "overall_confidence_score": 0.85
 }
 ```

 **Default 的 JSON：**

 ```json
 {
   "findings": [],
   "overall_correctness": "",
   "overall_explanation": "",
   "overall_confidence_score": 0.0
 }
 ```

 **使用位置：**

 - `parse_review_output_event()` 返回值
 - `ExitedReviewModeEvent.review_output` 的 `Some` 变体
 - `exit_review_mode()` 中渲染 user/assistant message
 - `on_exited_review_mode()` 中渲染 UI
 - `format_review_findings_block()` 和 `render_review_output_text()` 的输入

 ---

 ## 19.6 ReviewFinding 结构体

 ```rust
 // protocol/src/protocol.rs:2859-2867

 /// A single review finding describing an observed issue or recommendation.
 #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
 pub struct ReviewFinding {
     pub title: String,
     pub body: String,
     pub confidence_score: f32,
     pub priority: i32,
     pub code_location: ReviewCodeLocation,
 }
 ```

 **深度走读：**

 - doc comment "A single review finding describing an observed issue or recommendation"
 - `title: String`：标题
   - REVIEW_PROMPT 要求 ≤80 字符
   - 应包含优先级标记（如 `[P1]`）
   - 应使用 imperative 语气
 - `body: String`：正文
   - valid Markdown
   - 解释*为什么*是问题
   - 引用 files/lines/functions
   - 最多 1 段
   - 代码块不超过 3 行
 - `confidence_score: f32`：置信度（0.0-1.0）
 - `priority: i32`：优先级
   - 0=P0, 1=P1, 2=P2, 3=P3
   - REVIEW_PROMPT 说 optional，但 Rust 类型是 `i32`（非 Option）
   - 实际不能省略，默认值取决于模型
 - `code_location: ReviewCodeLocation`：代码位置（必填）

 **JSON 序列化：**

 ```json
 {
   "title": "[P1] Buffer overflow in parse function",
   "body": "The `parse` function doesn't check buffer bounds before reading, which can cause memory corruption when input exceeds 1024 bytes.",
   "confidence_score": 0.95,
   "priority": 1,
   "code_location": {
     "absolute_file_path": "/home/user/repo/src/parser.rs",
     "line_range": { "start": 42, "end": 48 }
   }
 }
 ```

 **格式化输出（format_review_findings_block）：**

 ```
 - [P1] Buffer overflow in parse function — /home/user/repo/src/parser.rs:42-48
   The `parse` function doesn't check buffer bounds before reading, which can
   cause memory corruption when input exceeds 1024 bytes.
 ```

 ---

 ## 19.7 ReviewCodeLocation 和 ReviewLineRange

 ```rust
 // protocol/src/protocol.rs:2869-2882

 /// Location of the code related to a review finding.
 #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
 pub struct ReviewCodeLocation {
     pub absolute_file_path: PathBuf,
     pub line_range: ReviewLineRange,
 }

 /// Inclusive line range in a file associated with the finding.
 #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
 pub struct ReviewLineRange {
     pub start: u32,
     pub end: u32,
 }
 ```

 **深度走读：**

 **ReviewCodeLocation:**
 - doc comment "Location of the code related to a review finding"
 - `absolute_file_path: PathBuf`：文件绝对路径
   - REVIEW_PROMPT 要求必填
   - 应与 diff 重叠
   - 代码层不校验文件存在性
 - `line_range: ReviewLineRange`：行范围

 **ReviewLineRange:**
 - doc comment "Inclusive line range in a file associated with the finding"
 - `start: u32`：起始行（inclusive）
 - `end: u32`：结束行（inclusive）
 - REVIEW_PROMPT 要求尽可能短（避免超过 5-10 行）
 - 代码层不校验行范围有效性

 **JSON 序列化：**

 ```json
 {
   "absolute_file_path": "/home/user/repo/src/parser.rs",
   "line_range": { "start": 42, "end": 48 }
 }
 ```

 **格式化输出（format_location）：**

 ```
 /home/user/repo/src/parser.rs:42-48
 ```

 ---

 ## 19.8 审查事件类型

 ### 19.8.1 EnteredReviewMode

 ```rust
 // protocol/src/protocol.rs:1336-1337

 pub enum EventMsg {
     // ...

     /// Entered review mode.
     EnteredReviewMode(ReviewRequest),  // L1337

     // ...
 }
 ```

 **深度走读：**

 - doc comment "Entered review mode"
 - 载荷：`ReviewRequest`（含 target 和 user_facing_hint）
 - emit 位置：`spawn_review_thread()` L178
 - 消费位置：TUI `on_entered_review_mode()`
 - 用途：通知 UI 进入审查模式

 **事件流：**

 ```
 spawn_review_thread:
   sess.send_event(&tc, EventMsg::EnteredReviewMode(review_request))

 TUI:
   EventMsg::EnteredReviewMode(review_request) =>
     on_entered_review_mode(review_request, from_replay)
       → enter_review_mode_with_hint(hint, from_replay)
 ```

 ### 19.8.2 ExitedReviewMode

 ```rust
 // protocol/src/protocol.rs:1339-1340

 pub enum EventMsg {
     // ...

     /// Exited review mode with an optional final result to apply.
     ExitedReviewMode(ExitedReviewModeEvent),  // L1340

     // ...
 }
 ```

 **深度走读：**

 - doc comment "Exited review mode with an optional final result to apply"
 - 载荷：`ExitedReviewModeEvent`（含 `Option<ReviewOutputEvent>`）
 - emit 位置：`exit_review_mode()` L222-226
 - 消费位置：TUI `on_exited_review_mode()`
 - 用途：通知 UI 退出审查模式并渲染结果

 **事件流：**

 ```
 exit_review_mode:
   sess.send_event(ctx, EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
       review_output,  // Some(成功) 或 None(中断)
   }))

 TUI:
   EventMsg::ExitedReviewMode(review) =>
     on_exited_review_mode(review)
       → 渲染 findings/explanation/error
       → exit_review_mode_after_item()
 ```

 ### 19.8.3 ExitedReviewModeEvent

 ```rust
 // protocol/src/protocol.rs:1801-1803

 #[derive(Debug, Clone, Deserialize, Serialize, PartialEq, JsonSchema, TS)]
 pub struct ExitedReviewModeEvent {
     pub review_output: Option<ReviewOutputEvent>,
 }
 ```

 **深度走读：**

 - `review_output: Option<ReviewOutputEvent>`：
   - `Some(ReviewOutputEvent)`：审查成功（可能 findings 为空）
   - `None`：审查中断/取消/启动失败
 - 注意：`Some(ReviewOutputEvent::default())` 也可能出现（全空，会显示 "failed"）

 ---

 ## 19.9 SubAgentSource::Review

 ```rust
 // protocol/src/protocol.rs:2432, 2552

 pub enum SubAgentSource {
     // ... 其他变体 ...
     Review,
     // ... 其他变体 ...
 }

 impl fmt::Display for SubAgentSource {
     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
         match self {
             // ... 其他变体 ...
             SubAgentSource::Review => f.write_str("review"),
             // ... 其他变体 ...
         }
     }
 }
 ```

 **深度走读：**

 - `Review` 变体：标记审查子代理
 - `Display`：序列化为 `"review"`
 - 使用位置：`run_agere_thread_one_shot(..., SubAgentSource::Review, ...)`
 - 用途：
   1. 标记子代理身份
   2. 下游可通过 SubAgentSource 调整行为
   3. 日志和遥测中标识审查子代理

 ---

 ## 19.10 NonSteerableTurnKind::Review

 ```rust
 // protocol/src/protocol.rs:2432 (另一个枚举中的 Review)
 // app-server-protocol/src/protocol/v2.rs:155

 pub enum NonSteerableTurnKind {
     // ...
     Review,
     // ...
 }
 ```

 **深度走读：**

 - `NonSteerableTurnKind`：不可引导的 turn 种类
 - `Review` 变体：审查属于不可引导 turn
 - 意味着审查 turn 不接受同 turn steering
 - `protocol.rs:4370`：`"cannot steer a review turn"` 错误
 - v2 映射：`CoreNonSteerableTurnKind::Review => Self::Review` (v2.rs:246)

 **"不可引导"的含义：**

 - 普通对话 turn 中，用户可以在同 turn 中追加消息（steering）
 - 审查 turn 中，用户不能追加消息（审查是独立的一次性任务）
 - 如果尝试 steer 审查 turn，会得到 "cannot steer a review turn" 错误

 ---

 ## 19.11 Guardian 相关类型（关联对比）

 ### 19.11.1 ReviewDecision

 ```rust
 // protocol/src/protocol.rs:3517-3559

 pub enum ReviewDecision {
     Approved,
     ApprovedExecpolicyAmendment { .. },
     ApprovedForSession,
     NetworkPolicyAmendment { .. },
     Abort,
 }

 impl ReviewDecision {
     fn as_str(&self) -> &'static str {
         match self {
             ReviewDecision::Approved => "approved",
             ReviewDecision::ApprovedExecpolicyAmendment { .. } => "approved_with_amendment",
             ReviewDecision::ApprovedForSession => "approved_for_session",
             // ...
         }
     }
 }
 ```

 **深度走读：**

 - Guardian 审查的决策枚举（非 `/review` 的输出）
 - `Approved`：批准操作
 - `ApprovedExecpolicyAmendment`：批准但修改 exec policy
 - `ApprovedForSession`：会话内批准
 - `NetworkPolicyAmendment`：批准但修改网络策略
 - `Abort`：拒绝
 - v2 映射（v2.rs:1285-1300）：
   - `Approved` → `Accept`
   - `ApprovedExecpolicyAmendment` → `AcceptWithAmendment`
   - `ApprovedForSession` → `AcceptForSession`
   - `NetworkPolicyAmendment` → `AcceptWithNetworkPolicy`
   - `Abort` → `Cancel`

 ### 19.11.2 ApprovalsReviewer

 ```rust
 // protocol/src/protocol.rs:3360-3364 (core)
 // app-server-protocol/src/protocol/v2.rs:328-332 (v2)

 pub enum ApprovalsReviewer {
     User,
     AutoReview,
 }

 // v2 中的转换
 impl ApprovalsReviewer {
     pub fn to_core(self) -> CoreApprovalsReviewer { ... }
 }

 impl From<CoreApprovalsReviewer> for ApprovalsReviewer { ... }
 ```

 **深度走读：**

 - `User`：用户手动审批
 - `AutoReview`：Guardian 自动审查
 - core 和 v2 之间双向转换
 - v2 中是 experimental：`#[experimental("config/read.approvalsReviewer")]`

 ---

 > **一句话回顾**：协议层深度走读覆盖了 11 个类型定义——`Op::Review`（操作）、`ReviewDelivery`（交付方式）、`ReviewTarget`（4 变体 tagged union + JSON/TS schema）、`ReviewRequest`（请求 + skip_serializing_if）、`ReviewOutputEvent`（输出 + Default）、`ReviewFinding`（单条发现）、`ReviewCodeLocation/LineRange`（位置）、`Entered/ExitedReviewMode`（事件）、`SubAgentSource::Review`（子代理标记）、`NonSteerableTurnKind::Review`（不可引导）、Guardian 的 `ReviewDecision/ApprovalsReviewer`（关联对比）。

 ---

 # 第二十部分 · 综合示例库

 本部分提供更多综合示例，涵盖各种组合场景。

 ---

 ## 20.1 示例：多个 findings 的完整审查

 **输入：** `/review`（UncommittedChanges）

 **子代理返回 JSON：**

 ```json
 {
   "findings": [
     {
       "title": "[P0] SQL injection vulnerability in query builder",
       "body": "The `build_query` function concatenates user input directly into SQL strings without parameterization, allowing SQL injection attacks.",
       "confidence_score": 0.98,
       "priority": 0,
       "code_location": {
         "absolute_file_path": "/home/user/repo/src/db/query.rs",
         "line_range": { "start": 15, "end": 22 }
       }
     },
     {
       "title": "[P1] Race condition in cache invalidation",
       "body": "The cache invalidation logic uses a non-atomic read-then-write pattern, which can cause stale data under concurrent access.",
       "confidence_score": 0.85,
       "priority": 1,
       "code_location": {
         "absolute_file_path": "/home/user/repo/src/cache.rs",
         "line_range": { "start": 88, "end": 95 }
       }
     },
     {
       "title": "[P2] Missing input validation for negative values",
       "body": "The `calculate_price` function doesn't validate that quantity is non-negative, which could produce incorrect pricing for negative quantities.",
       "confidence_score": 0.7,
       "priority": 2,
       "code_location": {
         "absolute_file_path": "/home/user/repo/src/pricing.rs",
         "line_range": { "start": 30, "end": 35 }
       }
     },
     {
       "title": "[P3] Inconsistent naming convention",
       "body": "The function `getUserData` uses camelCase while the rest of the module uses snake_case, reducing code consistency.",
       "confidence_score": 0.6,
       "priority": 3,
       "code_location": {
         "absolute_file_path": "/home/user/repo/src/api.rs",
         "line_range": { "start": 12, "end": 12 }
       }
     }
   ],
   "overall_correctness": "patch is incorrect",
   "overall_explanation": "The SQL injection vulnerability is a critical security issue that must be fixed before merging. The race condition and missing validation should also be addressed.",
   "overall_confidence_score": 0.92
 }
 ```

 **exit_review_mode 渲染的 user message：**

 ```xml
 <user_action>
   <context>User initiated a review task. Here's the full review output from reviewer model. User may select one or more comments to resolve.</context>
   <action>review</action>
   <results>
   The SQL injection vulnerability is a critical security issue that must be fixed before merging. The race condition and missing validation should also be addressed.

 Full review comments:

 - [P0] SQL injection vulnerability in query builder — /home/user/repo/src/db/query.rs:15-22
   The `build_query` function concatenates user input directly into SQL strings without parameterization, allowing SQL injection attacks.

 - [P1] Race condition in cache invalidation — /home/user/repo/src/cache.rs:88-95
   The cache invalidation logic uses a non-atomic read-then-write pattern, which can cause stale data under concurrent access.

 - [P2] Missing input validation for negative values — /home/user/repo/src/pricing.rs:30-35
   The `calculate_price` function doesn't validate that quantity is non-negative, which could produce incorrect pricing for negative quantities.

 - [P3] Inconsistent naming convention — /home/user/repo/src/api.rs:12-12
   The function `getUserData` uses camelCase while the rest of the module uses snake_case, reducing code consistency.
   </results>
 </user_action>
 ```

 **exit_review_mode 渲染的 assistant message：**

 ```
 The SQL injection vulnerability is a critical security issue that must be fixed before merging. The race condition and missing validation should also be addressed.

 Full review comments:

 - [P0] SQL injection vulnerability in query builder — /home/user/repo/src/db/query.rs:15-22
   The `build_query` function concatenates user input directly into SQL strings without parameterization, allowing SQL injection attacks.

 - [P1] Race condition in cache invalidation — /home/user/repo/src/cache.rs:88-95
   The cache invalidation logic uses a non-atomic read-then-write pattern, which can cause stale data under concurrent access.

 - [P2] Missing input validation for negative values — /home/user/repo/src/pricing.rs:30-35
   The `calculate_price` function doesn't validate that quantity is non-negative, which could produce incorrect pricing for negative quantities.

 - [P3] Inconsistent naming convention — /home/user/repo/src/api.rs:12-12
   The function `getUserData` uses camelCase while the rest of the module uses snake_case, reducing code consistency.
 ```

 **TUI 显示：**

 ```
 >> Code review started: current changes <<

 The SQL injection vulnerability is a critical security issue that must be fixed
 before merging. The race condition and missing validation should also be addressed.

 Full review comments:

 - [P0] SQL injection vulnerability in query builder — /home/user/repo/src/db/query.rs:15-22
   The `build_query` function concatenates user input directly into SQL strings
   without parameterization, allowing SQL injection attacks.

 - [P1] Race condition in cache invalidation — /home/user/repo/src/cache.rs:88-95
   The cache invalidation logic uses a non-atomic read-then-write pattern, which
   can cause stale data under concurrent access.

 - [P2] Missing input validation for negative values — /home/user/repo/src/pricing.rs:30-35
   The `calculate_price` function doesn't validate that quantity is non-negative,
   which could produce incorrect pricing for negative quantities.

 - [P3] Inconsistent naming convention — /home/user/repo/src/api.rs:12-12
   The function `getUserData` uses camelCase while the rest of the module uses
   snake_case, reducing code consistency.

 << Code review finished >>
 ```

 ---

 ## 20.2 示例：无 findings 的审查（代码正确）

 **输入：** `/review abc1234`（Commit）

 **子代理返回 JSON：**

 ```json
 {
   "findings": [],
   "overall_correctness": "patch is correct",
   "overall_explanation": "The changes are well-structured and don't introduce any bugs. All edge cases are handled correctly.",
   "overall_confidence_score": 0.9
 }
 ```

 **exit_review_mode 渲染：**

 ```
 user_message:
 <user_action>
   <context>User initiated a review task...</context>
   <action>review</action>
   <results>
   The changes are well-structured and don't introduce any bugs. All edge cases are handled correctly.
   </results>
 </user_action>
 // 注意: findings 为空，不追加 findings block

 assistant_message:
 The changes are well-structured and don't introduce any bugs. All edge cases are handled correctly.
 // 只有 explanation，无 findings block
 ```

 **TUI 显示：**

 ```
 >> Code review started: commit abc1234 <<

 The changes are well-structured and don't introduce any bugs. All edge cases
 are handled correctly.

 << Code review finished >>
 ```

 ---

 ## 20.3 示例：JSON 嵌在文本中的解析

 **子代理返回：**

 ```
 I've reviewed the changes and found the following issues:

 {"findings": [{"title": "[P1] Memory leak", "body": "The allocated buffer is never freed.", "confidence_score": 0.9, "priority": 1, "code_location": {"absolute_file_path": "/tmp/main.rs", "line_range": {"start": 10, "end": 15}}}], "overall_correctness": "patch is incorrect", "overall_explanation": "Memory leak detected.", "overall_confidence_score": 0.85}

 Please address these issues before merging.
 ```

 **parse_review_output_event 解析过程：**

 ```
 策略 1: serde_json::from_str(整体文本)
   → 失败（文本不是纯 JSON，有前后文字）

 策略 2: 提取子串
   start = text.find('{') = 62 (第一个 { 的位置)
   end = text.rfind('}') = 最后一个 } 的位置
   slice = text[62..=end] = '{"findings": [...], ...}'
   serde_json::from_str(slice)
   → 成功！
   → 返回 ReviewOutputEvent { findings: [...], ... }
 ```

 **解析结果：**

 ```rust
 ReviewOutputEvent {
     findings: vec![ReviewFinding {
         title: "[P1] Memory leak".to_string(),
         body: "The allocated buffer is never freed.".to_string(),
         confidence_score: 0.9,
         priority: 1,
         code_location: ReviewCodeLocation {
             absolute_file_path: PathBuf::from("/tmp/main.rs"),
             line_range: ReviewLineRange { start: 10, end: 15 },
         },
     }],
     overall_correctness: "patch is incorrect".to_string(),
     overall_explanation: "Memory leak detected.".to_string(),
     overall_confidence_score: 0.85,
 }
 ```

 ---

 ## 20.4 示例：BaseBranch 审查的完整 prompt 链

 **输入：** `/review develop`（审查相对 develop 分支的改动）

 **当前仓库状态：**

 ```
 当前分支: feature/new-api
 develop 分支: abc123def456
 merge-base(feature/new-api, develop): abc123def456
 (即 develop 是 feature/new-api 的祖先，没有分叉)
 ```

 **resolve_review_request 过程：**

 ```
 1. target = BaseBranch { branch: "develop" }
 2. review_prompt(BaseBranch { "develop" }, cwd):
    a. merge_base_with_head(cwd, "develop")
       → git merge-base HEAD develop
       → Ok(Some("abc123def456"))
    b. 有 merge-base → 用精确模板:
       render(BASE_BRANCH_PROMPT_TEMPLATE, [
           ("base_branch", "develop"),
           ("merge_base_sha", "abc123def456"),
       ])
       = "Review the code changes against the base branch 'develop'.
          The merge base commit for this comparison is abc123def456.
          Run `git diff abc123def456` to inspect the changes relative
          to develop. Provide prioritized, actionable findings."
 3. user_facing_hint = "changes against 'develop'"
 ```

 **子代理收到的完整输入：**

 ```
 系统指令 (instructions):
   REVIEW_PROMPT (review_prompt.md 全文)
   "You are acting as a reviewer for a proposed code change..."

 用户指令 (input):
   "Review the code changes against the base branch 'develop'.
    The merge base commit for this comparison is abc123def456.
    Run `git diff abc123def456` to inspect the changes relative
    to develop. Provide prioritized, actionable findings."
 ```

 **子代理执行：**

 ```
 1. 子代理执行: git diff abc123def456
    → 显示 feature/new-api 相对 develop 的所有改动
 2. 子代理分析 diff
 3. 子代理生成 findings JSON
 4. 返回 TurnComplete { last_agent_message: Some(json) }
 ```

 ---

 ## 20.5 示例：Custom 指令审查

 **输入：** `/review Focus on security and error handling in the authentication module`

 **resolve_review_request 过程：**

 ```
 1. target = Custom { instructions: "Focus on security and error handling in the authentication module" }
 2. review_prompt(Custom { instructions }, cwd):
    prompt = instructions.trim() = "Focus on security and error handling in the authentication module"
    prompt.is_empty() = false
    → Ok("Focus on security and error handling in the authentication module")
 3. user_facing_hint = "Focus on security and error handling in the authentication module"
 ```

 **子代理收到的完整输入：**

 ```
 系统指令:
   REVIEW_PROMPT (审查准则 + 输出 schema)

 用户指令:
   "Focus on security and error handling in the authentication module"
 ```

 **关键点：**

 - Custom 指令与 REVIEW_PROMPT 配合：REVIEW_PROMPT 定义审查准则和输出格式，Custom 指令定义审查重点
 - 子代理会聚焦于 security 和 error handling
 - 但输出格式仍是 ReviewOutputEvent JSON（由 REVIEW_PROMPT 约束）
 - UI banner 显示完整指令文本（可能很长）

 ---

 > **一句话回顾**：综合示例库提供了 5 个完整场景——多 findings 审查（P0-P3 四条 + 完整渲染链）、无 findings 审查（代码正确 + 只有 explanation）、JSON 嵌在文本中（策略 2 子串提取解析）、BaseBranch 审查（完整 prompt 链含 merge-base 计算）、Custom 指令审查（聚焦特定方面 + 与 REVIEW_PROMPT 配合）。

 ---

 ---

 # 第二十一部分 · handlers.rs 审查入口深度走读

 本部分对 `core/src/session/handlers.rs` 中的审查入口函数 `review()` 和 `Op::Review` 分发逻辑进行深度走读。

 ---

 ## 21.1 review() 函数完整走读

 ```rust
 // core/src/session/handlers.rs:1002-1024

 pub async fn review(sess: &Arc<Session>, sub_id: String, review_request: ReviewRequest) {
 ```

 **走读：**

 - `pub async fn review`：公开异步函数
 - `sess: &Arc<Session>`：会话引用（借用，非拥有）
 - `sub_id: String`：子 turn ID（由 submission loop 生成）
 - `review_request: ReviewRequest`：审查请求（move 语义）
 - 返回 `()`：无返回值（结果通过事件传递）

 ```rust
     let turn_context = sess.new_default_turn_with_sub_id(sub_id.clone()).await;
 ```

 **走读：**

 - `new_default_turn_with_sub_id(sub_id)`：创建默认 turn 上下文
   - 生成新的 turn context，包含默认配置
   - `sub_id` 用于标识这个 turn
   - `.await`：异步操作（可能涉及 I/O）
 - `turn_context`：审查 turn 的上下文（注意：这不是最终的审查子代理上下文，`spawn_review_thread` 会构建另一个）

 ```rust
     sess.maybe_emit_unknown_model_warning_for_turn(turn_context.as_ref())
         .await;
 ```

 **走读：**

 - `maybe_emit_unknown_model_warning_for_turn`：如果模型未知，emit 警告
   - 检查 `turn_context` 中的模型是否已知
   - 如果未知，emit `ErrorEvent` 或警告事件
   - `.await`：异步操作
 - 这是在审查开始前的安全检查

 ```rust
     sess.refresh_mcp_servers_if_requested(&turn_context).await;
 ```

 **走读：**

 - `refresh_mcp_servers_if_requested`：如果请求了 MCP 服务器刷新，执行刷新
   - 检查是否有 MCP 服务器刷新请求
   - 如果有，刷新 MCP 服务器列表
   - `.await`：异步操作
 - 确保审查时 MCP 服务器是最新的

 ```rust
     match resolve_review_request(review_request, &turn_context.cwd) {
         Ok(resolved) => {
             spawn_review_thread(Arc::clone(sess), turn_context.clone(), sub_id, resolved).await;
         }
         Err(err) => {
             let event = Event {
                 id: sub_id,
                 msg: EventMsg::Error(ErrorEvent {
                     message: err.to_string(),
                     agere_error_info: Some(AgereErrorInfo::Other),
                 }),
             };
             sess.send_event(&turn_context, event.msg).await;
         }
     }
 }
 ```

 **走读：**

 - `resolve_review_request(review_request, &turn_context.cwd)`：解析审查请求
   - `review_request`：move 消费
   - `&turn_context.cwd`：使用 turn 上下文的 cwd（可能被覆盖）
   - 返回 `anyhow::Result<ResolvedReviewRequest>`

 - `Ok(resolved)`：解析成功
   - `spawn_review_thread(Arc::clone(sess), turn_context.clone(), sub_id, resolved)`：spawn 审查线程
     - `Arc::clone(sess)`：clone 会话 Arc（引用计数+1）
     - `turn_context.clone()`：clone turn 上下文
     - `sub_id`：子 turn ID
     - `resolved`：解析后的审查请求
   - `.await`：异步操作

 - `Err(err)`：解析失败
   - 构造 `Event`：
     - `id: sub_id`：turn ID
     - `msg: EventMsg::Error(ErrorEvent { ... })`：错误事件
       - `message: err.to_string()`：错误消息
       - `agere_error_info: Some(AgereErrorInfo::Other)`：错误分类
   - `sess.send_event(&turn_context, event.msg)`：emit 错误事件
   - `.await`：异步操作
   - 注意：不进入审查模式，不 emit EnteredReviewMode

 ---

 ## 21.2 Op::Review 分发逻辑

 ```rust
 // core/src/session/handlers.rs:1210-1211

 // 在 submission_loop 中
 Op::Review { review_request } => {
     review(&sess, sub.id.clone(), review_request).await;
 }
 ```

 **走读：**

 - `Op::Review { review_request }`：模式匹配，解构出 `review_request`
 - `review(&sess, sub.id.clone(), review_request)`：调用审查入口
   - `&sess`：会话引用
   - `sub.id.clone()`：clone 子 turn ID
   - `review_request`：move 审查请求
 - `.await`：异步操作

 **submission_loop 上下文：**

 ```rust
 pub(super) async fn submission_loop(
     sess: Arc<Session>,
     config: Arc<Config>,
     rx_sub: Receiver<Submission>,
 ) {
     // To break out of this loop, send Op::Shutdown.
     while let Ok(sub) = rx_sub.recv().await {
         // ... 处理 sub ...
         match sub.op {
             // ... 其他 Op 变体 ...
             Op::Review { review_request } => {
                 review(&sess, sub.id.clone(), review_request).await;
             }
             // ... 其他 Op 变体 ...
         }
     }
 }
 ```

 **走读：**

 - `submission_loop`：提交循环，持续接收 `Submission` 并处理
 - `while let Ok(sub) = rx_sub.recv().await`：接收提交（channel 关闭时退出）
 - `match sub.op`：匹配操作类型
 - `Op::Review` 分支：调用 `review()`
 - 每个 Op 变体有自己的处理分支

 ---

 ## 21.3 review() 与 spawn_review_thread() 的关系

 ```
 review()                          spawn_review_thread()
 ─────────                         ──────────────────────
 入口函数                           实际构建审查上下文
 创建默认 turn_context              构建审查专用 TurnContext
 emit 未知模型警告                  确定审查模型
 刷新 MCP 服务器                    裁剪 features
 resolve_review_request            构建 ToolsConfig
   ├ Ok → spawn_review_thread      构造初始 input
   └ Err → emit ErrorEvent         spawn_task(ReviewTask)
                                    emit EnteredReviewMode
 ```

 **关键区别：**

 - `review()` 是入口，负责"准备审查"（解析请求、检查模型、刷新 MCP）
 - `spawn_review_thread()` 是执行，负责"构建隔离上下文并启动审查任务"
 - `review()` 的 `turn_context` 是默认的，`spawn_review_thread()` 会构建专用的 `review_turn_context`
 - `review()` 在主会话线程上同步执行，`spawn_review_thread()` spawn 后立即返回

 ---

 ## 21.4 错误传播链

 ```
 resolve_review_request 失败
   │
   ├─ Custom 指令为空
   │  → anyhow::bail!("Review prompt cannot be empty")
   │  → review() 中 Err(err)
   │  → emit ErrorEvent { message: "Review prompt cannot be empty" }
   │  → UI 显示错误，不进入审查模式
   │
   ├─ merge_base_with_head 失败
   │  → git 命令失败或 cwd 非 git 仓库
   │  → review() 中 Err(err)
   │  → emit ErrorEvent { message: err.to_string() }
   │  → UI 显示错误，不进入审查模式
   │
   └─ 其他 anyhow 错误
      → review() 中 Err(err)
      → emit ErrorEvent { message: err.to_string() }
      → UI 显示错误，不进入审查模式
 ```

 **关键点：**

 - resolve 失败不会进入审查模式（不 emit EnteredReviewMode）
 - 错误通过 `EventMsg::Error` 传播给 UI
 - `AgereErrorInfo::Other`：通用错误分类
 - 用户需要修正问题后重新运行 `/review`

 ---

 ## 21.5 review() 的调用者与被调用者

 **调用者：**

 - `submission_loop` 中的 `Op::Review` 分支（L1210-1211）

 **被调用者：**

 - `resolve_review_request()` — 解析审查请求
 - `sess.new_default_turn_with_sub_id()` — 创建 turn 上下文
 - `sess.maybe_emit_unknown_model_warning_for_turn()` — 模型警告
 - `sess.refresh_mcp_servers_if_requested()` — MCP 刷新
 - `spawn_review_thread()` — spawn 审查线程
 - `sess.send_event()` — emit 错误事件

 ---

 > **一句话回顾**：handlers.rs 深度走读揭示了审查入口 `review()` 的 5 步流程——创建 turn_context、emit 模型警告、刷新 MCP、resolve 审查请求（Ok→spawn/Err→emit Error）、以及 submission_loop 中 `Op::Review` 的分发逻辑；resolve 失败不进入审查模式，通过 ErrorEvent 传播错误。

 ---

 # 第二十二部分 · 完整源码清单

 本部分提供审查相关核心文件的完整源码（简化版），方便对照阅读。

 ---

 ## 22.1 core/src/tasks/review.rs 完整源码（简化）

 ```rust
 // === 导入 ===
 use std::borrow::Cow;
 use std::sync::Arc;
 use std::sync::LazyLock;

 use agere_protocol::config_types::WebSearchMode;
 use agere_protocol::items::TurnItem;
 use agere_protocol::models::ContentItem;
 use agere_protocol::models::ResponseItem;
 use agere_protocol::protocol::*;
 use agere_utils_common::Template;
 use tokio_util::sync::CancellationToken;

 use crate::agere_delegate::run_agere_thread_one_shot;
 use crate::config::Constrained;
 use crate::review_format::{format_review_findings_block, render_review_output_text};
 use crate::session::session::Session;
 use crate::session::turn_context::TurnContext;
 use crate::state::TaskKind;
 use agere_features::Feature;
 use agere_protocol::user_input::UserInput;

 use super::{SessionTask, SessionTaskContext};

 // === 常量 ===

 static REVIEW_EXIT_SUCCESS_TEMPLATE: LazyLock<Template> = LazyLock::new(|| {
     let normalized = normalize_review_template_line_endings(
         crate::client_common::REVIEW_EXIT_SUCCESS_TMPL,
     );
     Template::parse(normalized.as_ref())
         .unwrap_or_else(|err| panic!("review exit success template must parse: {err}"))
 });

 // === ReviewTask ===

 #[derive(Clone, Copy)]
 pub(crate) struct ReviewTask;

 impl ReviewTask {
     pub(crate) fn new() -> Self { Self }
 }

 impl SessionTask for ReviewTask {
     fn kind(&self) -> TaskKind { TaskKind::Review }

     fn span_name(&self) -> &'static str { "session_task.review" }

     async fn run(
         self: Arc<Self>,
         session: Arc<SessionTaskContext>,
         ctx: Arc<TurnContext>,
         input: Vec<UserInput>,
         cancellation_token: CancellationToken,
     ) -> Option<String> {
         session.session.services.session_telemetry.counter(
             "agere.task.review", 1, &[],
         );

         let output = match start_review_conversation(
             session.clone(), ctx.clone(), input, cancellation_token.clone(),
         ).await {
             Some(receiver) => process_review_events(session.clone(), ctx.clone(), receiver).await,
             None => None,
         };

         if !cancellation_token.is_cancelled() {
             exit_review_mode(session.clone_session(), output.clone(), ctx.clone()).await;
         }
         None
     }

     async fn abort(&self, session: Arc<SessionTaskContext>, ctx: Arc<TurnContext>) {
         exit_review_mode(session.clone_session(), None, ctx).await;
     }
 }

 // === start_review_conversation ===

 async fn start_review_conversation(
     session: Arc<SessionTaskContext>,
     ctx: Arc<TurnContext>,
     input: Vec<UserInput>,
     cancellation_token: CancellationToken,
 ) -> Option<async_channel::Receiver<Event>> {
     let config = ctx.config.clone();
     let mut sub_agent_config = config.as_ref().clone();

     if let Err(err) = sub_agent_config.web_search_mode.set(WebSearchMode::Disabled) {
         panic!("by construction Constrained<WebSearchMode> must always support Disabled: {err}");
     }
     let _ = sub_agent_config.features.disable(Feature::SpawnCsv);
     let _ = sub_agent_config.features.disable(Feature::Collab);

     sub_agent_config.base_instructions = Some(crate::REVIEW_PROMPT.to_string());
     sub_agent_config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);

     let model = config.review_model.clone()
         .unwrap_or_else(|| ctx.model_info.slug.clone());
     sub_agent_config.model = Some(model);

     run_agere_thread_one_shot(
         sub_agent_config, session.auth_manager(), session.models_manager(),
         input, session.clone_session(), ctx.clone(), cancellation_token,
         SubAgentSource::Review, None, None,
     ).await.ok().map(|io| io.rx_event)
 }

 // === process_review_events ===

 async fn process_review_events(
     session: Arc<SessionTaskContext>,
     ctx: Arc<TurnContext>,
     receiver: async_channel::Receiver<Event>,
 ) -> Option<ReviewOutputEvent> {
     let mut prev_agent_message: Option<Event> = None;

     while let Ok(event) = receiver.recv().await {
         match event.clone().msg {
             EventMsg::AgentMessage(_) => {
                 if let Some(prev) = prev_agent_message.take() {
                     session.clone_session().send_event(ctx.as_ref(), prev.msg).await;
                 }
                 prev_agent_message = Some(event);
             }
             EventMsg::ItemCompleted(ItemCompletedEvent {
                 item: TurnItem::AgentMessage(_), ..
             })
             | EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { .. })
             | EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent { .. }) => {}
             EventMsg::TurnComplete(task_complete) => {
                 return task_complete.last_agent_message.as_deref()
                     .map(parse_review_output_event);
             }
             EventMsg::TurnAborted(_) => return None,
             other => {
                 session.clone_session().send_event(ctx.as_ref(), other).await;
             }
         }
     }
     None
 }

 // === parse_review_output_event ===

 fn parse_review_output_event(text: &str) -> ReviewOutputEvent {
     if let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(text) {
         return ev;
     }
     if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}'))
         && start < end
         && let Some(slice) = text.get(start..=end)
         && let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(slice)
     {
         return ev;
     }
     ReviewOutputEvent {
         overall_explanation: text.to_string(),
         ..Default::default()
     }
 }

 // === exit_review_mode ===

 pub(crate) async fn exit_review_mode(
     session: Arc<Session>,
     review_output: Option<ReviewOutputEvent>,
     ctx: Arc<TurnContext>,
 ) {
     const REVIEW_USER_MESSAGE_ID: &str = "review_rollout_user";
     const REVIEW_ASSISTANT_MESSAGE_ID: &str = "review_rollout_assistant";

     let (user_message, assistant_message) = if let Some(out) = review_output.clone() {
         let mut findings_str = String::new();
         let text = out.overall_explanation.trim();
         if !text.is_empty() { findings_str.push_str(text); }
         if !out.findings.is_empty() {
             let block = format_review_findings_block(&out.findings, None);
             findings_str.push_str(&format!("\n{block}"));
         }
         let rendered = render_review_exit_success(&findings_str);
         let assistant_message = render_review_output_text(&out);
         (rendered, assistant_message)
     } else {
         let rendered = normalize_review_template_line_endings(
             crate::client_common::REVIEW_EXIT_INTERRUPTED_TMPL,
         ).into_owned();
         let assistant_message = "Review was interrupted. Please re-run /review and wait for it to complete.".to_string();
         (rendered, assistant_message)
     };

     session.record_conversation_items(&ctx, &[ResponseItem::Message {
         id: Some(REVIEW_USER_MESSAGE_ID.to_string()),
         role: "user".to_string(),
         content: vec![ContentItem::InputText { text: user_message }],
         phase: None,
     }]).await;

     session.send_event(ctx.as_ref(), EventMsg::ExitedReviewMode(
         ExitedReviewModeEvent { review_output },
     )).await;

     session.record_response_item_and_emit_turn_item(ctx.as_ref(), ResponseItem::Message {
         id: Some(REVIEW_ASSISTANT_MESSAGE_ID.to_string()),
         role: "assistant".to_string(),
         content: vec![ContentItem::OutputText { text: assistant_message }],
         phase: None,
     }).await;

     session.ensure_rollout_materialized().await;
 }

 // === 辅助函数 ===

 fn render_review_exit_success(results: &str) -> String {
     REVIEW_EXIT_SUCCESS_TEMPLATE
         .render([("results", results)])
         .unwrap_or_else(|err| panic!("review exit success template must render: {err}"))
 }

 fn normalize_review_template_line_endings(template: &str) -> Cow<'_, str> {
     if template.contains('\r') {
         Cow::Owned(template.replace("\r\n", "\n").replace('\r', "\n"))
     } else {
         Cow::Borrowed(template)
     }
 }
 ```

 ---

 ## 22.2 core/src/review_format.rs 完整源码

 ```rust
 use agere_protocol::protocol::ReviewFinding;
 use agere_protocol::protocol::ReviewOutputEvent;

 fn format_location(item: &ReviewFinding) -> String {
     let path = item.code_location.absolute_file_path.display();
     let start = item.code_location.line_range.start;
     let end = item.code_location.line_range.end;
     format!("{path}:{start}-{end}")
 }

 const REVIEW_FALLBACK_MESSAGE: &str = "Reviewer failed to output a response.";

 pub fn format_review_findings_block(
     findings: &[ReviewFinding],
     selection: Option<&[bool]>,
 ) -> String {
     let mut lines: Vec<String> = Vec::new();
     lines.push(String::new());

     if findings.len() > 1 {
         lines.push("Full review comments:".to_string());
     } else {
         lines.push("Review comment:".to_string());
     }

     for (idx, item) in findings.iter().enumerate() {
         lines.push(String::new());
         let title = &item.title;
         let location = format_location(item);

         if let Some(flags) = selection {
             let checked = flags.get(idx).copied().unwrap_or(true);
             let marker = if checked { "[x]" } else { "[ ]" };
             lines.push(format!("- {marker} {title} — {location}"));
         } else {
             lines.push(format!("- {title} — {location}"));
         }

         for body_line in item.body.lines() {
             lines.push(format!("  {body_line}"));
         }
     }
     lines.join("\n")
 }

 pub fn render_review_output_text(output: &ReviewOutputEvent) -> String {
     let mut sections = Vec::new();
     let explanation = output.overall_explanation.trim();
     if !explanation.is_empty() {
         sections.push(explanation.to_string());
     }
     if !output.findings.is_empty() {
         let findings = format_review_findings_block(&output.findings, None);
         let trimmed = findings.trim();
         if !trimmed.is_empty() {
             sections.push(trimmed.to_string());
         }
     }
     if sections.is_empty() {
         REVIEW_FALLBACK_MESSAGE.to_string()
     } else {
         sections.join("\n\n")
     }
 }
 ```

 ---

 ## 22.3 core/src/review_prompts.rs 完整源码（简化）

 ```rust
 use agere_git_utils::merge_base_with_head;
 use agere_protocol::protocol::{ReviewRequest, ReviewTarget};
 use agere_utils_common::Template;
 use agere_utils_fs::AbsolutePathBuf;
 use std::sync::LazyLock;

 #[derive(Clone, Debug, PartialEq)]
 pub struct ResolvedReviewRequest {
     pub target: ReviewTarget,
     pub prompt: String,
     pub user_facing_hint: String,
 }

 const UNCOMMITTED_PROMPT: &str = "Review the current code changes (staged, unstaged, and untracked files) and provide prioritized findings.";

 const BASE_BRANCH_PROMPT_BACKUP: &str = "Review the code changes against the base branch '{{branch}}'. Start by finding the merge diff...";
 const BASE_BRANCH_PROMPT: &str = "Review the code changes against the base branch '{{base_branch}}'. The merge base commit for this comparison is {{merge_base_sha}}. Run `git diff {{merge_base_sha}}`...";

 const COMMIT_PROMPT_WITH_TITLE: &str = "Review the code changes introduced by commit {{sha}} (\"{{title}}\"). Provide prioritized, actionable findings.";
 const COMMIT_PROMPT: &str = "Review the code changes introduced by commit {{sha}}. Provide prioritized, actionable findings.";

 static BASE_BRANCH_PROMPT_BACKUP_TEMPLATE: LazyLock<Template> = /* ... */;
 static BASE_BRANCH_PROMPT_TEMPLATE: LazyLock<Template> = /* ... */;
 static COMMIT_PROMPT_WITH_TITLE_TEMPLATE: LazyLock<Template> = /* ... */;
 static COMMIT_PROMPT_TEMPLATE: LazyLock<Template> = /* ... */;

 pub fn resolve_review_request(
     request: ReviewRequest,
     cwd: &AbsolutePathBuf,
 ) -> anyhow::Result<ResolvedReviewRequest> {
     let target = request.target;
     let prompt = review_prompt(&target, cwd)?;
     let user_facing_hint = request.user_facing_hint
         .unwrap_or_else(|| user_facing_hint(&target));
     Ok(ResolvedReviewRequest { target, prompt, user_facing_hint })
 }

 pub fn review_prompt(target: &ReviewTarget, cwd: &AbsolutePathBuf) -> anyhow::Result<String> {
     match target {
         ReviewTarget::UncommittedChanges => Ok(UNCOMMITTED_PROMPT.to_string()),
         ReviewTarget::BaseBranch { branch } => {
             if let Some(commit) = merge_base_with_head(cwd, branch)? {
                 Ok(render_review_prompt(&BASE_BRANCH_PROMPT_TEMPLATE, [
                     ("base_branch", branch.as_str()),
                     ("merge_base_sha", commit.as_str()),
                 ]))
             } else {
                 Ok(render_review_prompt(&BASE_BRANCH_PROMPT_BACKUP_TEMPLATE, [
                     ("branch", branch.as_str()),
                 ]))
             }
         }
         ReviewTarget::Commit { sha, title } => {
             if let Some(title) = title {
                 Ok(render_review_prompt(&COMMIT_PROMPT_WITH_TITLE_TEMPLATE, [
                     ("sha", sha.as_str()), ("title", title.as_str()),
                 ]))
             } else {
                 Ok(render_review_prompt(&COMMIT_PROMPT_TEMPLATE, [
                     ("sha", sha.as_str()),
                 ]))
             }
         }
         ReviewTarget::Custom { instructions } => {
             let prompt = instructions.trim();
             if prompt.is_empty() { anyhow::bail!("Review prompt cannot be empty"); }
             Ok(prompt.to_string())
         }
     }
 }

 fn render_review_prompt<'a, const N: usize>(
     template: &Template,
     variables: [(&'a str, &'a str); N],
 ) -> String {
     template.render(variables)
         .unwrap_or_else(|err| panic!("review prompt template must render: {err}"))
 }

 pub fn user_facing_hint(target: &ReviewTarget) -> String {
     match target {
         ReviewTarget::UncommittedChanges => "current changes".to_string(),
         ReviewTarget::BaseBranch { branch } => format!("changes against '{branch}'"),
         ReviewTarget::Commit { sha, title } => {
             let short_sha: String = sha.chars().take(7).collect();
             if let Some(title) = title {
                 format!("commit {short_sha}: {title}")
             } else {
                 format!("commit {short_sha}")
             }
         }
         ReviewTarget::Custom { instructions } => instructions.trim().to_string(),
     }
 }

 impl From<ResolvedReviewRequest> for ReviewRequest {
     fn from(resolved: ResolvedReviewRequest) -> Self {
         ReviewRequest {
             target: resolved.target,
             user_facing_hint: Some(resolved.user_facing_hint),
         }
     }
 }
 ```

 ---

 ## 22.4 审查相关协议类型完整定义

 ```rust
 // === protocol/src/protocol.rs 审查相关类型 ===

 // 操作
 pub enum Op {
     Review { review_request: ReviewRequest },
 }
 // Op::Review => "review"

 // 交付方式
 #[serde(rename_all = "snake_case")]
 pub enum ReviewDelivery { Inline, Detached }

 // 审查目标
 #[serde(tag = "type", rename_all = "camelCase")]
 #[ts(tag = "type")]
 pub enum ReviewTarget {
     UncommittedChanges,
     #[serde(rename_all = "camelCase")]
     BaseBranch { branch: String },
     #[serde(rename_all = "camelCase")]
     Commit { sha: String, title: Option<String> },
     #[serde(rename_all = "camelCase")]
     Custom { instructions: String },
 }

 // 审查请求
 pub struct ReviewRequest {
     pub target: ReviewTarget,
     #[serde(skip_serializing_if = "Option::is_none")]
     #[ts(optional)]
     pub user_facing_hint: Option<String>,
 }

 // 审查输出
 pub struct ReviewOutputEvent {
     pub findings: Vec<ReviewFinding>,
     pub overall_correctness: String,
     pub overall_explanation: String,
     pub overall_confidence_score: f32,
 }
 // + Default impl

 // 单条发现
 pub struct ReviewFinding {
     pub title: String,
     pub body: String,
     pub confidence_score: f32,
     pub priority: i32,
     pub code_location: ReviewCodeLocation,
 }

 // 代码位置
 pub struct ReviewCodeLocation {
     pub absolute_file_path: PathBuf,
     pub line_range: ReviewLineRange,
 }

 // 行范围
 pub struct ReviewLineRange {
     pub start: u32,
     pub end: u32,
 }

 // 事件
 pub enum EventMsg {
     EnteredReviewMode(ReviewRequest),
     ExitedReviewMode(ExitedReviewModeEvent),
 }

 pub struct ExitedReviewModeEvent {
     pub review_output: Option<ReviewOutputEvent>,
 }

 // 子代理来源
 pub enum SubAgentSource {
     Review, // => "review"
 }

 // 不可引导 turn
 pub enum NonSteerableTurnKind {
     Review,
 }

 // Guardian 相关
 pub enum ApprovalsReviewer { User, AutoReview }
 pub enum ReviewDecision {
     Approved,
     ApprovedExecpolicyAmendment { .. },
     ApprovedForSession,
     NetworkPolicyAmendment { .. },
     Abort,
 }
 ```

 ---

 > **一句话回顾**：完整源码清单提供了 4 个核心文件的简化完整源码——`tasks/review.rs`（ReviewTask + start/process/parse/exit + 辅助函数）、`review_format.rs`（format_location + format_findings_block + render_output_text）、`review_prompts.rs`（ResolvedReviewRequest + resolve/prompt/hint + From）、以及协议层所有审查类型定义。

 ---

 ---

 # 第二十三部分 · 新手阅读指南

 本部分为完全的新手提供一条从零到理解 `/review` 的阅读路径。

 ---

 ## 23.1 推荐阅读顺序

 ### 第一阶段：建立直觉（30 分钟）

 1. **读 §1.1** — 理解 `/review` 是什么、解决什么问题
 2. **读 §1.3** — 60 秒速览，建立端到端直觉
 3. **读 §1.4 术语表** — 记住核心类型名
 4. **读 §1.5 文件地图** — 知道代码在哪里

 ### 第二阶段：理解架构（1 小时）

 5. **读 §1.2 架构图** — 理解 9 层关系
 6. **读 §7.1 状态机** — 理解状态转换
 7. **读 §7.2 数据流图** — 理解数据变换
 8. **读 §7.3 事件过滤图** — 理解事件处理

 ### 第三阶段：深入代码（2 小时）

 9. **读 §2.1-2.9 层参考手册** — 逐层理解
 10. **读 §6.1 tasks/review.rs 注解** — 核心文件逐行
 11. **读 §6.2 review_prompts.rs 注解** — prompt 生成
 12. **读 §6.3 review_format.rs 注解** — 格式化
 13. **读 §10 session/review.rs 注解** — 上下文构建
 14. **读 §13 TUI 注解** — UI 处理
 15. **读 §21 handlers.rs 注解** — 入口分发

 ### 第四阶段：理解场景（1 小时）

 16. **读 §3.1 正常 /review** — 完整链路
 17. **读 §3.4 中断场景** — 错误处理
 18. **读 §3.5 /autoreview** — 区分概念
 19. **读 §3.6 Guardian 审查** — 关联对比
 20. **读 §18 扩展场景** — 边界案例

 ### 第五阶段：理解设计（30 分钟）

 21. **读 §16 设计决策** — 为什么这样设计
 22. **读 §17 常见模式** — Rust 惯用法
 23. **读 §11 review_prompt.md 详解** — 审查准则

 ### 第六阶段：验证理解（30 分钟）

 24. **读 §8 测试代码走读** — 验证理解
 25. **读 §14 完整测试注解** — 深入测试
 26. **读 §9 调试指南** — 排障能力
 27. **读 §5.4 FAQ** — 自测

 ---

 ## 23.2 关键概念检查清单

 读完文档后，你应该能回答以下问题。如果某个问题答不上来，回去复习对应章节。

 ### 基础概念

 - [ ] `/review` 和 `/autoreview` 有什么区别？（§1.1.3, §3.5）
 - [ ] `/review` 和 Guardian 审查有什么区别？（§1.1.3, §3.6）
 - [ ] `ReviewTarget` 有哪几种变体？（§1.4, §2.2.2）
 - [ ] `ReviewOutputEvent` 包含哪些字段？（§1.4, §2.2.2）
 - [ ] `EnteredReviewMode` 和 `ExitedReviewMode` 事件分别在何处 emit？（§2.3, §2.4）

 ### 流程理解

 - [ ] 一次 `/review` 经过哪些阶段？（§3.1.3）
 - [ ] `resolve_review_request` 做了什么？（§2.5.4）
 - [ ] `spawn_review_thread` 构建了什么样的 TurnContext？（§2.3.4, §10）
 - [ ] `ReviewTask::run` 的三步流程是什么？（§2.4.4）
 - [ ] `process_review_events` 抑制了哪些事件？为什么？（§4.3）
 - [ ] `parse_review_output_event` 的三级降级是什么？（§2.4.4, §4.5.2）
 - [ ] `exit_review_mode` 记录了哪些消息？用什么 ID？（§2.4.4, §4.6）

 ### 隔离机制

 - [ ] 审查子代理用哪个模型？如何确定？（§4.1.1）
 - [ ] 审查子代理禁用了哪些 feature？（§4.1.2）
 - [ ] 审查子代理的审批策略是什么？（§4.2.2）
 - [ ] 审查子代理携带主会话历史吗？（§4.2.3）
 - [ ] 审查子代理有 developer/user 指令吗？（§4.2.5）
 - [ ] 为什么 `TurnContext.features` 用父的？（§16.11）

 ### 错误处理

 - [ ] resolve 失败会怎样？（§3.1.6 分支1）
 - [ ] 子代理返回非 JSON 会怎样？（§3.1.6 分支3, §4.5.2）
 - [ ] 审查被取消会怎样？（§3.4）
 - [ ] channel 异常关闭会怎样？（§3.4.5 分支1）
 - [ ] web_search_mode.set 失败在 task 层和 session 层分别怎样？（§4.1.3）

 ### 持久化

 - [ ] 审查结果如何持久化？（§4.6）
 - [ ] `ensure_rollout_materialized` 为什么在 emit 之后？（§4.6.1, §16.6）
 - [ ] 后续 turn 如何引用审查结果？（§4.6.3）
 - [ ] rollout 中审查记录用什么 ID？（§4.6.2）

 ### TUI

 - [ ] `is_review_mode` 影响哪些 UI 行为？（§13.9）
 - [ ] `pre_review_token_info` 的类型是什么？为什么是嵌套 Option？（§13.1）
 - [ ] 审查模式下用户消息如何处理？（§13.9.1）
 - [ ] replay 时审查模式如何恢复？（§13.8）

 ### 测试

 - [ ] 有哪些集成测试？各验证什么？（§4.7.2, §8）
 - [ ] 测试用什么工具链？（§8.3.2）
 - [ ] `wait_for_event` 为什么比 `sleep` 好？（§8.3.3）
 - [ ] 如何验证审查结果被持久化？（§8.2.1）

 ---

 ## 23.3 代码导航技巧

 ### 技巧 1：从命令入口开始追踪

 ```
 rg "SlashCommand::Review" tui/src/
   → 找到命令解析
   → 追踪 Op::Review 的构造
   → 追踪 Op::Review 的提交
 ```

 ### 技巧 2：从 Op 分发开始追踪

 ```
 rg "Op::Review" core/src/
   → 找到 handlers.rs 中的分发
   → 追踪 review() 函数
   → 追踪 spawn_review_thread()
 ```

 ### 技巧 3：从事件开始追踪

 ```
 rg "EnteredReviewMode" --type rust
   → 找到 emit 位置（session/review.rs）
   → 找到消费位置（tui/chatwidget.rs）
   → 理解事件流
 ```

 ### 技巧 4：从类型定义开始追踪

 ```
 rg "struct ReviewOutputEvent" protocol/src/
   → 找到类型定义
   → rg "ReviewOutputEvent" --type rust
   → 找到所有使用位置
 ```

 ### 技巧 5：从测试开始追踪

 ```
 rg "review" core/tests/suite/
   → 找到测试文件
   → 读测试理解预期行为
   → 追踪测试中的函数调用
 ```

 ### 技巧 6：从 prompt 开始追踪

 ```
 rg "REVIEW_PROMPT" core/src/
   → 找到常量定义（client_common.rs）
   → 找到使用位置（tasks/review.rs）
   → 读 review_prompt.md 理解审查准则
 ```

 ---

 ## 23.4 常见困惑解答

 ### 困惑 1："为什么有两个 review 概念？"

 **解答：** 代码库中有两个"review"：
 1. `/review` 斜杠命令 — 审查代码改动（§1.1.3 概念A）
 2. Guardian 审查 — 审查操作是否安全（§1.1.3 概念B）

 它们是完全不同的机制，共享的仅是"review"这个名字。`/autoreview` 属于 Guardian 体系，不是 `/review` 的变体。

 ### 困惑 2："为什么审查子代理不携带主会话历史？"

 **解答：** 设计意图是审查视角独立（§4.2.3, §16.8）：
 - 避免主会话上下文影响审查
 - 节约 token
 - 确保可复现性
 - 审查子代理只收到审查 prompt 和 REVIEW_PROMPT

 ### 困惑 3："为什么有些事件被抑制？"

 **解答：** 审查流程有意用结构化输出替代流式输出（§4.3, §16.2）：
 - `AgentMessageDelta` / `AgentMessageContentDelta`：流式增量，审查不需要
 - `ItemCompleted(AgentMessage)`：转发会触发 legacy AgentMessage
 - `AgentMessage` 被暂存，只从 `TurnComplete.last_agent_message` 解析

 ### 困惑 4："为什么 web_search_mode.set 在两处行为不同？"

 **解答：** 不同容错策略（§4.1.3, §16.7）：
 - task 层 panic：设计假设必须成功（"by construction must support Disabled"）
 - session 层 warn + 保持原值：可能受 ConfigRequirements 约束

 ### 困惑 5："TurnContext.features 为什么用父的而不是裁剪后的？"

 **解答：** 不同用途（§16.11）：
 - `TurnContext.features`：用于 turn 执行期间的 feature 检查
 - `per_turn_config.features`：用于子代理配置
 - `review_features`：仅用于 ToolsConfig 构建
 - task 层会再次裁剪，所以用父的不影响最终行为

 ### 困惑 6："parse_review_output_event 为什么有三级降级？"

 **解答：** 模型不总是按 schema 输出（§4.5.2, §16.4）：
 - 策略 1：整体 JSON（理想情况）
 - 策略 2：子串 JSON（JSON 嵌在文本中）
 - 策略 3：纯文本兜底（完全不是 JSON）
 - 设计哲学：优雅降级，宁可不完美也不要崩溃

 ### 困惑 7："exit_review_mode 为什么用固定 ID？"

 **解答：** 可识别性 + 幂等性（§16.5）：
 - 固定 ID 让后续逻辑识别审查记录
 - 如果 exit_review_mode 被多次调用（竞态），固定 ID 可用于去重
 - 调试时搜索固定 ID 可快速定位

 ### 困惑 8："ensure_rollout_materialized 为什么在 emit 之后？"

 **解答：** 用户体验优先（§4.6.1, §16.6）：
 - 先让用户看到结果（emit），再做磁盘 I/O
 - 磁盘 I/O 可能较慢（文件创建 + git 元数据收集）
 - 延迟到 emit 后避免阻塞用户看到结果

 ---

 ## 23.5 学习路径建议

 ### 路径 A：先理解流程再读代码

 1. 读 §1（架构总览）建立直觉
 2. 读 §3（生命周期追踪）理解流程
 3. 读 §2（层参考手册）理解各层
 4. 读 §6（逐行注解）深入代码
 5. 读 §8（测试走读）验证理解

 ### 路径 B：先读代码再理解设计

 1. 读 §22（完整源码清单）看代码
 2. 读 §6（逐行注解）理解每行
 3. 读 §16（设计决策）理解为什么
 4. 读 §3（生命周期追踪）理解流程
 5. 读 §8（测试走读）验证理解

 ### 路径 C：问题驱动学习

 1. 从 §5.4 FAQ 开始，找到感兴趣的问题
 2. 跟随 FAQ 中的章节引用深入
 3. 读 §9（调试指南）理解排障
 4. 读 §18（扩展场景）理解边界案例
 5. 读 §8（测试走读）验证理解

 ---

 > **一句话回顾**：新手阅读指南提供了 6 阶段阅读路径（直觉→架构→代码→场景→设计→验证）、30 道概念检查清单、6 个代码导航技巧、8 个常见困惑解答、3 条学习路径建议，帮助你从零到深入理解 `/review` 流程。

 ---

 # 第二十四部分 · 完整函数参考

 本部分列出审查相关的所有函数，按模块分组，含签名、用途和文档章节引用。

 ---

 ## 24.1 tui/src/slash_command.rs

 ```rust
 // 命令解析
 impl SlashCommand {
     pub fn from_str(s: &str) -> Result<SlashCommand, ...>;
     pub fn command(&self) -> &'static str;
     pub fn description(&self) -> &'static str;
     pub fn supports_inline_args(&self) -> bool;
 }
 ```

 | 函数 | 用途 | 文档 |
 |------|------|------|
 | `from_str` | 从字符串解析斜杠命令 | §2.1.3 |
 | `command` | 返回命令字符串（如 "review"） | §2.1.3 |
 | `description` | 返回描述文本 | §2.1.3 |
 | `supports_inline_args` | 是否支持 inline 参数 | §2.1.3, §2.1.5 |

 ---

 ## 24.2 tui/src/auto_review_denials.rs

 ```rust
 impl RecentAutoReviewDenials {
     pub(crate) fn push(&mut self, event: GuardianAssessmentEvent);
     pub(crate) fn is_empty(&self) -> bool;
     pub(crate) fn entries(&self) -> impl Iterator<Item = &GuardianAssessmentEvent>;
     pub(crate) fn take(&mut self, id: &str) -> Option<GuardianAssessmentEvent>;
 }

 pub(crate) fn action_summary(action: &GuardianAssessmentAction) -> String;
 ```

 | 函数 | 用途 | 文档 |
 |------|------|------|
 | `push` | 添加拒绝记录（去重 + truncate 10） | §2.7.4, §3.5.4 |
 | `is_empty` | 是否为空 | §3.5.6 |
 | `entries` | 迭代记录 | §3.5.5 |
 | `take` | 按 id 取出并移除 | §3.5.4, §3.5.6 |
 | `action_summary` | 格式化操作摘要 | §3.5.5 |

 ---

 ## 24.3 tui/src/chatwidget.rs（审查相关）

 ```rust
 // 进入审查模式
 fn enter_review_mode_with_hint(&mut self, hint: String, from_replay: bool);

 // 退出审查模式
 fn exit_review_mode_after_item(&mut self);

 // 恢复 token info
 fn restore_pre_review_token_info(&mut self);

 // 事件处理（#[cfg(test)]）
 #[cfg(test)]
 fn on_entered_review_mode(&mut self, review: ReviewRequest, from_replay: bool);

 #[cfg(test)]
 fn on_exited_review_mode(&mut self, review: ExitedReviewModeEvent);

 // Guardian 相关
 fn is_guardian_review(&self) -> bool;
 ```

 | 函数 | 用途 | 文档 |
 |------|------|------|
 | `enter_review_mode_with_hint` | 进入审查模式（保存 token + banner） | §13.2 |
 | `exit_review_mode_after_item` | 退出审查模式（flush + 恢复 + banner） | §13.3 |
 | `restore_pre_review_token_info` | 恢复审查前 token info | §13.6 |
 | `on_entered_review_mode` | 处理 EnteredReviewMode 事件 | §13.4 |
 | `on_exited_review_mode` | 处理 ExitedReviewMode 事件（渲染结果） | §13.5 |
 | `is_guardian_review` | 判断是否为 Guardian 审查 | §3.6.8 |

 ---

 ## 24.4 core/src/session/handlers.rs

 ```rust
 pub async fn review(sess: &Arc<Session>, sub_id: String, review_request: ReviewRequest);
 ```

 | 函数 | 用途 | 文档 |
 |------|------|------|
 | `review` | 审查入口（resolve + spawn / emit Error） | §2.3.3, §21.1 |

 ---

 ## 24.5 core/src/session/review.rs

 ```rust
 pub(super) async fn spawn_review_thread(
     sess: Arc<Session>,
     parent_turn_context: Arc<TurnContext>,
     sub_id: String,
     resolved: ResolvedReviewRequest,
 );
 ```

 | 函数 | 用途 | 文档 |
 |------|------|------|
 | `spawn_review_thread` | 构建隔离 TurnContext + spawn ReviewTask + emit EnteredReviewMode | §2.3.4, §10 |

 ---

 ## 24.6 core/src/tasks/review.rs

 ```rust
 // ReviewTask
 impl ReviewTask {
     pub(crate) fn new() -> Self;
 }
 impl SessionTask for ReviewTask {
     fn kind(&self) -> TaskKind;
     fn span_name(&self) -> &'static str;
     async fn run(...) -> Option<String>;
     async fn abort(...);
 }

 // 内部函数
 async fn start_review_conversation(...) -> Option<Receiver<Event>>;
 async fn process_review_events(...) -> Option<ReviewOutputEvent>;
 fn parse_review_output_event(text: &str) -> ReviewOutputEvent;
 pub(crate) async fn exit_review_mode(...);
 fn render_review_exit_success(results: &str) -> String;
 fn normalize_review_template_line_endings(template: &str) -> Cow<'_, str>;
 ```

 | 函数 | 用途 | 文档 |
 |------|------|------|
 | `ReviewTask::new` | 创建审查任务实例 | §2.4.2 |
 | `ReviewTask::run` | 主执行流程 | §2.4.4, §6.1.3 |
 | `ReviewTask::abort` | 中断处理 | §2.4.4, §6.1.3 |
 | `start_review_conversation` | 起子代理 | §2.4.4, §6.1.4 |
 | `process_review_events` | 消费事件 | §2.4.4, §6.1.5 |
 | `parse_review_output_event` | 解析输出（三级降级） | §2.4.4, §6.1.6 |
 | `exit_review_mode` | 退出回灌 | §2.4.4, §6.1.7 |
 | `render_review_exit_success` | 渲染成功模板 | §6.1.8 |
 | `normalize_review_template_line_endings` | CRLF 规范化 | §6.1.8 |

 ---

 ## 24.7 core/src/review_prompts.rs

 ```rust
 pub fn resolve_review_request(
     request: ReviewRequest,
     cwd: &AbsolutePathBuf,
 ) -> anyhow::Result<ResolvedReviewRequest>;

 pub fn review_prompt(target: &ReviewTarget, cwd: &AbsolutePathBuf) -> anyhow::Result<String>;

 pub fn user_facing_hint(target: &ReviewTarget) -> String;

 fn render_review_prompt<'a, const N: usize>(
     template: &Template,
     variables: [(&'a str, &'a str); N],
 ) -> String;

 impl From<ResolvedReviewRequest> for ReviewRequest;
 ```

 | 函数 | 用途 | 文档 |
 |------|------|------|
 | `resolve_review_request` | 解析审查请求 | §2.5.3, §6.2.2 |
 | `review_prompt` | 按 target 生成 prompt | §2.5.4, §6.2.3 |
 | `user_facing_hint` | 生成 UI 提示 | §2.5.4, §6.2.4 |
 | `render_review_prompt` | 渲染模板 | §6.2.4 |
 | `From` | ResolvedReviewRequest → ReviewRequest | §6.2.5 |

 ---

 ## 24.8 core/src/review_format.rs

 ```rust
 pub fn format_review_findings_block(
     findings: &[ReviewFinding],
     selection: Option<&[bool]>,
 ) -> String;

 pub fn render_review_output_text(output: &ReviewOutputEvent) -> String;

 fn format_location(item: &ReviewFinding) -> String;
 ```

 | 函数 | 用途 | 文档 |
 |------|------|------|
 | `format_review_findings_block` | 格式化 findings 列表 | §2.6.4, §6.3.2 |
 | `render_review_output_text` | 渲染审查摘要 | §2.6.4, §6.3.3 |
 | `format_location` | 格式化位置 | §6.3.1 |

 ---

 ## 24.9 core/src/client_common.rs（审查相关常量）

 ```rust
 pub const REVIEW_PROMPT: &str = include_str!("../review_prompt.md");
 pub const REVIEW_EXIT_SUCCESS_TMPL: &str = include_str!("../templates/review/exit_success.xml");
 pub const REVIEW_EXIT_INTERRUPTED_TMPL: &str = include_str!("../templates/review/exit_interrupted.xml");
 ```

 | 常量 | 用途 | 文档 |
 |------|------|------|
 | `REVIEW_PROMPT` | 审查者系统 prompt | §2.5.2, §11 |
 | `REVIEW_EXIT_SUCCESS_TMPL` | 成功退出模板 | §2.5.6 |
 | `REVIEW_EXIT_INTERRUPTED_TMPL` | 中断退出模板 | §2.5.6 |

 ---

 ## 24.10 函数调用关系图

 ```
 SlashCommand::from_str()
   └─> (TUI 分发)
         └─> session.submit(Op::Review)
               └─> submission_loop
                     └─> handlers::review()
                           ├─> sess.new_default_turn_with_sub_id()
                           ├─> sess.maybe_emit_unknown_model_warning_for_turn()
                           ├─> sess.refresh_mcp_servers_if_requested()
                           └─> resolve_review_request()
                                 ├─> review_prompt()
                                 │     ├─> merge_base_with_head() [BaseBranch]
                                 │     └─> render_review_prompt()
                                 └─> user_facing_hint()
                                       └─> (返回 ResolvedReviewRequest)
                             └─> spawn_review_thread()
                                   ├─> models_manager.get_model_info()
                                   ├─> features.disable() [WebSearch]
                                   ├─> ToolsConfig::new().with_*()
                                   ├─> (构建 per_turn_config)
                                   ├─> (构建 TurnContext)
                                   ├─> sess.spawn_task(ReviewTask::new())
                                   └─> sess.send_event(EnteredReviewMode)
                                         └─> ReviewTask::run()
                                               ├─> session_telemetry.counter()
                                               └─> start_review_conversation()
                                                     ├─> web_search_mode.set(Disabled)
                                                     ├─> features.disable() [SpawnCsv, Collab]
                                                     ├─> base_instructions = REVIEW_PROMPT
                                                     ├─> approval_policy = Never
                                                     ├─> model = review_model or 主模型
                                                     └─> run_agere_thread_one_shot()
                                                           └─> (返回 Receiver<Event>)
                                               └─> process_review_events()
                                                     ├─> (AgentMessage 暂存)
                                                     ├─> (Delta/ItemCompleted 抑制)
                                                     ├─> TurnComplete → parse_review_output_event()
                                                     │     ├─> serde_json::from_str() [策略1]
                                                     │     ├─> find{}/rfind{}/from_str() [策略2]
                                                     │     └─> ReviewOutputEvent::default() [策略3]
                                                     └─> TurnAborted → None
                                               └─> exit_review_mode()
                                                     ├─> format_review_findings_block()
                                                     ├─> render_review_output_text()
                                                     ├─> render_review_exit_success()
                                                     ├─> normalize_review_template_line_endings()
                                                     ├─> record_conversation_items()
                                                     ├─> send_event(ExitedReviewMode)
                                                     ├─> record_response_item_and_emit_turn_item()
                                                     └─> ensure_rollout_materialized()
 ```

 ---

 > **一句话回顾**：完整函数参考列出了 10 个模块的所有审查相关函数（含签名、用途、文档引用），并提供了完整的函数调用关系图，从 `SlashCommand::from_str` 到 `ensure_rollout_materialized` 的完整调用链一目了然。

 ---

 ---

 # 第二十五部分 · 完整 JSON 样例库

 本部分提供审查流程中所有 JSON 数据的完整样例，方便对照和调试。

 ---

 ## 25.1 ReviewTarget JSON 样例

 ### UncommittedChanges

 ```json
 {
   "type": "uncommittedChanges"
 }
 ```

 **说明：** 无额外字段。审查 working tree 中的所有改动（staged + unstaged + untracked）。

 ### BaseBranch

 ```json
 {
   "type": "baseBranch",
   "branch": "main"
 }
 ```

 **说明：** `branch` 是 base 分支名。审查当前分支相对该分支的 diff。

 ```json
 {
   "type": "baseBranch",
   "branch": "develop"
 }
 ```

 ### Commit（无 title）

 ```json
 {
   "type": "commit",
   "sha": "abc123def456"
 }
 ```

 **说明：** `sha` 是 commit SHA。无 `title` 字段（或为 null）。

 ### Commit（有 title）

 ```json
 {
   "type": "commit",
   "sha": "abc123def456",
   "title": "Fix buffer overflow in parser"
 }
 ```

 **说明：** `title` 是可选的人类可读标签（如 commit subject）。

 ### Custom

 ```json
 {
   "type": "custom",
   "instructions": "Focus on security and error handling"
 }
 ```

 **说明：** `instructions` 是用户自定义指令。不能为空（trim 后）。

 ---

 ## 25.2 ReviewRequest JSON 样例

 ### 无 hint

 ```json
 {
   "target": {
     "type": "uncommittedChanges"
   }
 }
 ```

 **说明：** `user_facing_hint` 为 None 时省略（`skip_serializing_if = "Option::is_none"`）。

 ### 有 hint

 ```json
 {
   "target": {
     "type": "baseBranch",
     "branch": "main"
   },
   "user_facing_hint": "changes against 'main'"
 }
 ```

 **说明：** `user_facing_hint` 存在时包含在 JSON 中。

 ### Custom + hint

 ```json
 {
   "target": {
     "type": "custom",
     "instructions": "Check error handling"
   },
   "user_facing_hint": "Check error handling"
 }
 ```

 ---

 ## 25.3 ReviewOutputEvent JSON 样例

 ### 完整审查结果（多个 findings）

 ```json
 {
   "findings": [
     {
       "title": "[P0] SQL injection in query builder",
       "body": "User input is concatenated directly into SQL strings without parameterization.",
       "confidence_score": 0.98,
       "priority": 0,
       "code_location": {
         "absolute_file_path": "/home/user/repo/src/db.rs",
         "line_range": { "start": 15, "end": 22 }
       }
     },
     {
       "title": "[P1] Race condition in cache",
       "body": "Non-atomic read-then-write pattern causes stale data under concurrent access.",
       "confidence_score": 0.85,
       "priority": 1,
       "code_location": {
         "absolute_file_path": "/home/user/repo/src/cache.rs",
         "line_range": { "start": 88, "end": 95 }
       }
     }
   ],
   "overall_correctness": "patch is incorrect",
   "overall_explanation": "SQL injection vulnerability must be fixed before merge.",
   "overall_confidence_score": 0.92
 }
 ```

 ### 无 findings（代码正确）

 ```json
 {
   "findings": [],
   "overall_correctness": "patch is correct",
   "overall_explanation": "Changes are well-structured with no bugs.",
   "overall_confidence_score": 0.9
 }
 ```

 ### 无 findings（代码不正确但无具体 finding）

 ```json
 {
   "findings": [],
   "overall_correctness": "patch is incorrect",
   "overall_explanation": "The overall approach has issues but no discrete actionable bug.",
   "overall_confidence_score": 0.6
 }
 ```

 ### 纯文本兜底（Default）

 ```json
 {
   "findings": [],
   "overall_correctness": "",
   "overall_explanation": "The code looks good overall, no major issues found.",
   "overall_confidence_score": 0.0
 }
 ```

 **说明：** 这是 `parse_review_output_event` 策略 3 的兜底结果。纯文本放进 `overall_explanation`，其余为默认值。

 ### 全空（Default）

 ```json
 {
   "findings": [],
   "overall_correctness": "",
   "overall_explanation": "",
   "overall_confidence_score": 0.0
 }
 ```

 **说明：** `ReviewOutputEvent::default()`。会导致 "Reviewer failed to output a response." 错误。

 ---

 ## 25.4 ReviewFinding JSON 样例

 ### P0 finding

 ```json
 {
   "title": "[P0] SQL injection vulnerability in query builder",
   "body": "The `build_query` function concatenates user input directly into SQL strings without parameterization, allowing SQL injection attacks when `user_input` contains malicious SQL.",
   "confidence_score": 0.98,
   "priority": 0,
   "code_location": {
     "absolute_file_path": "/home/user/repo/src/db/query.rs",
     "line_range": { "start": 15, "end": 22 }
   }
 }
 ```

 ### P1 finding

 ```json
 {
   "title": "[P1] Race condition in cache invalidation",
   "body": "The cache invalidation uses a non-atomic read-then-write pattern, causing stale data under concurrent access when two threads invalidate simultaneously.",
   "confidence_score": 0.85,
   "priority": 1,
   "code_location": {
     "absolute_file_path": "/home/user/repo/src/cache.rs",
     "line_range": { "start": 88, "end": 95 }
   }
 }
 ```

 ### P2 finding

 ```json
 {
   "title": "[P2] Missing input validation for negative quantities",
   "body": "The `calculate_price` function doesn't validate that quantity is non-negative, producing incorrect pricing for negative inputs.",
   "confidence_score": 0.7,
   "priority": 2,
   "code_location": {
     "absolute_file_path": "/home/user/repo/src/pricing.rs",
     "line_range": { "start": 30, "end": 35 }
   }
 }
 ```

 ### P3 finding

 ```json
 {
   "title": "[P3] Inconsistent naming: camelCase in snake_case module",
   "body": "The function `getUserData` uses camelCase while the rest of the module uses snake_case.",
   "confidence_score": 0.6,
   "priority": 3,
   "code_location": {
     "absolute_file_path": "/home/user/repo/src/api.rs",
     "line_range": { "start": 12, "end": 12 }
   }
 }
 ```

 ### 单行 finding

 ```json
 {
   "title": "[P2] Unused import: std::collections::HashMap",
   "body": "The import `std::collections::HashMap` is never used in this file.",
   "confidence_score": 0.95,
   "priority": 2,
   "code_location": {
     "absolute_file_path": "/home/user/repo/src/main.rs",
     "line_range": { "start": 3, "end": 3 }
   }
 }
 ```

 ---

 ## 25.5 事件 JSON 样例

 ### EnteredReviewMode 事件

 ```json
 {
   "type": "entered_review_mode",
   "review_request": {
     "target": {
       "type": "uncommittedChanges"
     },
     "user_facing_hint": "current changes"
   }
 }
 ```

 ### ExitedReviewMode 事件（成功）

 ```json
 {
   "type": "exited_review_mode",
   "review_output": {
     "findings": [
       {
         "title": "[P1] Buffer overflow",
         "body": "...",
         "confidence_score": 0.9,
         "priority": 1,
         "code_location": {
           "absolute_file_path": "/tmp/file.rs",
           "line_range": { "start": 10, "end": 20 }
         }
       }
     ],
     "overall_correctness": "patch is incorrect",
     "overall_explanation": "Buffer overflow detected.",
     "overall_confidence_score": 0.85
   }
 }
 ```

 ### ExitedReviewMode 事件（中断）

 ```json
 {
   "type": "exited_review_mode",
   "review_output": null
 }
 ```

 ---

 ## 25.6 API 请求 JSON 样例

 ### 完整 API 请求（简化）

 ```json
 {
   "model": "gpt-5.4",
   "instructions": "# Review guidelines:\n\nYou are acting as a reviewer for a proposed code change made by another engineer.\n\n...",
   "input": [
     {
       "type": "message",
       "role": "user",
       "content": [
         {
           "type": "input_text",
           "text": "Review the current code changes (staged, unstaged, and untracked files) and provide prioritized findings."
         }
       ]
     }
   ],
   "tools": [
     { "type": "function", "name": "shell", "..." : "..." },
     { "type": "function", "name": "apply_patch", "..." : "..." }
   ]
 }
 ```

 **说明：**

 - `model`：审查模型（`review_model` 或主模型）
 - `instructions`：REVIEW_PROMPT 全文（审查准则 + 输出 schema）
 - `input`：初始输入（审查 prompt 作为 user message）
   - 注意：不包含主会话历史（隔离）
 - `tools`：可用工具（web search 已禁用）

 ### BaseBranch 审查的 input

 ```json
 {
   "input": [
     {
       "type": "message",
       "role": "user",
       "content": [
         {
           "type": "input_text",
           "text": "Review the code changes against the base branch 'main'. The merge base commit for this comparison is abc123def456. Run `git diff abc123def456` to inspect the changes relative to main. Provide prioritized, actionable findings."
         }
       ]
     }
   ]
 }
 ```

 ---

 ## 25.7 rollout JSONL 样例

 ### 成功审查的 rollout 记录

 ```json
 {"timestamp":"2026-07-05T10:00:00.000Z","type":"response_item","payload":{"type":"message","role":"user","id":"review_rollout_user","content":[{"type":"input_text","text":"<user_action>\n  <context>User initiated a review task. Here's the full review output from reviewer model. User may select one or more comments to resolve.</context>\n  <action>review</action>\n  <results>\n  Buffer overflow detected.\n\nFull review comments:\n\n- [P1] Buffer overflow — /tmp/file.rs:10-20\n  The parse function doesn't check bounds.\n  </results>\n</user_action>"}]}}
 {"timestamp":"2026-07-05T10:00:01.000Z","type":"response_item","payload":{"type":"message","role":"assistant","id":"review_rollout_assistant","content":[{"type":"output_text","text":"Buffer overflow detected.\n\nFull review comments:\n\n- [P1] Buffer overflow — /tmp/file.rs:10-20\n  The parse function doesn't check bounds."}]}}
 ```

 ### 中断审查的 rollout 记录

 ```json
 {"timestamp":"2026-07-05T10:00:00.000Z","type":"response_item","payload":{"type":"message","role":"user","id":"review_rollout_user","content":[{"type":"input_text","text":"<user_action>\n  <context>User initiated a review task, but was interrupted. If user asks about this, tell them to re-initiate a review with `/review` and wait for it to complete.</context>\n  <action>review</action>\n  <results>\n  None.\n  </results>\n</user_action>"}]}}
 {"timestamp":"2026-07-05T10:00:01.000Z","type":"response_item","payload":{"type":"message","role":"assistant","id":"review_rollout_assistant","content":[{"type":"output_text","text":"Review was interrupted. Please re-run /review and wait for it to complete."}]}}
 ```

 ---

 ## 25.8 后续 turn 引用审查结果的 input 样例

 当审查完成后，后续常规 turn 的 input 会包含审查记录：

 ```json
 {
   "input": [
     {
       "type": "message",
       "role": "user",
       "content": [
         {
           "type": "input_text",
           "text": "<user_action>\n  <context>User initiated a review task...</context>\n  <action>review</action>\n  <results>\n  Buffer overflow detected...\n  </results>\n</user_action>"
         }
       ]
     },
     {
       "type": "message",
       "role": "assistant",
       "content": [
         {
           "type": "output_text",
           "text": "Buffer overflow detected...\n\nFull review comments:\n\n- [P1] Buffer overflow..."
         }
       ]
     },
     {
       "type": "message",
       "role": "user",
       "content": [
         {
           "type": "input_text",
           "text": "back to parent"
         }
       ]
     }
   ]
 }
 ```

 **说明：** 前两条是审查记录（user + assistant），最后一条是新的用户输入。Agent 可以引用审查结果。

 ---

 > **一句话回顾**：完整 JSON 样例库提供了 8 类 JSON 样例——ReviewTarget（5 变体）、ReviewRequest（3 样例）、ReviewOutputEvent（5 样例含兜底/全空）、ReviewFinding（5 个不同优先级）、事件（3 样例）、API 请求（2 样例）、rollout JSONL（成功/中断）、后续 turn 引用（含审查记录 + 新输入）。

 ---

 # 第二十六部分 · 完整 ASCII 图表集

 本部分汇集文档中所有 ASCII 图表，方便集中查阅。

 ---

 ## 26.1 全局架构图

 ```
 ┌─────────────────────────────────────────────────────────────────────┐
 │                         用户输入 /review                            │
 └──────────────────────────────┬──────────────────────────────────────┘
                                │
                     ┌──────────▼──────────┐
                     │  TUI: SlashCommand   │  tui/src/slash_command.rs
                     │  ::Review 解析        │
                     └──────────┬──────────┘
                                │ Op::Review { review_request }
                     ┌──────────▼──────────┐
                     │  Session Handler     │  core/src/session/handlers.rs
                     │  review()            │
                     └──────────┬──────────┘
                                │ resolve_review_request()
                     ┌──────────▼──────────┐
                     │  Review Prompts      │  core/src/review_prompts.rs
                     │  生成审查 prompt      │
                     └──────────┬──────────┘
                                │ ResolvedReviewRequest
                     ┌──────────▼──────────┐
                     │  Session::review     │  core/src/session/review.rs
                     │  spawn_review_thread │  构建 TurnContext (隔离)
                     └──────────┬──────────┘
                                │ spawn_task(ReviewTask)
                                │ emit EnteredReviewMode
                     ┌──────────▼──────────┐
                     │  ReviewTask::run     │  core/src/tasks/review.rs
                     │  ├ start_review_     │
                     │  │  conversation()   │──┐ run_agere_thread_one_shot
                     │  ├ process_review_   │  │ (子代理: reviewer model)
                     │  │  events()         │◄─┘ Event 流
                     │  └ exit_review_mode()│
                     └──────────┬──────────┘
                                │
                     ┌──────────▼──────────┐
                     │  parse_review_       │  解析 JSON → ReviewOutputEvent
                     │  output_event()      │  失败兜底 → 纯文本
                     └──────────┬──────────┘
                                │
                     ┌──────────▼──────────┐
                     │  exit_review_mode()  │  emit ExitedReviewMode
                     │  + record items      │  record user/assistant msg
                     │  + format findings   │  ensure_rollout_materialized
                     └──────────┬──────────┘
                                │
                     ┌──────────▼──────────┐
                     │  TUI: ChatWidget     │  tui/src/chatwidget.rs
                     │  on_exited_review_   │  渲染 findings / 恢复状态
                     │  mode()              │
                     └─────────────────────┘
 ```

 ---

 ## 26.2 审查流程状态机

 ```
                          ┌─────────────────┐
                          │     IDLE        │
                          └────────┬────────┘
                                   │ Op::Review
                                   ▼
                          ┌─────────────────┐
                          │  RESOLVING      │
                          └────────┬────────┘
                      ┌────────────┼────────────┐
                      │            │            │
                   Ok           Err          (其他)
                      │            │            │
                      ▼            ▼            ▼
              ┌──────────┐  ┌─────────┐  ┌─────────┐
              │SPAWNING  │  │ ERROR   │  │  IDLE   │
              └────┬─────┘  └─────────┘  └─────────┘
                   │ emit EnteredReviewMode
                   ▼
              ┌─────────────────┐
              │  REVIEWING      │◄──── Ctrl+C
              └────────┬────────┘
                       │
              ┌────────┼────────┐
              │        │        │
        TurnComplete  Abort  channel close
              │        │        │
              ▼        ▼        ▼
         ┌────────┐ ┌────────┐ ┌────────┐
         │PARSING │ │ABORTING│ │ (None) │
         └───┬────┘ └───┬────┘ └───┬────┘
             │          │          │
             └──────────┴──────────┘
                        │
                        ▼
                 ┌──────────────┐
                 │  EXITING     │
                 └──────┬───────┘
                        │
                        ▼
                 ┌──────────────┐
                 │  IDLE        │
                 └──────────────┘
 ```

 ---

 ## 26.3 TUI 状态机

 ```
   ┌──────────────┐     EnteredReviewMode     ┌──────────────┐
   │  NORMAL      │ ────────────────────────► │  REVIEW_MODE │
   │ is_review_   │                            │ is_review_   │
   │ mode = false │ ◄──────────────────────── │ mode = true  │
   └──────────────┘     ExitedReviewMode       └──────┬───────┘
        ▲                                            │
        │                                    ExitedReviewMode
        │                                            │
        │              ┌──────────────┐              │
        │              │  RENDERING   │              │
        └──────────────┤ 渲染 findings │◄─────────────┘
                        │ / explanation │
                        │ / error       │
                        └──────────────┘
 ```

 ---

 ## 26.4 事件过滤管道

 ```
 子代理事件流                    process_review_events
 ════════════════              ┌─────────────────┐
 │ Delta events  │ ── Drop ──► │  事件过滤器      │
 │ ItemCompleted │ ── Drop ──► │                 │
 │ AgentMessage  │ ── Buffer ─►│                 │
 │ TurnComplete  │ ── Terminal │                 │
 │ TurnAborted   │ ── Terminal │                 │
 │ 其他          │ ── Forward ─►│                 │
 ════════════════              └────────┬────────┘
                                        │
                                        ▼
                               ┌─────────────────┐
                               │  主会话事件流    │
                               │  ExitedReviewMode│
                               │  + 其他转发事件  │
                               └─────────────────┘
 ```

 ---

 ## 26.5 数据流变换链

 ```
 "/review main"
       │
       ▼ SlashCommand::from_str
 SlashCommand::Review, args="main"
       │
       ▼ 构造 ReviewRequest
 ReviewRequest { BaseBranch { "main" } }
       │
       ▼ Op::Review
 Op::Review { review_request }
       │
       ▼ resolve_review_request
 ResolvedReviewRequest { prompt, hint }
       │
       ▼ spawn_review_thread
 Vec<UserInput> [Text { prompt }]
       │
       ▼ run_agere_thread_one_shot
 API 请求 JSON { model, instructions, input }
       │
       ▼ 子代理执行
 Event 流
       │
       ▼ process_review_events
 Option<String> (last_agent_message)
       │
       ▼ parse_review_output_event
 ReviewOutputEvent
       │
       ▼ exit_review_mode
 (user_msg, asst_msg) + ExitedReviewMode
       │
       ▼ record + persist
 Conversation Items + Rollout JSONL
 ```

 ---

 ## 26.6 函数调用关系图

 ```
 SlashCommand::from_str
   └─> session.submit(Op::Review)
         └─> handlers::review()
               ├─> resolve_review_request()
               │     └─> review_prompt() / user_facing_hint()
               └─> spawn_review_thread()
                     ├─> ToolsConfig::new()
                     ├─> spawn_task(ReviewTask)
                     └─> emit EnteredReviewMode
                           └─> ReviewTask::run()
                                 ├─> start_review_conversation()
                                 │     └─> run_agere_thread_one_shot()
                                 ├─> process_review_events()
                                 │     └─> parse_review_output_event()
                                 └─> exit_review_mode()
                                       ├─> format_review_findings_block()
                                       ├─> render_review_output_text()
                                       ├─> record_conversation_items()
                                       ├─> emit ExitedReviewMode
                                       └─> ensure_rollout_materialized()
 ```

 ---

 ## 26.7 隔离机制图

 ```
 ┌─────────────────────────────────────────────────────────┐
 │                    主会话                                │
 │  ┌─────────┐  ┌──────────┐  ┌────────────┐             │
 │  │ 历史    │  │ developer│  │ user       │             │
 │  │ 记录    │  │ 指令     │  │ 指令       │             │
 │  └─────────┘  └──────────┘  └────────────┘             │
 │  ┌─────────┐  ┌──────────┐  ┌────────────┐             │
 │  │ web     │  │ 主模型   │  │ 审批策略   │             │
 │  │ search  │  │          │  │            │             │
 │  └─────────┘  └──────────┘  └────────────┘             │
 └───────────────────────┬─────────────────────────────────┘
                         │ 隔离边界
 ┌───────────────────────▼─────────────────────────────────┐
 │                  审查子代理                               │
 │  ┌─────────┐  ┌──────────┐  ┌────────────┐             │
 │  │ 无历史  │  │ 无 dev   │  │ 无 user    │             │
 │  │         │  │ 指令     │  │ 指令       │             │
 │  └─────────┘  └──────────┘  └────────────┘             │
 │  ┌─────────┐  ┌──────────┐  ┌────────────┐             │
 │  │ web     │  │ review   │  │ Never      │             │
 │  │ search  │  │ _model   │  │ (无需审批)  │             │
 │  │ 禁用    │  │ 或主模型  │  │            │             │
 │  └─────────┘  └──────────┘  └────────────┘             │
 │  ┌──────────────────────────────────────┐              │
 │  │ base_instructions = REVIEW_PROMPT    │              │
 │  └──────────────────────────────────────┘              │
 └─────────────────────────────────────────────────────────┘
 ```

 ---

 ## 26.8 三级降级解析图

 ```
 子代理输出文本
       │
       ▼
 ┌─────────────────┐
 │ 策略 1: 整体 JSON │
 │ serde_json::     │
 │ from_str(text)   │
 └────────┬────────┘
          │
     Ok?  ├─ Yes ──► 返回 ReviewOutputEvent
          │
         No
          │
          ▼
 ┌─────────────────┐
 │ 策略 2: 子串 JSON │
 │ find('{')...     │
 │ rfind('}')...    │
 │ from_str(slice)  │
 └────────┬────────┘
          │
     Ok?  ├─ Yes ──► 返回 ReviewOutputEvent
          │
         No
          │
          ▼
 ┌─────────────────┐
 │ 策略 3: 纯文本   │
 │ ReviewOutputEvent│
 │ { overall_exp:   │
 │   text, ..Default│
 │ }                │
 └────────┬────────┘
          │
          ▼
    返回兜底结果
 ```

 ---

 > **一句话回顾**：完整 ASCII 图表集汇集了 8 张图——全局架构图、审查流程状态机、TUI 状态机、事件过滤管道、数据流变换链、函数调用关系图、隔离机制图、三级降级解析图，方便集中查阅和理解。

 ---

 ---

 # 第二十七部分 · 完整测试注解（续）

 本部分对剩余的测试进行完整代码展示和注解。

 ---

 ## 27.1 `review_op_with_plain_text_emits_review_fallback` 完整注解

 ```rust
 #[cfg_attr(windows, tokio::test(flavor = "multi_thread", worker_threads = 4))]
 #[cfg_attr(not(windows), tokio::test(flavor = "multi_thread", worker_threads = 2))]
 async fn review_op_with_plain_text_emits_review_fallback() {
     skip_if_no_network!();
 ```

 **注解：** 同其他测试的属性配置。

 ```rust
     // 模拟子代理返回纯文本（非 JSON）
     let sse_raw = r#"[
         {"type":"response.output_item.done", "item":{
             "type":"message", "role":"assistant",
             "content":[{"type":"output_text","text":"just plain text"}]
         }},
         {"type":"response.completed", "response": {"id": "__ID__"}}
     ]"#;
 ```

 **注解：**

 - SSE 返回纯文本 "just plain text"（非 JSON）
 - `response.output_item.done`：助手消息完成
 - `response.completed`：响应完成

 ```rust
     let (server, _request_log) =
         start_responses_server_with_sse(sse_raw, /*expected_requests*/ 1).await;
     let agere_home = Arc::new(TempDir::new().unwrap());
     let agere = new_conversation_for_server(&server, agere_home.clone(), |_| {}).await;

     agere
         .submit(Op::Review {
             review_request: ReviewRequest {
                 target: ReviewTarget::Custom {
                     instructions: "Plain text review".to_string(),
                 },
                 user_facing_hint: None,
             },
         })
         .await
         .unwrap();
 ```

 **注解：** 提交审查请求（Custom 指令 = "Plain text review"）。

 ```rust
     let _entered = wait_for_event(&agere, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
     let closed = wait_for_event(&agere, |ev| matches!(ev, EventMsg::ExitedReviewMode(_))).await;
     let review = match closed {
         EventMsg::ExitedReviewMode(ev) => ev
             .review_output
             .expect("expected ExitedReviewMode with Some(review_output)"),
         other => panic!("expected ExitedReviewMode(..), got {other:?}"),
     };
 ```

 **注解：**

 - 等待 `EnteredReviewMode` 和 `ExitedReviewMode`
 - 从 `ExitedReviewMode` 提取 `review_output`
 - `expect("Some")`：断言有审查结果
   - 注意：即使是纯文本兜底，`review_output` 也是 `Some`（非 `None`）
   - `None` 只在中断/取消时出现

 ```rust
     // 期望结构化兜底，携带纯文本
     let expected = ReviewOutputEvent {
         overall_explanation: "just plain text".to_string(),
         ..Default::default()
     };
     assert_eq!(expected, review);
 ```

 **注解：**

 - 构造预期的 `ReviewOutputEvent`：
   - `overall_explanation: "just plain text"` — 纯文本放进 explanation
   - `..Default::default()` — 其余为默认值
     - `findings: vec![]`
     - `overall_correctness: ""`
     - `overall_confidence_score: 0.0`
 - `assert_eq!(expected, review)`：深度比较
 - 验证 `parse_review_output_event` 的策略 3（纯文本兜底）

 ```rust
     let _complete = wait_for_event(&agere, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

     let _agere_home_guard = agere_home;
     server.verify().await;
 }
 ```

 **注解：** 等待 TurnComplete，验证 mock 请求。

 ---

 ## 27.2 `review_does_not_emit_agent_message_on_structured_output` 完整注解

 ```rust
 async fn review_does_not_emit_agent_message_on_structured_output() {
     skip_if_no_network!();

     let review_json = serde_json::json!({
         "findings": [{
             "title": "Example",
             "body": "Structured review output.",
             "confidence_score": 0.5,
             "priority": 1,
             "code_location": {
                 "absolute_file_path": "/tmp/file.rs",
                 "line_range": {"start": 1, "end": 2}
             }
         }],
         "overall_correctness": "ok",
         "overall_explanation": "ok",
         "overall_confidence_score": 0.5
     }).to_string();
 ```

 **注解：**

 - 构造 `ReviewOutputEvent` JSON（1 个 finding）
 - 注意 `overall_correctness` 是 `"ok"` 而非 `"patch is correct"`
   - 代码层不校验 correctness 值，任何字符串都可

 ```rust
     let sse_template = r#"[
         {"type":"response.output_item.done", "item":{
             "type":"message", "role":"assistant",
             "content":[{"type":"output_text","text":__REVIEW__}]}},
         {"type":"response.completed", "response": {"id": "__ID__"}}
     ]"#;
     let review_json_escaped = serde_json::to_string(&review_json).unwrap();
     let sse_raw = sse_template.replace("__REVIEW__", &review_json_escaped);
     let (server, _request_log) =
         start_responses_server_with_sse(&sse_raw, /*expected_requests*/ 1).await;
     let agere_home = Arc::new(TempDir::new().unwrap());
     let agere = new_conversation_for_server(&server, agere_home.clone(), |_| {}).await;
 ```

 **注解：** 同 `review_op_emits_lifecycle_and_review_output` 的设置。

 ```rust
     agere
         .submit(Op::Review {
             review_request: ReviewRequest {
                 target: ReviewTarget::Custom {
                     instructions: "check structured".to_string(),
                 },
                 user_facing_hint: None,
             },
         })
         .await
         .unwrap();
 ```

 **注解：** 提交审查请求（Custom 指令 = "check structured"）。

 ```rust
     // 排水事件直到 TurnComplete；确保只看到最终的 AgentMessage
     let mut saw_entered = false;
     let mut saw_exited = false;
     let mut agent_messages = 0;
     wait_for_event(&agere, |event| match event {
         EventMsg::TurnComplete(_) => true,
         EventMsg::AgentMessage(_) => {
             agent_messages += 1;
             false
         }
         EventMsg::EnteredReviewMode(_) => {
             saw_entered = true;
             false
         }
         EventMsg::ExitedReviewMode(_) => {
             saw_exited = true;
             false
         }
         _ => false,
     })
     .await;
 ```

 **注解：**

 - `wait_for_event`：循环接收事件
 - `TurnComplete` → 返回 true（结束）
 - `AgentMessage` → 计数器+1，继续等待
 - `EnteredReviewMode` / `ExitedReviewMode` → 设标志，继续
 - 其他 → 继续

 ```rust
     assert_eq!(1, agent_messages, "expected exactly one AgentMessage event");
     assert!(saw_entered && saw_exited, "missing review lifecycle events");

     let _agere_home_guard = agere_home;
     server.verify().await;
 }
 ```

 **注解：**

 - `assert_eq!(1, agent_messages)`：断言恰好 1 个 AgentMessage
   - 验证暂存机制：多条 AgentMessage 只转发最后一条
   - 但因为子代理只返回一条，所以只有 1 个
 - `assert!(saw_entered && saw_exited)`：断言看到了生命周期事件

 ---

 ## 27.3 `review_uses_custom_review_model_from_config` 完整注解

 ```rust
 async fn review_uses_custom_review_model_from_config() {
     skip_if_no_network!();

     // 最小流：只有 completed 事件
     let sse_raw = r#"[
         {"type":"response.completed", "response": {"id": "__ID__"}}
     ]"#;
     let (server, request_log) =
         start_responses_server_with_sse(sse_raw, /*expected_requests*/ 1).await;
 ```

 **注解：**

 - SSE 只返回 `response.completed`（无 `output_item.done`）
 - 这意味着子代理没有产出消息
 - `last_agent_message` 为 `None`
 - `parse_review_output_event(None)` → `None`
 - `exit_review_mode(None)` → 中断模板
 - 但这个测试的目的是验证模型，不是验证输出

 ```rust
     let agere_home = Arc::new(TempDir::new().unwrap());
     // 选择不同于主模型的审查模型
     let agere = new_conversation_for_server(&server, agere_home.clone(), |cfg| {
         cfg.model = Some("gpt-4.1".to_string());
         cfg.review_model = Some("gpt-5.4".to_string());
     })
     .await;
 ```

 **注解：**

 - `cfg.model = "gpt-4.1"`：主会话模型
 - `cfg.review_model = "gpt-5.4"`：审查模型
 - 两个不同，验证审查用 review_model

 ```rust
     agere
         .submit(Op::Review {
             review_request: ReviewRequest {
                 target: ReviewTarget::Custom {
                     instructions: "use custom model".to_string(),
                 },
                 user_facing_hint: None,
             },
         })
         .await
         .unwrap();
 ```

 **注解：** 提交审查请求。

 ```rust
     // 等待完成
     let _entered = wait_for_event(&agere, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
     let _closed = wait_for_event(&agere, |ev| {
         matches!(
             ev,
             EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
                 review_output: None
             })
         )
     })
     .await;
     let _complete = wait_for_event(&agere, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
 ```

 **注解：**

 - 等待 `EnteredReviewMode`
 - 等待 `ExitedReviewMode(None)` — review_output 为 None（因为无 output_item.done）
 - 等待 `TurnComplete`

 ```rust
     // 断言请求体模型等于配置的审查模型
     let request = request_log.single_request();
     assert_eq!(request.path(), "/v1/responses");
     let body = request.body_json();
     assert_eq!(body["model"].as_str().unwrap(), "gpt-5.4");
 ```

 **注解：**

 - `request_log.single_request()`：获取唯一的请求
 - `request.path()`：验证路径是 `/v1/responses`
 - `body["model"]`：验证请求体中的模型是 `"gpt-5.4"`
   - 不是 `"gpt-4.1"`（主会话模型）
   - 验证 `review_model` 配置生效

 ---

 ## 27.4 `review_uses_session_model_when_review_model_unset` 完整注解

 ```rust
 async fn review_uses_session_model_when_review_model_unset() {
     skip_if_no_network!();

     let sse_raw = r#"[
         {"type":"response.completed", "response": {"id": "__ID__"}}
     ]"#;
     let (server, request_log) =
         start_responses_server_with_sse(sse_raw, /*expected_requests*/ 1).await;
     let agere_home = Arc::new(TempDir::new().unwrap());
     let agere = new_conversation_for_server(&server, agere_home.clone(), |cfg| {
         cfg.model = Some("gpt-4.1".to_string());
         cfg.review_model = None;  // 未设置审查模型
     })
     .await;
 ```

 **注解：**

 - `cfg.model = "gpt-4.1"`：主会话模型
 - `cfg.review_model = None`：未设置审查模型
 - 验证 fallback 到主会话模型

 ```rust
     agere
         .submit(Op::Review {
             review_request: ReviewRequest {
                 target: ReviewTarget::Custom {
                     instructions: "use session model".to_string(),
                 },
                 user_facing_hint: None,
             },
         })
         .await
         .unwrap();

     let _entered = wait_for_event(&agere, |ev| matches!(ev, EventMsg::EnteredReviewMode(_))).await;
     let _closed = wait_for_event(&agere, |ev| {
         matches!(
             ev,
             EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
                 review_output: None
             })
         )
     })
     .await;
     let _complete = wait_for_event(&agere, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
 ```

 **注解：** 同上一个测试的事件等待。

 ```rust
     let request = request_log.single_request();
     assert_eq!(request.path(), "/v1/responses");
     let body = request.body_json();
     assert_eq!(body["model"].as_str().unwrap(), "gpt-4.1");
 ```

 **注解：**

 - `body["model"]`：验证请求体中的模型是 `"gpt-4.1"`（主会话模型）
   - 因为 `review_model = None`，fallback 到主模型

 ---

 ## 27.5 测试对比总结

 | 测试 | target | SSE 返回 | 验证重点 | review_output |
 |------|--------|---------|---------|---------------|
 | review_op_emits_lifecycle | Custom | JSON | 完整生命周期 + rollout | Some(structured) |
 | review_op_with_plain_text | Custom | 纯文本 | 兜底解析 | Some(fallback) |
 | review_filters_events | Custom | 流式 Delta | 事件抑制 | None (无 output_item) |
 | review_does_not_emit_agent_message | Custom | JSON | AgentMessage 数量 | None (无 last_msg) |
 | review_uses_custom_model | Custom | completed | review_model 配置 | None |
 | review_uses_session_model | Custom | completed | fallback 主模型 | None |
 | review_input_isolated | Custom | completed | 历史隔离 | None |
 | review_history_surfaces | Custom | 纯文本 | 后续 turn 引用 | Some(fallback) |
 | review_uses_overridden_cwd | BaseBranch | completed | cwd 覆盖 | None |

 ---

 > **一句话回顾**：完整测试注解（续）展示了 4 个测试的完整代码——`review_op_with_plain_text_emits_review_fallback`（验证策略 3 兜底）、`review_does_not_emit_agent_message_on_structured_output`（验证 AgentMessage 数量）、`review_uses_custom_review_model_from_config`（验证 review_model 生效）、`review_uses_session_model_when_review_model_unset`（验证 fallback），以及 9 个测试的对比总结表。

 ---

 # 第二十八部分 · 完整对比表集

 本部分汇集文档中所有对比表，方便集中查阅。

 ---

 ## 28.1 /review vs /autoreview vs Guardian 对比

 | 维度 | /review | /autoreview | Guardian 审查 |
 |------|---------|-------------|--------------|
 | 审查对象 | 代码改动 | Guardian 拒绝的操作 | 操作是否安全 |
 | 触发方式 | 用户主动 | 审批拒绝后重试 | Agent 请求审批操作 |
 | 执行者 | 隔离子代理 | (批准重试) | Guardian 审查会话 |
 | 输出类型 | ReviewOutputEvent | (无输出) | ReviewDecision |
 | 输出字段 | findings, correctness | - | Approved/Abort/Amendment |
 | 生命周期事件 | Entered/ExitedReviewMode | - | GuardianAssessmentEvent |
 | UI 状态 | is_review_mode | - | pending_guardian_review_status |
 | 权限模型 | Never (无需审批) | - | 取决于配置 |
 | 工具可用性 | web_search 禁用 | - | 取决于配置 |
 | 历史隔离 | 不携带主会话历史 | - | 独立审查会话 |
 | 持久化 | rollout conversation items | 不持久化 | 不持久化拒绝 |
 | 配置项 | review_model | - | approvals_reviewer |
 | 关键文件 | tasks/review.rs | auto_review_denials.rs | guardian/review.rs |
 | 代码量 | ~11KB | ~3KB | ~70KB |

 ---

 ## 28.2 ReviewTarget 变体对比

 | 变体 | 字段 | prompt 来源 | merge-base | UI hint |
 |------|------|------------|------------|---------|
 | UncommittedChanges | (无) | UNCOMMITTED_PROMPT (固定) | 不需要 | "current changes" |
 | BaseBranch | branch | BASE_BRANCH_PROMPT 或 BACKUP | 需要 | "changes against '{branch}'" |
 | Commit | sha, title? | COMMIT_PROMPT 或 WITH_TITLE | 不需要 | "commit {short_sha}[: title]" |
 | Custom | instructions | 直接用 instructions | 不需要 | instructions (trim) |

 ---

 ## 28.3 事件处理策略对比

 | 事件类型 | 处理策略 | 转发到主会话 | 原因 |
 |----------|---------|-------------|------|
 | AgentMessageContentDelta | Drop | 否 | 流式增量，审查不需要 |
 | AgentMessageDelta | Drop | 否 | legacy 流式增量 |
 | ItemCompleted(AgentMessage) | Drop | 否 | 避免触发 legacy AgentMessage |
 | AgentMessage | Buffer | 最终丢弃 | 只用 TurnComplete 的 last_agent_message |
 | TurnComplete | Terminal | 否（触发解析） | 从 last_agent_message 解析输出 |
 | TurnAborted | Terminal | 否（返回 None） | 中断信号 |
 | 其他（工具调用等） | Forward | 是 | 审查过程中的工具调用可见 |

 ---

 ## 28.4 Feature 禁用对比

 | Feature | 会话层禁用 | 任务层禁用 | 原因 |
 |---------|-----------|-----------|------|
 | WebSearchRequest | ✅ | ✅ (via web_search_mode) | 审查不需要联网搜索 |
 | WebSearchCached | ✅ | ✅ (via web_search_mode) | 同上 |
 | SpawnCsv | ❌ | ✅ | 审查不应 spawn 批量子代理 |
 | Collab | ❌ | ✅ | 审查不应使用协作工具 |

 ---

 ## 28.5 错误处理对比

 | 错误场景 | 处理方式 | 用户体验 |
 |----------|---------|---------|
 | resolve 失败 | emit ErrorEvent | 显示错误，不进入审查 |
 | 启动失败 | output=None → 中断模板 | "Review was interrupted..." |
 | 非 JSON 输出 | parse 兜底 → explanation | 显示纯文本 |
 | 部分 JSON | 提取子串解析 | 正常显示 findings |
 | 全空输出 | Default → fallback | "Reviewer failed to output..." |
 | 审查取消 | TurnAborted → None → abort | 中断模板 |
 | channel 关闭 | recv() Err → None | 中断模板 |
 | web_search set 失败(task) | panic | 程序崩溃 |
 | web_search set 失败(session) | warn + 保持原值 | 审查继续 |
 | 模板渲染失败 | panic | 程序崩溃 |

 ---

 ## 28.6 web_search_mode.set 行为对比

 | 层 | 失败行为 | 原因 |
 |----|---------|------|
 | task 层 (start_review_conversation) | panic! | 设计假设必须成功 |
 | session 层 (spawn_review_thread) | warn! + 保持原值 | 可能受 ConfigRequirements 约束 |

 ---

 ## 28.7 TurnContext 字段来源对比

 | 字段 | 来源 | 说明 |
 |------|------|------|
 | config | per_turn_config (new) | 审查专用配置 |
 | model_info | review_model_info (new) | 审查模型信息 |
 | tools_config | new | 审查专用工具配置 |
 | features | parent | 注意：用父的，非裁剪后的 |
 | developer_instructions | None | 审查不带 |
 | user_instructions | None | 审查不带 |
 | cwd | parent | 继承（含覆盖） |
 | approval_policy | parent | 继承（task 层覆盖为 Never） |
 | 其他 | parent 或 new | 多数继承 |

 ---

 ## 28.8 退出模板对比

 | 模板 | 文件 | 变量 | 用途 |
 |------|------|------|------|
 | exit_success.xml | core/templates/review/ | {{results}} | 成功退出，包裹审查结果 |
 | exit_interrupted.xml | core/templates/review/ | (无) | 中断退出，提示重新运行 |

 ---

 ## 28.9 消息 ID 对比

 | ID | 角色 | 内容 |
 |----|------|------|
 | "review_rollout_user" | user message | XML 模板（成功/中断） |
 | "review_rollout_assistant" | assistant message | 纯文本（成功/中断） |

 ---

 ## 28.10 测试覆盖对比

 | 场景 | 集成测试 | 单元测试 |
 |------|---------|---------|
 | 结构化输出 | ✅ | - |
 | 纯文本兜底 | ✅ | - |
 | 事件抑制 | ✅ | - |
 | AgentMessage 数量 | ✅ | - |
 | review_model 配置 | ✅ | - |
 | review_model fallback | ✅ | - |
 | 历史隔离 | ✅ | - |
 | 后续 turn 引用 | ✅ | - |
 | cwd 覆盖 | ✅ | - |
 | 模板渲染 | - | ✅ |
 | CRLF 规范化 | - | ✅ |
 | Prompt 模板 | - | ✅ (4个) |
 | AutoReview denials | - | ✅ |

 ---

 > **一句话回顾**：完整对比表集汇集了 10 张对比表——/review vs /autoreview vs Guardian、ReviewTarget 变体、事件处理策略、Feature 禁用、错误处理、web_search_mode.set 行为、TurnContext 字段来源、退出模板、消息 ID、测试覆盖，方便集中查阅和理解。

 ---

 ---

 # 第二十九部分 · 详细场景走读（续）

 本部分对更多场景进行详细走读，补充第三部分的深度。

 ---

 ## 29.1 场景：Commit 审查的完整走读

 ### 输入

 ```
 /review abc1234
 ```

 ### 阶段 1：命令解析

 ```
 1. SlashCommand::from_str("review") → Ok(Review)
 2. args = "abc1234"
 3. supports_inline_args() = true
 4. 识别 "abc1234" 为 commit SHA
 5. 构造 ReviewRequest:
    ReviewRequest {
        target: ReviewTarget::Commit {
            sha: "abc1234".to_string(),
            title: None,  // 从命令行无法获取 title
        },
        user_facing_hint: None,
    }
 6. session.submit(Op::Review { review_request })
 ```

 ### 阶段 2：请求解析

 ```
 handlers::review():
   1. turn_context = sess.new_default_turn_with_sub_id(sub_id)
   2. resolve_review_request(review_request, &turn_context.cwd)

 resolve_review_request():
   target = Commit { sha: "abc1234", title: None }
   prompt = review_prompt(Commit { sha, title }, cwd)

 review_prompt(Commit { sha: "abc1234", title: None }, cwd):
   match Commit { sha, title } =>
   match title:
       None =>
           render(COMMIT_PROMPT_TEMPLATE, [("sha", "abc1234")])
           = "Review the code changes introduced by commit abc1234.
              Provide prioritized, actionable findings."

   user_facing_hint = user_facing_hint(Commit { sha: "abc1234", title: None })
     short_sha = "abc1234".chars().take(7) = "abc1234"
     → "commit abc1234"

   返回 ResolvedReviewRequest {
       target: Commit { sha: "abc1234", title: None },
       prompt: "Review the code changes introduced by commit abc1234...",
       user_facing_hint: "commit abc1234",
   }
 ```

 ### 阶段 3：上下文构建

 ```
 spawn_review_thread():
   1. model = config.review_model.unwrap_or("gpt-4.1") = "gpt-5.4"
   2. review_features.disable(WebSearchRequest, WebSearchCached)
   3. tools_config = ToolsConfig::new(...).with_*()
   4. per_turn_config.model = "gpt-5.4"
   5. TurnContext { config: per_turn_config, developer_instructions: None, ... }
   6. input = [UserInput::Text { text: "Review the code changes...abc1234..." }]
   7. spawn_task(ReviewTask::new())
   8. emit EnteredReviewMode(ReviewRequest {
          target: Commit { sha: "abc1234", title: None },
          user_facing_hint: Some("commit abc1234"),
      })
 ```

 ### 阶段 4：TUI 进入审查模式

 ```
 TUI 收到 EnteredReviewMode:
   1. pre_review_token_info = Some(token_info)
   2. is_review_mode = true
   3. banner = ">> Code review started: commit abc1234 <<"
   4. add_to_history(banner)
   5. request_redraw()
 ```

 ### 阶段 5：子代理执行

 ```
 ReviewTask::run():
   1. counter("agere.task.review", 1)
   2. start_review_conversation():
      a. sub_agent_config.base_instructions = REVIEW_PROMPT
      b. approval_policy = Never
      c. features.disable(SpawnCsv, Collab)
      d. model = "gpt-5.4"
      e. run_agere_thread_one_shot(...)

 子代理执行:
   系统指令: REVIEW_PROMPT
     "You are acting as a reviewer..."
     "1. It meaningfully impacts accuracy..."
     "OUTPUT FORMAT: {findings: [...], ...}"

   用户指令: "Review the code changes introduced by commit abc1234.
              Provide prioritized, actionable findings."

   子代理操作:
     1. git show abc1234  (查看 commit 改动)
     2. 分析改动
     3. 生成 findings JSON
     4. 返回 TurnComplete { last_agent_message: Some(json) }
 ```

 ### 阶段 6：事件消费与解析

 ```
 process_review_events():
   1. 收到 AgentMessageContentDelta → 抑制
   2. 收到 AgentMessageDelta → 抑制
   3. 收到 ItemCompleted(AgentMessage) → 抑制
   4. 收到 AgentMessage(json) → 暂存
   5. 收到 TurnComplete { last_agent_message: Some(json) }
      → parse_review_output_event(json)
        → serde_json::from_str → 成功
        → 返回 ReviewOutputEvent {
            findings: [ReviewFinding {
                title: "[P2] Missing error handling",
                body: "The commit doesn't handle the case where...",
                confidence_score: 0.8,
                priority: 2,
                code_location: ReviewCodeLocation {
                    absolute_file_path: "/home/user/repo/src/main.rs",
                    line_range: ReviewLineRange { start: 42, end: 48 },
                },
            }],
            overall_correctness: "patch is incorrect",
            overall_explanation: "Missing error handling could cause panics.",
            overall_confidence_score: 0.8,
          }
 ```

 ### 阶段 7：退出与回灌

 ```
 exit_review_mode(Some(output)):
   1. findings_str = "Missing error handling could cause panics."
   2. block = format_review_findings_block(findings, None)
      = "\nReview comment:\n\n- [P2] Missing error handling — /home/user/repo/src/main.rs:42-48\n  The commit doesn't handle the case where..."
   3. findings_str += "\n" + block
   4. user_message = render_review_exit_success(findings_str)
      = "<user_action>...<results>Missing error handling...\n\nReview comment:...\n</results></user_action>"
   5. assistant_message = render_review_output_text(output)
      = "Missing error handling could cause panics.\n\nReview comment:\n\n- [P2] Missing error handling..."
   6. record user message (id: "review_rollout_user")
   7. emit ExitedReviewMode(Some(output))
   8. record assistant message (id: "review_rollout_assistant")
   9. ensure_rollout_materialized()
 ```

 ### 阶段 8：UI 渲染

 ```
 TUI 收到 ExitedReviewMode(Some(output)):
   1. review_markdown = render_review_output_text(output)
   2. record_agent_markdown(review_markdown)
   3. flush 三件套
   4. findings 非空 → 已在 record_agent_markdown 中处理
   5. exit_review_mode_after_item():
      a. flush streams
      b. is_review_mode = false
      c. restore_pre_review_token_info()
      d. banner = "<< Code review finished >>"
      e. request_redraw()
 ```

 ### 最终 UI 显示

 ```
 >> Code review started: commit abc1234 <<

 Missing error handling could cause panics.

 Review comment:

 - [P2] Missing error handling — /home/user/repo/src/main.rs:42-48
   The commit doesn't handle the case where...

 << Code review finished >>
 ```

 ---

 ## 29.2 场景：Custom 指令审查的完整走读

 ### 输入

 ```
 /review Focus on security vulnerabilities and input validation
 ```

 ### 阶段 1-3：命令解析、请求解析、上下文构建

 ```
 1. SlashCommand::Review, args = "Focus on security vulnerabilities and input validation"
 2. ReviewRequest {
        target: ReviewTarget::Custom {
            instructions: "Focus on security vulnerabilities and input validation",
        },
        user_facing_hint: None,
    }

 resolve_review_request():
   target = Custom { instructions: "Focus on security..." }
   prompt = review_prompt(Custom { instructions }, cwd):
     prompt = instructions.trim() = "Focus on security vulnerabilities and input validation"
     prompt.is_empty() = false
     → Ok("Focus on security vulnerabilities and input validation")
   user_facing_hint = "Focus on security vulnerabilities and input validation"

 spawn_review_thread():
   (同其他场景)
   emit EnteredReviewMode(ReviewRequest {
       target: Custom { instructions: "Focus on security..." },
       user_facing_hint: Some("Focus on security..."),
   })
 ```

 ### 阶段 4：TUI

 ```
 banner = ">> Code review started: Focus on security vulnerabilities and input validation <<"
 ```

 **注意：** banner 可能很长（完整的自定义指令），可能换行显示。

 ### 阶段 5：子代理执行

 ```
 子代理收到:
   系统指令: REVIEW_PROMPT (审查准则 + 输出 schema)
   用户指令: "Focus on security vulnerabilities and input validation"

 子代理行为:
   1. git diff (审查 working tree 改动)
   2. 聚焦于 security 和 input validation
   3. 生成 findings（只包含 security/validation 相关问题）
   4. 返回 JSON
 ```

 **关键点：**

 - Custom 指令不改变审查准则（REVIEW_PROMPT 仍然定义 bug 判定和输出格式）
 - Custom 指令只影响审查重点
 - 输出仍是 ReviewOutputEvent JSON

 ---

 ## 29.3 场景：空 Custom 指令的错误处理

 ### 输入

 ```
 /review    
 ```
 （注意：后面只有空格）

 ### 行为

 ```
 1. SlashCommand::Review, args = "   " (只有空格)
 2. ReviewRequest {
        target: ReviewTarget::Custom {
            instructions: "   ",
        },
        user_facing_hint: None,
    }
    (假设空格被识别为 Custom 指令)

 3. handlers::review():
    resolve_review_request(review_request, cwd)

 4. review_prompt(Custom { instructions: "   " }, cwd):
    prompt = "   ".trim() = ""
    prompt.is_empty() = true
    → anyhow::bail!("Review prompt cannot be empty")

 5. resolve_review_request 返回 Err

 6. handlers::review() 中:
    match Err(err) =>
      emit EventMsg::Error(ErrorEvent {
          message: "Review prompt cannot be empty",
          agere_error_info: Some(AgereErrorInfo::Other),
      })

 7. TUI 显示错误消息
    (不进入审查模式，不显示 banner)
 ```

 **关键点：**

 - 空指令在 resolve 阶段被捕获
 - 不进入审查模式（不 emit EnteredReviewMode）
 - 错误通过 ErrorEvent 传播
 - 用户需要重新输入非空指令

 ---

 ## 29.4 场景：BaseBranch 无 merge-base 的 fallback

 ### 输入

 ```
 /review orphan-branch
 ```
 （orphan-branch 是一个与当前分支无共同祖先的分支）

 ### 行为

 ```
 1. ReviewRequest { target: BaseBranch { branch: "orphan-branch" } }

 2. resolve_review_request():
    review_prompt(BaseBranch { "orphan-branch" }, cwd):
      merge_base_with_head(cwd, "orphan-branch")
        → git merge-base HEAD orphan-branch
        → 失败（无共同祖先）
        → Ok(None)  (注意：不是 Err，而是 Ok(None))

      match None =>
        // 使用 backup 模板
        render(BASE_BRANCH_PROMPT_BACKUP_TEMPLATE, [("branch", "orphan-branch")])
        = "Review the code changes against the base branch 'orphan-branch'.
           Start by finding the merge diff between the current branch and
           orphan-branch's upstream e.g. (`git merge-base HEAD \"$(git rev-parse
           --abbrev-ref \"orphan-branch@{upstream}\")\"`), then run `git diff`
           against that SHA to see what changes we would merge into the
           orphan-branch branch. Provide prioritized, actionable findings."

    user_facing_hint = "changes against 'orphan-branch'"

 3. spawn_review_thread() (同正常流程)
 4. emit EnteredReviewMode
 5. TUI: ">> Code review started: changes against 'orphan-branch' <<"
 6. 子代理收到 backup prompt
 7. 子代理自行执行 git 命令寻找 merge-base
 8. 子代理可能:
    a. 找到 merge-base → 执行 git diff → 审查
    b. 找不到 → 返回 "无法找到共同祖先" 或空 findings
 ```

 **关键点：**

 - 无 merge-base 不是错误（`Ok(None)`）
 - 使用 backup 模板（包含 git 命令示例）
 - 子代理自行处理（可能成功也可能失败）
 - backup 模板中的 git 命令示例指导子代理操作

 ---

 ## 29.5 场景：审查完成后立即对话引用结果

 ### 输入

 ```
 /review          (第一步：审查)
 Fix the P1 issue  (第二步：让 Agent 修复)
 ```

 ### 行为

 ```
 第一步：/review
   1. 审查完成
   2. rollout 记录:
      - user message (id: "review_rollout_user"): <user_action>...<results>...P1 issue...</results></user_action>
      - assistant message (id: "review_rollout_assistant"): "P1 issue found...\n\nFull review comments:..."

 第二步：Fix the P1 issue
   1. session.submit(Op::UserInput { items: [Text { text: "Fix the P1 issue" }] })
   2. Agent 收到的 input:
      [
        { role: "user", content: [{ text: "<user_action>...review results...</user_action>" }] },
        { role: "assistant", content: [{ text: "P1 issue found..." }] },
        { role: "user", content: [{ text: "Fix the P1 issue" }] }
      ]
   3. Agent 可以看到审查结果（前两条消息）
   4. Agent 理解 <user_action> 是审查结果
   5. Agent 根据 "Fix the P1 issue" 指令修复 P1 问题
 ```

 **关键点：**

 - 审查结果在后续 turn 中可见（作为 input 的一部分）
 - XML 模板让 Agent 理解这是审查结果
 - Agent 可以引用具体的 finding（如 P1 issue）
 - 测试 `review_history_surfaces_in_parent_session` 验证了这一点

 ---

 ## 29.6 场景：连续多次审查

 ### 输入

 ```
 /review          (第一次审查)
 /review          (第二次审查，代码已修改)
 ```

 ### 行为

 ```
 第一次 /review:
   1. 审查完成
   2. rollout 记录审查结果 1
   3. token_info 恢复

 第二次 /review:
   1. 新的 turn（独立于第一次）
   2. pre_review_token_info = Some(token_info)（重新保存）
   3. 审查完成
   4. rollout 记录审查结果 2
   5. token_info 恢复

 后续 turn 的 input:
   [
     { 审查 1 user message },
     { 审查 1 assistant message },
     { 审查 2 user message },
     { 审查 2 assistant message },
     { 新的用户输入 }
   ]
 ```

 **关键点：**

 - 每次审查是独立的 turn
 - 审查结果累积在 rollout 中
 - 后续 turn 可以看到所有历史审查结果
 - `pre_review_token_info` 每次进入审查时重新保存

 ---

 > **一句话回顾**：详细场景走读（续）提供了 6 个完整场景——Commit 审查（8 阶段完整走读）、Custom 指令审查（聚焦特定方面）、空 Custom 指令（resolve 失败 → ErrorEvent）、BaseBranch 无 merge-base（backup 模板 fallback）、审查后对话引用（XML 模板让 Agent 理解审查结果）、连续多次审查（独立 turn + 累积记录）。

 ---

 # 第三十部分 · 完整事件参考

 本部分列出审查相关的所有事件，含定义、载荷、触发位置、消费位置。

 ---

 ## 30.1 审查生命周期事件

 ### EnteredReviewMode

 ```
 定义: EventMsg::EnteredReviewMode(ReviewRequest)
 载荷: ReviewRequest { target, user_facing_hint }
 触发位置: spawn_review_thread() (session/review.rs:178)
 消费位置: TUI on_entered_review_mode() (chatwidget.rs:8013)
 用途: 通知 UI 进入审查模式
 ```

 **载荷详解：**

 ```rust
 ReviewRequest {
     target: ReviewTarget,           // 审查目标
     user_facing_hint: Option<String>, // UI 提示（总是 Some，从 spawn_review_thread 构造）
 }
 ```

 **TUI 处理：**

 ```
 on_entered_review_mode(review_request, from_replay):
   hint = review_request.user_facing_hint.unwrap_or_else(|| user_facing_hint(&target))
   enter_review_mode_with_hint(hint, from_replay):
     1. 保存 token_info
     2. 设 is_review_mode = true
     3. 显示 banner ">> Code review started: {hint} <<"
 ```

 ### ExitedReviewMode

 ```
 定义: EventMsg::ExitedReviewMode(ExitedReviewModeEvent)
 载荷: ExitedReviewModeEvent { review_output: Option<ReviewOutputEvent> }
 触发位置: exit_review_mode() (tasks/review.rs:222)
 消费位置: TUI on_exited_review_mode() (chatwidget.rs:8016)
 用途: 通知 UI 退出审查模式并渲染结果
 ```

 **载荷详解：**

 ```rust
 ExitedReviewModeEvent {
     review_output: Option<ReviewOutputEvent>,
     // Some: 审查成功（可能 findings 为空）
     // None: 审查中断/取消/启动失败
 }
 ```

 **TUI 处理：**

 ```
 on_exited_review_mode(review):
   match review.review_output:
     Some(output) =>
       1. review_markdown = render_review_output_text(output)
       2. record_agent_markdown(review_markdown)
       3. flush 三件套
       4. if findings.is_empty():
          if explanation.is_empty(): 显示错误
          else: 渲染 explanation
     None =>
       (不额外渲染)
   exit_review_mode_after_item():
     1. flush 三件套
     2. is_review_mode = false
     3. restore_pre_review_token_info()
     4. 显示 "<< Code review finished >>"
 ```

 ---

 ## 30.2 子代理事件（process_review_events 处理）

 ### AgentMessage

 ```
 定义: EventMsg::AgentMessage(AgentMessageEvent)
 处理: 暂存到 prev_agent_message
 转发: 仅在收到下一条 AgentMessage 时转发上一条
 用途: 子代理的完整消息（可能多条，只保留最后一条）
 ```

 ### AgentMessageDelta

 ```
 定义: EventMsg::AgentMessageDelta(AgentMessageDeltaEvent)
 处理: 抑制（Drop）
 转发: 否
 原因: legacy 流式增量，审查不需要
 ```

 ### AgentMessageContentDelta

 ```
 ```
 定义: EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent)
 处理: 抑制（Drop）
 转发: 否
 原因: 内容增量，审查不需要
 ```

 ### ItemCompleted(AgentMessage)

 ```
 定义: EventMsg::ItemCompleted(ItemCompletedEvent { item: TurnItem::AgentMessage(_), .. })
 处理: 抑制（Drop）
 转发: 否
 原因: 转发会触发 legacy AgentMessage via as_legacy_events()
 ```

 ### TurnComplete

 ```
 定义: EventMsg::TurnComplete(TaskCompleteEvent)
 处理: Terminal（触发解析）
 转发: 否（但触发 exit_review_mode）
 用途: 从 last_agent_message 解析 ReviewOutputEvent
 ```

 **载荷详解：**

 ```rust
 TaskCompleteEvent {
     last_agent_message: Option<String>,
     // Some: 子代理返回了消息，解析为 ReviewOutputEvent
     // None: 子代理没有返回消息，exit_review_mode(None)
 }
 ```

 ### TurnAborted

 ```
 定义: EventMsg::TurnAborted(AbortEvent)
 处理: Terminal（返回 None）
 转发: 否
 用途: 中断信号，exit_review_mode(None)
 ```

 ### 其他事件

 ```
 处理: Forward（转发到主会话）
 转发: 是
 包括: 工具调用事件、错误事件等
 用途: 审查过程中的工具调用对主会话可见
 ```

 ---

 ## 30.3 错误事件

 ### ErrorEvent（resolve 失败）

 ```
 定义: EventMsg::Error(ErrorEvent)
 载荷: ErrorEvent { message, agere_error_info }
 触发位置: handlers::review() (handlers.rs:1014)
 消费位置: TUI 错误处理
 用途: resolve_review_request 失败时通知 UI
 ```

 **载荷详解：**

 ```rust
 ErrorEvent {
     message: String,              // 错误消息（如 "Review prompt cannot be empty"）
     agere_error_info: Some(AgereErrorInfo::Other), // 通用错误分类
 }
 ```

 ---

 ## 30.4 Guardian 相关事件（关联对比）

 ### GuardianAssessmentEvent

 ```
 定义: EventMsg::GuardianAssessment(GuardianAssessmentEvent)
 载荷: GuardianAssessmentEvent { id, status, action, rationale, ... }
 触发位置: Guardian 审查会话
 消费位置: TUI (记录到 RecentAutoReviewDenials if Denied)
 用途: Guardian 审查结果通知
 ```

 **与 /review 事件的区别：**

 - `EnteredReviewMode` / `ExitedReviewMode`：/review 审查生命周期
 - `GuardianAssessmentEvent`：Guardian 审查结果
 - 两套独立的事件系统

 ---

 ## 30.5 事件流完整时序

 ### 正常审查的完整事件流

 ```
 [1] EventMsg::EnteredReviewMode(ReviewRequest { ... })
     ↑ spawn_review_thread emit
     ↓ TUI: enter_review_mode_with_hint

 [2] (子代理执行中，产生内部事件)
     EventMsg::AgentMessageContentDelta → 抑制
     EventMsg::AgentMessageDelta → 抑制
     EventMsg::ItemCompleted(AgentMessage) → 抑制
     EventMsg::AgentMessage(json) → 暂存
     (可能有多轮)

 [3] EventMsg::TurnComplete { last_agent_message: Some(json) }
     ↑ 子代理完成
     ↓ process_review_events: parse_review_output_event(json)

 [4] (exit_review_mode 执行)
     record user message

 [5] EventMsg::ExitedReviewMode(ExitedReviewModeEvent { review_output: Some(...) })
     ↑ exit_review_mode emit
     ↓ TUI: on_exited_review_mode

 [6] record assistant message
 [7] ensure_rollout_materialized
 ```

 ### 中断审查的完整事件流

 ```
 [1] EventMsg::EnteredReviewMode(ReviewRequest { ... })
     ↑ spawn_review_thread emit
     ↓ TUI: enter_review_mode_with_hint

 [2] (子代理执行中)

 [3] EventMsg::TurnAborted(AbortEvent)
     ↑ 用户 Ctrl+C，子代理被取消
     ↓ process_review_events: return None

 [4] (run() 检查 is_cancelled() → true，跳过 exit_review_mode)

 [5] (abort() 被调用)
     exit_review_mode(None)
     record user message (中断模板)

 [6] EventMsg::ExitedReviewMode(ExitedReviewModeEvent { review_output: None })
     ↑ exit_review_mode emit
     ↓ TUI: on_exited_review_mode (None → 不渲染结果)

 [7] record assistant message ("Review was interrupted...")
 [8] ensure_rollout_materialized
 ```

 ### resolve 失败的事件流

 ```
 [1] (Op::Review 提交)

 [2] (resolve_review_request 返回 Err)

 [3] EventMsg::Error(ErrorEvent { message: "...", ... })
     ↑ handlers::review() emit
     ↓ TUI: 显示错误

 (不进入审查模式，不 emit EnteredReviewMode)
 ```

 ---

 > **一句话回顾**：完整事件参考列出了所有审查相关事件——生命周期事件（EnteredReviewMode/ExitedReviewMode）、子代理事件（AgentMessage暂存/Delta抑制/ItemCompleted抑制/TurnComplete终端/TurnAborted终端/其他转发）、错误事件（ErrorEvent）、Guardian 事件（GuardianAssessmentEvent），以及正常/中断/resolve失败三种完整事件流时序。

 ---



*文档结束*
