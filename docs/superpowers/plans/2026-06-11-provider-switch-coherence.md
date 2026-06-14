# Provider 切换一致性 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让会话中途切换 model provider 在协议、压缩、上下文窗口、切换原子性四个维度上系统性保持一致。

**Architecture:** 引入每会话私有的 `SwappableModelsManager` 包装类（内部可变指针委托给当前 provider 的 manager），在 `update_settings` 的 provider 分支统一重建并 swap，使 `session_configuration.provider` / `model_client.provider` / `services.models_manager` 三者原子同步；扩展 `PreviousTurnSettings` 与降档压缩使其 provider 感知；把 provider 覆盖经 turn 队列与 model 同序施加；新增按目标 `wire_api` 的历史清洗 seam。

**Tech Stack:** Rust (tokio async-trait)，crates：`models-manager`、`core`、`app-server`、`app-server-protocol`、`protocol`。

**对应 spec：** `docs/superpowers/specs/2026-06-11-provider-switch-coherence-design.md`

---

## 文件结构

| 文件 | 责任 | 动作 |
|---|---|---|
| `models-manager/src/swappable.rs` | 每会话私有、可热替换的 `ModelsManager` 包装类 | Create |
| `models-manager/src/lib.rs` | 导出 `SwappableModelsManager` | Modify |
| `core/src/state/service.rs` | `SessionServices` 增加 `swappable_models_manager` 句柄 | Modify |
| `core/src/session/session.rs` | 会话构建时用 wrapper 包装共享 manager；下沉 provider 重建辅助 | Modify |
| `core/src/session/mod.rs` | `update_settings` provider 分支重建+swap manager；`PreviousTurnSettings` 增字段 | Modify |
| `core/src/state/session.rs` | `PreviousTurnSettings` 读写不变（字段透传） | Modify |
| `core/src/session/rollout_reconstruction.rs` | 写 `PreviousTurnSettings` 时带上 provider_id/window | Modify |
| `core/src/session/turn.rs` | 降档压缩条件改为 model 或 provider 变化 | Modify |
| `protocol/src/protocol.rs` | `Op::UserInputWithTurnContext` 增 `model_provider` | Modify |
| `core/src/session/handlers.rs` | 把 `model_provider` 组装进 `SessionSettingsUpdate.provider` | Modify |
| `app-server/src/agere_message_processor.rs` | provider 覆盖改为放进 Op，移除即时施加 | Modify |
| `core/src/history_sanitize.rs` | 按 `WireApi` 清洗 `ResponseItem` 历史 | Create |
| `core/src/client.rs` | `stream()` 在分发前应用清洗 seam | Modify |
| `core/src/lib.rs` | 挂载 `history_sanitize` 模块 | Modify |
| `anthropic-client/src/translate/request.rs` | 丢弃/降级日志 debug→warn | Modify |
| `openai-chat-client/src/translate/request.rs` | 丢弃加密推理日志 debug→warn | Modify |

---

## Phase A — 组件 1：每会话可热替换的 models manager

### Task 1: `SwappableModelsManager` 包装类

**Files:**
- Create: `models-manager/src/swappable.rs`
- Modify: `models-manager/src/lib.rs`
- Test: `models-manager/src/swappable.rs`（`#[cfg(test)]`）

**设计要点：** 包装类持有「稳定的 picker auth」（`Option<Arc<AuthManager>>`，会话级，切 provider 不变——`auth_manager(&self) -> Option<&AuthManager>` 返回借用必须来自稳定字段才健全）+「可热替换 inner」（`std::sync::RwLock<SharedModelsManager>`）。除 `auth_manager()` 外的所有 trait 方法都委托给当前 inner（取快照 Arc → 释放锁 → 调用/await），以忠实反映新 provider 的 catalog 语义。

- [ ] **Step 1: 写失败测试**

在 `models-manager/src/swappable.rs` 末尾：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::manager::RefreshStrategy;
    use crate::test_support::static_manager_with_models; // 见 Step 3 备注

    #[tokio::test]
    async fn swap_replaces_inner_catalog() {
        let first = static_manager_with_models(&["model-a"]);
        let second = static_manager_with_models(&["model-b"]);
        let swappable = SwappableModelsManager::new(first, /*picker_auth*/ None);

        let before = swappable.raw_model_catalog(RefreshStrategy::Offline).await;
        assert!(before.models.iter().any(|m| m.slug == "model-a"));

        swappable.swap(second);

        let after = swappable.raw_model_catalog(RefreshStrategy::Offline).await;
        assert!(after.models.iter().any(|m| m.slug == "model-b"));
        assert!(!after.models.iter().any(|m| m.slug == "model-a"));
    }

    #[tokio::test]
    async fn auth_manager_is_stable_across_swap() {
        let first = static_manager_with_models(&["model-a"]);
        let second = static_manager_with_models(&["model-b"]);
        let swappable = SwappableModelsManager::new(first, /*picker_auth*/ None);
        assert!(swappable.auth_manager().is_none());
        swappable.swap(second);
        assert!(swappable.auth_manager().is_none());
    }
}
```

> 备注：若 `models-manager` 无现成的 static manager 测试构造器，在本任务先加一个最小 `test_support::static_manager_with_models(slugs: &[&str]) -> SharedModelsManager`，用 `StaticModelsManager::new(None, ModelsResponse { models: slugs.iter().map(|s| ModelInfo{ slug:(*s).into(), ..Default::default()}).collect() }, CollaborationModesConfig::default())`。如 `ModelInfo` 无 `Default`，用现有 bundled 构造或 `model_info::model_info_from_slug`。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p agere-models-manager swappable -- --nocapture`
Expected: 编译失败（`SwappableModelsManager` 未定义）。

- [ ] **Step 3: 实现包装类**

`models-manager/src/swappable.rs` 顶部：

```rust
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::TryLockError;

use agere_login::AuthManager;

use crate::collaboration_mode_presets::CollaborationModeMask;
use crate::manager::ModelsManager;
use crate::manager::RefreshStrategy;
use crate::manager::SharedModelsManager;
use agere_protocol::models::ModelInfo;
use agere_protocol::openai_models::ModelsResponse;

/// 每会话私有、可在 provider 切换时热替换 inner 的 `ModelsManager` 包装。
///
/// 除 `auth_manager()`（返回稳定的会话级 picker auth）外，所有方法都委托给当前 inner。
#[derive(Debug)]
pub struct SwappableModelsManager {
    inner: RwLock<SharedModelsManager>,
    picker_auth: Option<Arc<AuthManager>>,
}

impl SwappableModelsManager {
    pub fn new(inner: SharedModelsManager, picker_auth: Option<Arc<AuthManager>>) -> Self {
        Self {
            inner: RwLock::new(inner),
            picker_auth,
        }
    }

    /// 用新 provider 构建出的 manager 替换当前 inner。
    pub fn swap(&self, new_inner: SharedModelsManager) {
        *self
            .inner
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = new_inner;
    }

    fn current(&self) -> SharedModelsManager {
        Arc::clone(
            &self
                .inner
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

#[async_trait::async_trait]
impl ModelsManager for SwappableModelsManager {
    async fn raw_model_catalog(&self, refresh_strategy: RefreshStrategy) -> ModelsResponse {
        self.current().raw_model_catalog(refresh_strategy).await
    }

    async fn get_remote_models(&self) -> Vec<ModelInfo> {
        self.current().get_remote_models().await
    }

    fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError> {
        self.current().try_get_remote_models()
    }

    fn auth_manager(&self) -> Option<&AuthManager> {
        self.picker_auth.as_deref()
    }

    fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        self.current().list_collaboration_modes()
    }

    async fn refresh_if_new_etag(&self, etag: String) {
        self.current().refresh_if_new_etag(etag).await;
    }
}
```

> 说明：`list_models` / `build_available_models` / `try_list_models` / `get_default_model` / `get_model_info` 沿用 trait 默认实现，默认实现内部调用的 `self.raw_model_catalog` / `self.get_remote_models` / `self.try_get_remote_models` 均已委托至 inner，因此与 inner 行为一致。导入路径以实际 crate 为准（用 `cargo check` 校正 `CollaborationModeMask`、`ModelInfo`、`ModelsResponse`、`AuthManager` 的真实模块路径）。

`models-manager/src/lib.rs` 增加：

```rust
pub mod swappable;
pub use swappable::SwappableModelsManager;
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cargo test -p agere-models-manager swappable`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add models-manager/src/swappable.rs models-manager/src/lib.rs
git commit -m "feat(models-manager): add SwappableModelsManager for per-session provider hot-swap"
```

---

### Task 2: 会话构建时用 wrapper 包装共享 manager

**Files:**
- Modify: `core/src/state/service.rs:52`（`SessionServices` 增 `swappable_models_manager`）
- Modify: `core/src/session/session.rs:758`（构建处）
- Test: 复用 Task 3 的集成测试（本任务为纯接线，先保证编译与既有套件绿）

- [ ] **Step 1: `SessionServices` 增句柄字段**

在 `core/src/state/service.rs` 的 `SessionServices`（line 36-74）`models_manager` 字段后新增：

```rust
    pub(crate) models_manager: SharedModelsManager,
    /// 指向 `models_manager` 同一对象的有类型句柄，用于 provider 切换时热替换 inner。
    pub(crate) swappable_models_manager: std::sync::Arc<agere_models_manager::SwappableModelsManager>,
```

- [ ] **Step 2: 构建处包装**

在 `core/src/session/session.rs` 构建 `SessionServices` 处（line 758 附近 `models_manager: Arc::clone(&models_manager),`），改为：

```rust
                // 每会话私有 wrapper：初始 inner 为 ThreadManager 共享 manager，
                // 切 provider 时只替换本会话的 inner，不影响其它会话。
                swappable_models_manager: {
                    let swappable = std::sync::Arc::new(
                        agere_models_manager::SwappableModelsManager::new(
                            Arc::clone(&models_manager),
                            Some(Arc::clone(&auth_manager)),
                        ),
                    );
                    swappable
                },
                models_manager: {
                    // models_manager 字段指向同一 wrapper（作为 dyn ModelsManager）。
                    // 注意：上面 swappable 已 move，此处需在外层先建好再分别赋值，见 Step 3。
                    unreachable!("replaced in Step 3")
                },
```

- [ ] **Step 3: 修正赋值顺序（先建 wrapper，再分别赋值两个字段）**

把 Step 2 的两段替换为：在构建 `SessionServices { ... }` 的**语句之前**先建 wrapper，再在结构体里引用：

```rust
        let swappable_models_manager = std::sync::Arc::new(
            agere_models_manager::SwappableModelsManager::new(
                Arc::clone(&models_manager),
                Some(Arc::clone(&auth_manager)),
            ),
        );
        let models_manager: SharedModelsManager = swappable_models_manager.clone();
```

然后结构体内：

```rust
                models_manager: Arc::clone(&models_manager),
                swappable_models_manager: Arc::clone(&swappable_models_manager),
```

> 确认 `auth_manager` 在该作用域可得（`SessionServices.auth_manager` 同源）。`SharedModelsManager = Arc<dyn ModelsManager>`，`Arc<SwappableModelsManager>` 可直接 `.clone()` 后赋给 `SharedModelsManager`（自动 unsize）。如类型推断不通过，显式 `let models_manager: SharedModelsManager = swappable_models_manager.clone();`。

- [ ] **Step 4: 编译 + 既有套件回归**

Run: `cargo test -p agere-core session::tests -- --nocapture`
Expected: 编译通过，既有 session 测试全绿（manager 行为与原先一致，因为 inner 即原共享 manager）。

- [ ] **Step 5: 提交**

```bash
git add core/src/state/service.rs core/src/session/session.rs
git commit -m "feat(core): wrap session models_manager in per-session SwappableModelsManager"
```

---

### Task 3: `update_settings` provider 分支重建并 swap manager

**Files:**
- Modify: `core/src/session/mod.rs:1332-1383`（`update_settings`）
- Modify: `core/src/session/session.rs`（新增 `build_provider_models_manager` 辅助）
- Test: `core/src/session/tests.rs`

- [ ] **Step 1: 写失败测试**

在 `core/src/session/tests.rs` 末尾（参照现有 `session_settings_provider_update_changes_provider_and_snapshot` 用到的 `make_session_*` 助手；本测试需要真实 `Session` 以验证 `services.models_manager` 解析变化）：

```rust
#[tokio::test]
async fn provider_switch_rebuilds_models_manager_context_window() {
    // make_session_and_context 提供真实 Session + 初始 provider/model。
    let (session, _ctx) = make_session_and_context().await;

    // 构造一个把同名模型上下文窗口设小的 provider（config 型，带 models 覆盖），
    // 切换后断言 services.models_manager.get_model_info(slug).context_window 反映新值。
    let small = small_window_provider_for_tests(); // 见 Step 4 备注
    session
        .update_settings(SessionSettingsUpdate {
            provider: Some(("small".to_string(), small)),
            ..Default::default()
        })
        .await
        .expect("provider switch should apply");

    let cfg = session.models_manager_config_for_tests().await; // 见 Step 4 备注
    let info = session
        .services
        .models_manager
        .get_model_info(&session.current_model_slug_for_tests().await, &cfg)
        .await;
    assert_eq!(info.context_window, Some(256_000));
}
```

> 若现有测试助手不足，本任务可改为更聚焦的单元测试：直接对 `SwappableModelsManager` 调用 `swap()` 后断言 `get_model_info` 变化（已在 Task 1 覆盖核心），并对 `update_settings` 仅断言「swap 被调用」（通过切换后 `raw_model_catalog` 的 slug 集合变化）。优先用真实 `Session`，不行则降级。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p agere-core provider_switch_rebuilds_models_manager`
Expected: FAIL（切换后窗口仍为旧值 / 助手未定义）。

- [ ] **Step 3: 新增 provider→manager 构建辅助**

在 `core/src/session/session.rs`（`impl Session` 内合适位置）新增：

```rust
    /// 用给定 provider 与会话现有 config，构建一个该 provider 自己的 models manager。
    /// 供 provider 切换时热替换使用。
    pub(super) fn build_provider_models_manager(
        &self,
        provider: &agere_model_provider_info::ModelProviderInfo,
    ) -> agere_models_manager::manager::SharedModelsManager {
        use agere_features::Feature;
        use agere_models_manager::collaboration_mode_presets::CollaborationModesConfig;
        use agere_model_provider::create_model_provider;

        // CollaborationModesConfig 仅含一个由 feature 派生的 bool，无需跨 spawn 配线。
        let collaboration_modes_config = CollaborationModesConfig {
            default_mode_request_user_input: self
                .features
                .enabled(Feature::DefaultModeRequestUserInput),
        };
        let config = self
            .services
            // original config 持有 agere_home / model_catalog / models。
            // 通过 state 读取当前 session_configuration.original_config_do_not_use。
            ;
        // 见 Step 3b：config 实际从 state 读取。
        unimplemented!("filled in Step 3b")
    }
```

- [ ] **Step 3b: 填入 config 读取与构建主体**

把 Step 3 的函数体替换为完整实现（`config` 从 `session_configuration.original_config_do_not_use` 读取；该读取需 `self.state.lock().await`，故函数改为 `async`）：

```rust
    pub(super) async fn build_provider_models_manager(
        &self,
        provider: &agere_model_provider_info::ModelProviderInfo,
    ) -> agere_models_manager::manager::SharedModelsManager {
        use agere_features::Feature;
        use agere_models_manager::collaboration_mode_presets::CollaborationModesConfig;
        use agere_model_provider::create_model_provider;

        let collaboration_modes_config = CollaborationModesConfig {
            default_mode_request_user_input: self
                .features
                .enabled(Feature::DefaultModeRequestUserInput),
        };
        let (agere_home, model_catalog, models) = {
            let state = self.state.lock().await;
            let cfg = &state.session_configuration.original_config_do_not_use;
            (
                cfg.agere_home.to_path_buf(),
                cfg.model_catalog.clone(),
                cfg.models.clone(),
            )
        };
        let runtime_provider = create_model_provider(
            provider.clone(),
            Some(Arc::clone(&self.services.auth_manager)),
            models,
        );
        runtime_provider.models_manager(agere_home, model_catalog, collaboration_modes_config)
    }
```

> 校正：`create_model_provider` 的第三参为 `Vec<ModelConfig>`（即 `config.models`）；`models_manager(...)` 第三参为 `CollaborationModesConfig`。导入路径以 `cargo check` 为准。

- [ ] **Step 4: 在 `update_settings` provider 分支调用 swap**

在 `core/src/session/mod.rs` 的 `update_settings`（line 1378 附近）把：

```rust
    if let Some(provider) = provider_update {
        self.services.model_client.set_provider(provider);
    }
```

替换为：

```rust
    if let Some(provider) = provider_update {
        // 三者原子同步：wire client、本会话 models manager、session_configuration（上文已 apply）。
        self.services.model_client.set_provider(provider.clone());
        let new_manager = self.build_provider_models_manager(&provider).await;
        self.services.swappable_models_manager.swap(new_manager);
    }
```

> Step 1 测试若用到 `small_window_provider_for_tests` / `models_manager_config_for_tests` / `current_model_slug_for_tests`，在 `tests.rs` 内实现：provider 用 config 型（`wire_api: Chat` 或 `Responses`），`models` 含一个与当前 slug 同名、`context_window=256000` 的 `ModelConfig`；config 通过 `to_models_manager_config()` 取得。

- [ ] **Step 5: 运行测试确认通过 + 回归**

Run: `cargo test -p agere-core provider_switch_rebuilds_models_manager && cargo test -p agere-core session::tests`
Expected: PASS。

- [ ] **Step 6: 提交**

```bash
git add core/src/session/mod.rs core/src/session/session.rs core/src/session/tests.rs
git commit -m "feat(core): rebuild and swap session models_manager on provider switch"
```

---

## Phase B — 组件 3：降档压缩 provider 感知

### Task 4: `PreviousTurnSettings` 增加 provider 标识与窗口

**Files:**
- Modify: `core/src/session/mod.rs:269-272`（struct）
- Modify: `core/src/session/rollout_reconstruction.rs:174-177`（写入）
- Test: `core/src/session/tests.rs`

- [ ] **Step 1: 写失败测试**

在 `core/src/session/tests.rs`：

```rust
#[test]
fn previous_turn_settings_carry_provider_and_window() {
    let s = PreviousTurnSettings {
        model: "gpt-5".to_string(),
        realtime_active: None,
        provider_id: "openai".to_string(),
        context_window: Some(1_000_000),
    };
    assert_eq!(s.provider_id, "openai");
    assert_eq!(s.context_window, Some(1_000_000));
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p agere-core previous_turn_settings_carry_provider`
Expected: FAIL（字段不存在）。

- [ ] **Step 3: 扩展 struct**

`core/src/session/mod.rs:269-272` 改为：

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreviousTurnSettings {
    pub(crate) model: String,
    pub(crate) realtime_active: Option<bool>,
    /// 上一轮使用的 provider 标识，用于 provider 感知的降档压缩。
    pub(crate) provider_id: String,
    /// 上一轮解析出的上下文窗口，用于与切换后窗口比较（避免用当前 manager 误判）。
    pub(crate) context_window: Option<i64>,
}
```

- [ ] **Step 4: 更新写入处**

`core/src/session/rollout_reconstruction.rs:174-177`。`ctx`（`TurnContextItem`）需提供 provider_id 与 window。若 `TurnContextItem` 已含 `model_provider_id`（thread_config_snapshot 已有该概念）与上下文窗口则直接取；否则从 `ctx` 现有字段推导。改为：

```rust
active_segment.previous_turn_settings = Some(PreviousTurnSettings {
    model: ctx.model.clone(),
    realtime_active: ctx.realtime_active,
    provider_id: ctx.model_provider_id.clone(),
    context_window: ctx.context_window,
});
```

> 若 `TurnContextItem` 无 `model_provider_id` / `context_window` 字段，则在本任务先给它补这两个字段，并在其构造处（写 TurnContext 快照的地方）用 `session_configuration.provider_id` 与 `turn_context.model_context_window()` 填充。用 `grep -rn "struct TurnContextItem" core/src` 定位。

- [ ] **Step 5: 修编译错误（所有构造 `PreviousTurnSettings` 的点）**

`grep -rn "PreviousTurnSettings {" core/src` 找到所有字面构造（含测试），补 `provider_id` 与 `context_window` 两字段。测试里给合理默认（如 `provider_id: "openai".into(), context_window: None`）。

- [ ] **Step 6: 运行确认通过**

Run: `cargo test -p agere-core previous_turn_settings_carry_provider && cargo build -p agere-core`
Expected: PASS + 编译通过。

- [ ] **Step 7: 提交**

```bash
git add -A
git commit -m "feat(core): track provider_id and context_window in PreviousTurnSettings"
```

---

### Task 5: 降档压缩条件 provider 感知

**Files:**
- Modify: `core/src/session/turn.rs:739-778`
- Test: `core/src/compact_tests.rs` 或 `core/src/session/tests.rs`

- [ ] **Step 1: 写失败测试**

降档压缩依赖真实 Session/turn，端到端较重。改为对触发**判定逻辑**做单元测试：把判定抽成纯函数 `should_run_downshift_compaction`，便于测试。先写测试（在 `core/src/session/turn.rs` 的 `#[cfg(test)]`）：

```rust
#[cfg(test)]
mod downshift_tests {
    use super::should_run_downshift_compaction;

    #[test]
    fn triggers_on_provider_change_same_model() {
        // 同 model，不同 provider，窗口变小，token 超新阈值 → 触发
        assert!(should_run_downshift_compaction(
            /*total*/ 300_000,
            /*new_auto_compact_limit*/ 230_000,
            /*old_model*/ "gpt-5",
            /*new_model*/ "gpt-5",
            /*old_provider*/ "openai",
            /*new_provider*/ "small",
            /*old_window*/ 1_000_000,
            /*new_window*/ 256_000,
        ));
    }

    #[test]
    fn triggers_on_model_change() {
        assert!(should_run_downshift_compaction(
            300_000, 230_000, "gpt-5", "gpt-5-mini", "openai", "openai", 400_000, 256_000,
        ));
    }

    #[test]
    fn no_trigger_when_nothing_changed() {
        assert!(!should_run_downshift_compaction(
            300_000, 230_000, "gpt-5", "gpt-5", "openai", "openai", 256_000, 256_000,
        ));
    }

    #[test]
    fn no_trigger_when_window_not_smaller() {
        assert!(!should_run_downshift_compaction(
            300_000, 230_000, "gpt-5", "gpt-5", "openai", "small", 256_000, 1_000_000,
        ));
    }

    #[test]
    fn no_trigger_under_limit() {
        assert!(!should_run_downshift_compaction(
            100_000, 230_000, "gpt-5", "gpt-5", "openai", "small", 1_000_000, 256_000,
        ));
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p agere-core downshift_tests`
Expected: FAIL（函数未定义）。

- [ ] **Step 3: 抽出纯函数**

在 `core/src/session/turn.rs`（`maybe_run_previous_model_inline_compact` 上方）新增：

```rust
#[allow(clippy::too_many_arguments)]
fn should_run_downshift_compaction(
    total_usage_tokens: i64,
    new_auto_compact_limit: i64,
    old_model: &str,
    new_model: &str,
    old_provider_id: &str,
    new_provider_id: &str,
    old_context_window: i64,
    new_context_window: i64,
) -> bool {
    let changed = old_model != new_model || old_provider_id != new_provider_id;
    total_usage_tokens > new_auto_compact_limit
        && changed
        && old_context_window > new_context_window
}
```

- [ ] **Step 4: 在 `maybe_run_previous_model_inline_compact` 调用纯函数**

把 `turn.rs:753-765` 替换为（用 `previous_turn_settings` 记录的旧窗口/旧 provider，而非当前 manager 重解析）：

```rust
    let old_context_window = match previous_turn_settings.context_window {
        Some(w) => w,
        // 旧窗口缺失时回退到用旧 model 在当前 manager 上的解析（退化为原行为）。
        None => match previous_model_turn_context.model_context_window() {
            Some(w) => w as i64,
            None => return Ok(false),
        },
    };
    let Some(new_context_window) = turn_context.model_context_window() else {
        return Ok(false);
    };
    let new_auto_compact_limit = turn_context
        .model_info
        .auto_compact_token_limit()
        .unwrap_or(i64::MAX);
    let new_provider_id = turn_context.config.model_provider_id.as_str();
    let should_run = should_run_downshift_compaction(
        total_usage_tokens,
        new_auto_compact_limit,
        previous_model_turn_context.model_info.slug.as_str(),
        turn_context.model_info.slug.as_str(),
        previous_turn_settings.provider_id.as_str(),
        new_provider_id,
        old_context_window,
        new_context_window as i64,
    );
```

> 类型校正：`model_context_window()` 返回 `Option<i64>` 还是其它整型，按实际签名转换（`as i64`）。`turn_context.config.model_provider_id` 取当前轮 provider id（per_turn_config 已在现有 diff 设置 `model_provider_id`）。`previous_model_turn_context` 仍按现有逻辑用 `with_model(previous_turn_settings.model.clone(), ...)` 构建（用于压缩执行的旧 model 上下文），但窗口比较改用记录值。

- [ ] **Step 5: 运行确认通过 + 回归**

Run: `cargo test -p agere-core downshift_tests && cargo test -p agere-core compact`
Expected: PASS。

- [ ] **Step 6: 提交**

```bash
git add core/src/session/turn.rs
git commit -m "feat(core): make downshift compaction provider-aware"
```

---

## Phase C — 组件 2：provider+model 原子切换（走 turn 队列）

### Task 6: `Op::UserInputWithTurnContext` 增 `model_provider`，handler 组装进 update

**Files:**
- Modify: `protocol/src/protocol.rs:446-510`
- Modify: `core/src/session/handlers.rs:177-225`
- Test: `core/src/session/tests.rs`

- [ ] **Step 1: 写失败测试**

`core/src/session/tests.rs`（参照现有 `tests.rs:4213` 处构造 `Op::UserInputWithTurnContext`）：

```rust
#[tokio::test]
async fn turn_context_op_applies_provider_with_model_in_one_update() {
    let (session, _ctx) = make_session_and_context().await;
    let small = small_window_provider_for_tests();
    // 经 handler 路径提交带 model_provider 的 Op，断言切换后 session_configuration.provider_id 改变。
    crate::session::handlers::override_turn_context(
        &session,
        "sub-1".to_string(),
        SessionSettingsUpdate {
            provider: Some(("small".to_string(), small)),
            collaboration_mode: None,
            ..Default::default()
        },
    )
    .await;
    let snap = session.thread_config_snapshot_for_tests().await;
    assert_eq!(snap.model_provider_id, "small");
}
```

> 该测试主要验证 `update_settings` 经由 `override_turn_context` 同序施加 provider。Op 字段本身的存在性由编译保证；可另加一个 protocol crate 的序列化测试。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p agere-core turn_context_op_applies_provider`
Expected: FAIL。

- [ ] **Step 3: protocol 增字段**

`protocol/src/protocol.rs` 的 `UserInputWithTurnContext` 变体，在 `model: Option<String>` 字段后新增：

```rust
    /// Updated model provider override. Carries the resolved provider so the
    /// update applies atomically with `model` in submission order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model_provider: Option<(String, agere_model_provider_info::ModelProviderInfo)>,
```

> 确认 `protocol` crate 已依赖 `agere_model_provider_info`；若无则改为只携带 `provider_id: String`，由 core 端在 handler 内用 config 解析（但这会把 config 依赖引入 core handler；优先携带已解析的 provider，与 app-server 端解析保持一致——见 Task 7）。若 `ModelProviderInfo` 未实现 `Eq`/`Hash` 等 Op 所需 derive，给该字段所在 enum 的 derive 做相应放宽或用 `#[serde(skip)]` 评估。

- [ ] **Step 4: handler 组装进 `SessionSettingsUpdate.provider`**

`core/src/session/handlers.rs:177-225`，在解构中加入 `model_provider`，并在构造 `SessionSettingsUpdate` 时设 `provider`：

解构增加：

```rust
            model,
            model_provider,
            effort,
```

构造体增加：

```rust
        SessionSettingsUpdate {
            provider: model_provider,
            cwd,
            approval_policy,
            ...
        },
```

- [ ] **Step 5: 修所有构造 `Op::UserInputWithTurnContext` 的点**

`grep -rn "UserInputWithTurnContext {" core/src exec app-server` 找到所有构造点（exec/src/lib.rs、app-server、tests），补 `model_provider: None`（app-server 在 Task 7 改为传实际值）。

- [ ] **Step 6: 运行确认通过**

Run: `cargo test -p agere-core turn_context_op_applies_provider && cargo build`
Expected: PASS + 全 crate 编译通过。

- [ ] **Step 7: 提交**

```bash
git add -A
git commit -m "feat: thread model_provider override through UserInputWithTurnContext op"
```

---

### Task 7: app-server 解析+校验后入队，移除即时施加

**Files:**
- Modify: `app-server/src/agere_message_processor.rs:6205-6269`
- Test: `app-server/src/message_processor/*tests*`（如有）或保留 protocol 层测试

- [ ] **Step 1: 调整施加路径**

`agere_message_processor.rs`：保留 `provider_update` 的**同步解析+校验**（line 6205-6236 不变），但删除即时施加块（line 6262-6269 的 `thread.set_model_provider(...)`）。改为把 `provider_update` 放进 `Op::UserInputWithTurnContext` 的 `model_provider` 字段。

在 `let turn_op = if has_any_overrides { Op::UserInputWithTurnContext { ... model, ... } }` 处加入：

```rust
                    model,
                    model_provider: provider_update,
                    effort,
```

并删除：

```rust
            if let Some((model_provider_id, provider)) = provider_update {
                thread
                    .set_model_provider(model_provider_id, provider)
                    .await
                    .map_err(|err| {
                        invalid_request(format!("invalid model provider override: {err}"))
                    })?;
            }
```

> 注意：`provider_update` 现被 move 进 Op，确保 `has_any_overrides` 在 `params.model_provider.is_some()` 时为真（line 6188 已含该判断）。`thread.set_model_provider` 若不再有其它调用方，可保留（供直接 API）或标 `#[allow(dead_code)]`；优先保留以备 test/外部用。

- [ ] **Step 2: 编译 + app-server 套件回归**

Run: `cargo test -p agere-app-server`
Expected: 编译通过，既有套件绿。

- [ ] **Step 3: 端到端顺序断言（如套件支持）**

若 app-server 有 turn 顺序测试框架，加一个：提交带 `modelProvider` + `model` 的 `turnStart`，断言二者在同一 turn 生效（provider_id 与 model 同时变化），且**前一个进行中的 turn 不受影响**。无框架则记 TODO 由组件 5 的集成测试覆盖。

- [ ] **Step 4: 提交**

```bash
git add app-server/src/agere_message_processor.rs
git commit -m "fix(app-server): apply model_provider override atomically via turn queue"
```

---

## Phase D — 组件 4：协议安全历史清洗

### Task 8: `sanitize_history_for_wire_api`

**Files:**
- Create: `core/src/history_sanitize.rs`
- Modify: `core/src/lib.rs`（挂模块）
- Test: `core/src/history_sanitize.rs`（`#[cfg(test)]`）

**语义：** 给定目标 `WireApi` 与 `&[ResponseItem]`，返回对该协议安全的 `Vec<ResponseItem>`。当前各 translator 已能防御（丢弃/降级），本函数集中处理**会导致畸形或语义错配**的项，并使决策可测：
- 目标 `Chat`：`Reasoning` 项清空 `encrypted_content`/`signature`（Chat 无法表达；保留 summary/content 文本）。
- 目标 `Responses`：移除仅 Anthropic 有意义的 `signature`（保留 `encrypted_content`，Responses 后端容忍）。
- 目标 `Anthropic`：保留 `encrypted_content`+`signature` 配对；其余不动（translator 自行降级）。
- 其它项（Message/FunctionCall/...）原样保留。

- [ ] **Step 1: 写失败测试**

`core/src/history_sanitize.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::sanitize_history_for_wire_api;
    use agere_model_provider_info::WireApi;
    use agere_protocol::models::ResponseItem;
    use agere_protocol::models::ReasoningItemReasoningSummary;

    fn reasoning_with(enc: Option<&str>, sig: Option<&str>) -> ResponseItem {
        ResponseItem::Reasoning {
            id: String::new(),
            summary: vec![ReasoningItemReasoningSummary::SummaryText {
                text: "s".into(),
            }],
            content: None,
            encrypted_content: enc.map(str::to_string),
            signature: sig.map(str::to_string),
        }
    }

    #[test]
    fn chat_strips_encrypted_and_signature() {
        let items = vec![reasoning_with(Some("E"), Some("S"))];
        let out = sanitize_history_for_wire_api(WireApi::Chat, &items);
        match &out[0] {
            ResponseItem::Reasoning { encrypted_content, signature, summary, .. } => {
                assert!(encrypted_content.is_none());
                assert!(signature.is_none());
                assert_eq!(summary.len(), 1); // 文本保留
            }
            _ => panic!("expected reasoning"),
        }
    }

    #[test]
    fn responses_drops_signature_keeps_encrypted() {
        let items = vec![reasoning_with(Some("E"), Some("S"))];
        let out = sanitize_history_for_wire_api(WireApi::Responses, &items);
        match &out[0] {
            ResponseItem::Reasoning { encrypted_content, signature, .. } => {
                assert_eq!(encrypted_content.as_deref(), Some("E"));
                assert!(signature.is_none());
            }
            _ => panic!("expected reasoning"),
        }
    }

    #[test]
    fn anthropic_preserves_pair() {
        let items = vec![reasoning_with(Some("E"), Some("S"))];
        let out = sanitize_history_for_wire_api(WireApi::Anthropic, &items);
        match &out[0] {
            ResponseItem::Reasoning { encrypted_content, signature, .. } => {
                assert_eq!(encrypted_content.as_deref(), Some("E"));
                assert_eq!(signature.as_deref(), Some("S"));
            }
            _ => panic!("expected reasoning"),
        }
    }

    #[test]
    fn non_reasoning_items_untouched() {
        let items = vec![ResponseItem::Message {
            id: None,
            role: "user".into(),
            content: vec![],
        }];
        let out = sanitize_history_for_wire_api(WireApi::Chat, &items);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], ResponseItem::Message { .. }));
    }
}
```

> `ResponseItem::Message` 的实际字段以 `protocol/src/models.rs` 为准（用 `cargo check` 校正构造）。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p agere-core history_sanitize`
Expected: FAIL（函数未定义）。

- [ ] **Step 3: 实现**

`core/src/history_sanitize.rs` 顶部：

```rust
use agere_model_provider_info::WireApi;
use agere_protocol::models::ResponseItem;

/// 返回对目标协议安全的历史副本。仅在跨协议会导致畸形或语义错配时调整 `Reasoning` 项；
/// 其余项原样保留。各协议 translator 仍负责最终映射，本函数提供集中、可测的前置规整。
pub(crate) fn sanitize_history_for_wire_api(
    wire_api: WireApi,
    items: &[ResponseItem],
) -> Vec<ResponseItem> {
    items
        .iter()
        .map(|item| match item {
            ResponseItem::Reasoning {
                id,
                summary,
                content,
                encrypted_content,
                signature,
            } => {
                let (encrypted_content, signature) = match wire_api {
                    // Chat 无推理槽：丢弃加密内容与签名，仅留文本（summary/content）。
                    WireApi::Chat => {
                        if encrypted_content.is_some() {
                            tracing::warn!(
                                "sanitize_history: dropping encrypted reasoning for Chat wire_api"
                            );
                        }
                        (None, None)
                    }
                    // Responses 不消费 Anthropic 签名：去签名，保留加密内容。
                    WireApi::Responses => (encrypted_content.clone(), None),
                    // Anthropic 需要配对：原样保留。
                    WireApi::Anthropic => (encrypted_content.clone(), signature.clone()),
                };
                ResponseItem::Reasoning {
                    id: id.clone(),
                    summary: summary.clone(),
                    content: content.clone(),
                    encrypted_content,
                    signature,
                }
            }
            other => other.clone(),
        })
        .collect()
}
```

`core/src/lib.rs` 增加：`mod history_sanitize;`（可见性按需 `pub(crate)`）。

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p agere-core history_sanitize`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add core/src/history_sanitize.rs core/src/lib.rs
git commit -m "feat(core): add per-wire-api history sanitization"
```

---

### Task 9: 在 `stream()` 应用清洗 seam + 日志升级

**Files:**
- Modify: `core/src/client.rs:1734-1807`（`stream()`）
- Modify: `anthropic-client/src/translate/request.rs:129-132,161-171`
- Modify: `openai-chat-client/src/translate/request.rs:178`
- Test: `core/src/client.rs`（`#[cfg(test)]`，或集成测试断言请求体合法）

- [ ] **Step 1: 在 `stream()` 分发前清洗**

`core/src/client.rs` `stream()` 开头（取得 `wire_api` 后）构造一个清洗后的 `Prompt`：

```rust
        let wire_api = self.client.provider().info().wire_api;
        // 跨协议切换后，按目标协议规整历史，避免发出畸形/错配请求。
        let sanitized_input =
            crate::history_sanitize::sanitize_history_for_wire_api(wire_api, &prompt.input);
        let mut sanitized_prompt = prompt.clone();
        sanitized_prompt.input = sanitized_input;
        let prompt = &sanitized_prompt;
        match wire_api {
            WireApi::Responses => { /* 原有分支不变 */ }
            ...
        }
```

> 确认 `Prompt: Clone`（`client_common.rs` 的 `Prompt` 需 `#[derive(Clone)]`；若无则给它加 Clone，或只 clone `input` 并在各分支传 `&sanitized_input`——优先给 `Prompt` 加 `Clone`，影响最小）。`prompt` 原为 `&Prompt`，此处 shadow 为指向本地 `sanitized_prompt` 的引用，生命周期覆盖整个 match。

- [ ] **Step 2: 日志升级 debug→warn**

- `anthropic-client/src/translate/request.rs:129-132`（drop RedactedThinking）与 `161-171`（reasoning→Text 降级）：把 `debug!` 改为 `warn!`。
- `openai-chat-client/src/translate/request.rs`：在 `ResponseItem::Reasoning` 分支，若（清洗后理论上不会再有，但作为纵深防御）检测到 `encrypted_content` 仍存在则 `warn!`。由于 Task 8 已在上游清空，这里仅保留既有文本转换 + 一行防御性 warn（可选）。

- [ ] **Step 3: 写测试（清洗后跨协议不 panic 且字段正确）**

集成层面：构造含 reasoning(enc+sig) + function_call/function_output 的 `Prompt.input`，对每个 `WireApi` 调用对应 translator（`build_anthropic_messages_from_response_items` / `build_chat_messages_from_response_items` / `build_responses_request` 的 input 组装），断言不 panic 且：
- Chat：无 RedactedThinking/Thinking，仅文本 assistant。
- Anthropic：enc+sig → RedactedThinking 保留。
- Responses：input 中 reasoning 项 signature 为空。

可放在 `core/src/client.rs` 的 `#[cfg(test)]`，先用 `sanitize_history_for_wire_api` 规整再喂给 translator（验证组合行为）。

- [ ] **Step 4: 运行 + 回归**

Run: `cargo test -p agere-core client && cargo test -p agere-anthropic-client && cargo test -p agere-openai-chat-client`
Expected: PASS。

- [ ] **Step 5: 提交**

```bash
git add core/src/client.rs anthropic-client/src/translate/request.rs openai-chat-client/src/translate/request.rs
git commit -m "feat: sanitize history per wire_api in stream() and surface reasoning drops"
```

---

## Phase E — 验证

### Task 10: 全量回归与文档

**Files:**
- Modify: `docs/superpowers/specs/2026-06-11-provider-switch-coherence-design.md`（如实现中有偏差，回填说明）

- [ ] **Step 1: 全 workspace 构建**

Run: `cargo build`
Expected: 通过。

- [ ] **Step 2: 目标 crate 测试**

Run: `cargo test -p agere-models-manager -p agere-core -p agere-app-server -p agere-model-provider-info`
Expected: 全绿，含本计划新增测试。

- [ ] **Step 3: 重点既有套件**

Run: `cargo test -p agere-core compact && cargo test -p agere-core session`
Expected: 全绿。

- [ ] **Step 4: clippy**

Run: `cargo clippy -p agere-core -p agere-models-manager --all-targets`
Expected: 无新增警告（`too_many_arguments` 已 allow）。

- [ ] **Step 5: 提交收尾**

```bash
git add -A
git commit -m "docs: reconcile provider-switch design with implementation"
```

---

## 自检对照

- 组件 1（上下文窗口根因）→ Task 1/2/3（wrapper + 每会话接线 + 切换重建 swap）。
- 组件 2（切换原子性）→ Task 6/7（Op 携带 provider + handler 同序施加 + app-server 移除即时施加）。
- 组件 3（降档 provider 感知）→ Task 4/5（PreviousTurnSettings 增字段 + 判定 provider 感知）。
- 组件 4（协议历史安全）→ Task 8/9（清洗函数 + stream() seam + 日志升级）。
- 组件 5（验证）→ 各 Task 内 TDD + Task 10 全量回归。

## 已知风险与校正点（实现时用 `cargo check` 逐一确认）

1. 多处导入路径/类型名（`CollaborationModeMask`、`ModelInfo`、`ModelsResponse`、`AuthManager`、`ModelConfig`）以实际 crate 为准。
2. `ModelProviderInfo` 放入 `Op` 需满足该 enum 的 derive 约束；不满足则退化为只携带 `provider_id` 并在 handler 解析（需评估 core 对 config 的依赖）。
3. `TurnContextItem` 可能缺 `model_provider_id`/`context_window`，Task 4 含补字段子步骤。
4. `Prompt` 需 `Clone`（Task 9）。
5. `model_context_window()` 整型类型与 `as i64` 转换按实际签名校正。
