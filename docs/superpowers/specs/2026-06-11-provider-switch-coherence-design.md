# Provider 切换一致性 设计文档

- 日期：2026-06-11
- 分支：`feat/provider-switch-coherence`
- 目标：让会话中途切换 model provider（`/provider`、app-server `turnStart.modelProvider`）在协议、压缩、上下文窗口三个维度上系统性地保持一致，而非逐点打补丁。

## 1. 背景与现状

当前未提交的改动已经实现了"运行时切换 provider"的主干：

- `ModelClient.provider` 改为 `StdRwLock<SharedModelProvider>`，新增 `set_provider()` 并重置缓存的 WebSocket 会话。
- `SessionSettingsUpdate` 增加 `provider`，`update_settings` 把它传播到 `model_client.set_provider()`。
- app-server `turnStart` 接受 `modelProvider` 覆盖，从 config 解析后调用 `thread.set_model_provider()`。
- TUI `/provider` 改为"下一条消息即生效"，并同时把 model 切到新 provider 默认模型、重建 TUI 侧 catalog。

协议分支（`stream()` 按 `wire_api` 动态分发）、压缩远程/内联选择（`supports_remote_compaction()` 仅 OpenAI/Azure 为真）、auth（请求时按新 provider 的 env_key/bearer 解析）、response_id 链（切换时随 WebSocket 一并重置）均已正确。

## 2. 根因

"当前 provider"被分散表示在三处，且会彼此 drift：

1. `session_configuration.provider` / `provider_id`（已随切换更新）
2. `model_client.provider`（已随切换更新）
3. `services.models_manager`（**会话创建时由原始 provider 构建一次，切换时不更新**）

由此派生出三个相互独立的缺陷：

- **C-A 上下文窗口根因**：`services.models_manager` 不更新，导致 `model_info.context_window`、可用模型列表、模型能力仍按**旧 provider** 解析。切到"同名模型但窗口更小"的 provider 时，核心不知道窗口变小，预轮压缩不会触发 → 可能向新后端发出超限请求直至 API 报 `ContextWindowExceeded`。
- **C-B 降档压缩 provider 盲区**：`maybe_run_previous_model_inline_compact` 的触发条件键于 **model slug 变化**（`turn.rs:764`），且 `PreviousTurnSettings` 不含 provider。纯 provider 切换（slug 不变）时降档压缩永不触发；即便触发，旧窗口也用当前 manager 解析，比较不可靠。
- **C-C 协议历史安全**：历史以协议中立的 `ResponseItem` 保存，各协议 translator 负责转换。跨协议切换时翻译层是防御性的（丢弃/降级而非崩溃），但加密推理/签名被**静默丢弃**（仅 debug 日志），且缺乏针对"会话中途切协议"的测试覆盖。

此外还有一个独立的次要缺陷：

- **C-D 切换原子性**：app-server 路径里 `model` 覆盖随 turn 队列施加（保序），但 `set_model_provider` 在 turn 提交**之前即时**施加。turn 执行中（多轮工具调用）切换会出现"新 provider + 旧 model"的短暂错配窗口。

## 3. 设计原则

**单一事实源 + 原子切换 + 协议安全历史。**一次 provider 切换应在**一个地方原子地**更新所有 provider 派生状态，使三处表示永不 drift；切换施加与 model 覆盖**同序**；历史在发往任一协议前按目标 `wire_api` 清洗。

## 4. 组件设计

### 组件 1 — provider 切换的状态一致性（核心，前置于组件 3）

**新增 `SwappableModelsManager`（`models-manager` crate）**

- 持有内部可变指针：`RwLock<SharedModelsManager>`（或等价的 `ArcSwap`）。
- 实现 `ModelsManager` trait，所有方法委托给当前 inner manager。
- 提供 `swap(new: SharedModelsManager)` 替换 inner。
- Session 把会话级 manager 包装为 `Arc<SwappableModelsManager>`，**对外仍以 `SharedModelsManager`（`Arc<dyn ModelsManager>`）类型暴露**——因此 `services.models_manager` 字段类型不变，约 53 处读取点零改动。Session 另存一个有类型的 `Arc<SwappableModelsManager>` 句柄用于切换。

**在 `Session::update_settings` 的 provider 分支统一更新**

切换时在同一处依次完成：
1. `session_configuration` 更新 provider / provider_id（已实现）
2. `model_client.set_provider(new_provider)`（已实现）
3. **新增**：用 `new_provider.models_manager(agere_home, config_model_catalog, collaboration_modes_config)` 工厂重建一个 manager，并 `swappable.swap(new_manager)`

重建所需的 `agere_home`、`config_model_catalog`、`collaboration_modes_config` 从 session 现有状态获取（与会话初始构建 `build_models_manager` 同源）。

**边界与权衡**

- StaticModelsManager（Anthropic / Bedrock / 配置型）重建无缓存代价；OpenAiModelsManager 重建会丢失 remote 模型缓存（etag），下次按需重新拉取，可接受。
- 切换在 turn 之间发生（见组件 2），不会与正在进行的解析竞争。

### 组件 2 — provider+model 原子切换（走 turn 队列）

- 给 `Op::UserInputWithTurnContext` 增加 `model_provider: Option<(String, ModelProviderInfo)>`（已解析的 provider）。
- `override_turn_context` / handlers 在组装 `SessionSettingsUpdate` 时，把 `model_provider` 与 `collaboration_mode`/model **放进同一个 update**，由同一次 `update_settings` 按提交顺序施加。
- app-server `turnStart`：保留**同步**解析 + 校验 provider（不存在则提前 `invalid_request` 失败），但把**施加**改为放入队列化的 op；移除即时的 `thread.set_model_provider()` 调用。
- 结果：provider 与 model 在同一 update 内施加，消除中途错配窗口。

### 组件 3 — 降档压缩对 provider 感知

- `PreviousTurnSettings` 增加 provider 标识（至少 `provider_id`；为正确解析旧窗口，可一并保存上一轮解析出的 `context_window` 或足以重建旧 manager 的信息）。
- `maybe_run_previous_model_inline_compact`：
  - 触发条件由 `slug != slug` 改为 **`model slug 变化 或 provider_id 变化`**。
  - 旧上下文窗口用**上一轮记录的窗口/旧 provider** 解析，而非当前 manager，保证 `old_window > new_window` 比较正确。
- 配合组件 1（manager 已切换），通用预轮压缩路径（`turn.rs:719`，`total_usage_tokens >= 新模型 auto_compact_limit`）对"同名模型、窗口变小"也能正确触发。

### 组件 4 — 协议安全历史（Anthropic / Responses / Chat）

- 引入按目标 `wire_api` 的**统一清洗 seam**：在 `stream()` 构建各协议请求前，对将要发送的 `ResponseItem` 历史做一次面向目标协议的规整（或将该规整集中进各 translator 的入口）。目标：跨协议切换后绝不发出畸形请求。
- 把加密推理 / 缺签名 thinking 块被丢弃/降级时的日志从 `debug` 升级为 `warn!`，便于排查静默语义损失。
- 不改变各协议既有的合法映射语义，仅保证健壮性与可见性。

### 组件 5 — 测试与验证

- **组件 1**：切 provider 后 `model_info.context_window` 反映新 provider；`services.models_manager` 解析使用新 catalog。
- **组件 2**：app-server 路径下 provider 与 model 在同一 update 内施加；turn 进行中切换不产生错配（顺序断言）。
- **组件 3**：纯 provider 切换到小窗口后端触发降档/通用压缩；slug 不变但 provider 变化时条件成立。
- **组件 4**：含 reasoning + tool_use 的历史在 Responses↔Anthropic↔Chat 三方向切换，断言请求体合法、降级路径正确、warn 日志产生。
- 回归：现有 `compact` / `session` / `model_provider_info` / app-server 套件全绿。

## 5. 影响面

- `models-manager` crate：新增 `SwappableModelsManager`（约 1 个聚焦的小文件）。
- `core/src/session/`：`update_settings` provider 分支扩展；`PreviousTurnSettings` 与降档压缩条件；session 持有 swappable 句柄。
- `core/src/client.rs` 或各 translator：协议清洗 seam + 日志级别。
- `app-server` + protocol：`Op::UserInputWithTurnContext.model_provider` 字段及其施加路径；移除即时施加。
- `services.models_manager` 的 53 处读取点：**不改动**（类型不变）。

## 6. 非目标（YAGNI）

- 不引入 provider 级别的模型自动映射/兼容性转换（如把 gpt-5 自动映射到 Anthropic 等价模型）。
- 不为非 env-key/bearer 的独立 OAuth provider 切换做 auth_manager 切换（当前 provider 群不需要）。
- 不做跨协议历史的语义无损转换（加密推理跨入 Chat 必然丢失，只保证健壮 + 可见）。

## 实现说明（与设计的偏差）

- **组件 2 的 Op 字段类型**：设计（§4 组件 2 第一条）写的是 `Op::UserInputWithTurnContext.model_provider: Option<(String, ModelProviderInfo)>`（携带已解析的 provider）。实际实现改为 `model_provider: Option<String>`，仅携带 provider **id**。原因：`protocol` crate 不能依赖 `model-provider-info`（会引入循环依赖）。解析改在 **core** 完成——`session/handlers.rs` 的 `user_input_or_turn_inner` 从会话已配置的 `model_providers` 中按 id 取出 `ModelProviderInfo`，组装进 `SessionSettingsUpdate.provider: Option<(String, ModelProviderInfo)>`；找不到 id 时 `warn!` 并不施加。app-server 侧仍保留**同步**校验（id 不存在则提前 `invalid_request` 失败），只是把**施加**改为随 turn 队列的 op（移除了即时的 `set_model_provider`），原子性目标不变。
- 其余组件（SwappableModelsManager 热替换、降档压缩 provider 感知、`sanitize_history_for_wire_api` 清洗 seam 与 debug→warn 日志升级）均按设计实现。
