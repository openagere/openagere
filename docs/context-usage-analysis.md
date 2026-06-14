 # Context Usage 统计全链路分析
 
 本文档从原点出发，详细分析 TUI 中 "Context 100% left, 231K used" 这类信息的完整统计逻辑，覆盖 **Anthropic**、**OpenAI Chat Completions**、**OpenAI Responses** 三种 API 类型的一致性与差异。
 
 ---
 
 ## 一、UI 显示入口
 
 ### 1.1 状态栏显示
 
 **文件**: `tui/src/chatwidget/status_surfaces.rs:521-526`
 
 ```rust
 StatusLineItem::ContextRemaining => self
     .status_line_context_remaining_percent()
     .map(|remaining| format!("Context {remaining}% left")),
 StatusLineItem::ContextUsed => self
     .status_line_context_used_percent()
     .map(|used| format!("Context {used}% used")),
 ```
 
 ### 1.2 底部栏 Context 标签
 
 **文件**: `tui/src/bottom_pane/footer.rs:974-985`
 
 ```rust
 pub(crate) fn context_window_line(percent: Option<i64>, used_tokens: Option<i64>) -> Line<'static> {
     if let Some(percent) = percent {
         let percent = percent.clamp(0, 100);
         return Line::from(vec![Span::from(format!("{percent}% context left")).dim()]);
     }
     if let Some(tokens) = used_tokens {
         let used_fmt = format_tokens_compact(tokens);
         return Line::from(vec![Span::from(format!("{used_fmt} used")).dim()]);
     }
     Line::from(vec![Span::from("100% context left").dim()])
 }
 ```
 
 ### 1.3 /status 命令卡片
 
 **文件**: `tui/src/status/card.rs:336-346`
 
 ```rust
 fn context_window_spans(&self) -> Option<Vec<Span<'static>>> {
     let context = self.token_usage.context_window.as_ref()?;
     let percent = context.percent_remaining;
     let used_fmt = format_tokens_compact(context.tokens_in_context);
     let window_fmt = format_tokens_compact(context.window);
 
     Some(vec![
         Span::from(format!("{percent}% left")),
         Span::from(" (").dim(),
         Span::from(used_fmt).dim(),
         Span::from(" used / ").dim(),
         Span::from(window_fmt).dim(),
         Span::from(")").dim(),
     ])
 }
 ```
 
 ---
 
 ## 二、核心数据结构
 
 ### 2.1 TokenUsage 结构体
 
 **文件**: `protocol/src/protocol.rs:1822-1833`
 
 ```rust
 pub struct TokenUsage {
     pub input_tokens: i64,
     pub cached_input_tokens: i64,
     pub output_tokens: i64,
     pub reasoning_output_tokens: i64,
     pub total_tokens: i64,
 }
 ```
 
 ### 2.2 TokenUsageInfo 结构体
 
 **文件**: `protocol/src/protocol.rs:1836-1842`
 
 ```rust
 pub struct TokenUsageInfo {
     pub total_token_usage: TokenUsage,
     pub last_token_usage: TokenUsage,
     pub model_context_window: Option<i64>,
 }
 ```
 
 关键字段说明：
 
 | 字段 | 用途 | 说明 |
 |------|------|------|
 | `last_token_usage` | 计算 Context % left | 状态栏百分比基于最近一次响应 |
 | `total_token_usage` | 显示累计总用量 | 卡片中的累积用量 |
 | `model_context_window` | 分母 | 模型最大上下文窗口 |
 
 ### 2.3 累积逻辑
 
 **文件**: `protocol/src/protocol.rs:1958-1964`
 
 ```rust
 pub fn add_assign(&mut self, other: &TokenUsage) {
     self.input_tokens += other.input_tokens;
     self.cached_input_tokens += other.cached_input_tokens;
     self.output_tokens += other.output_tokens;
     self.reasoning_output_tokens += other.reasoning_output_tokens;
     self.total_tokens += other.total_tokens;
 }
 ```
 
 **文件**: `protocol/src/protocol.rs:1863-1865`
 
 ```rust
 pub fn append_last_usage(&mut self, last: &TokenUsage) {
     self.total_token_usage.add_assign(last);
     self.last_token_usage = last.clone();
 }
 ```
 
 ---
 
 ## 三、Context Window 来源
 
 ### 3.1 模型元数据定义
 
 **文件**: `protocol/src/openai_models.rs:313-325`
 
 ```rust
 pub struct ModelInfo {
     pub context_window: Option<i64>,
     pub max_context_window: Option<i64>,
     pub effective_context_window_percent: i64,
 }
 ```
 
 ### 3.2 Context Window 解析
 
 **文件**: `protocol/src/openai_models.rs:340-342`
 
 ```rust
 pub fn resolved_context_window(&self) -> Option<i64> {
     self.context_window.or(self.max_context_window)
 }
 ```
 
 解析规则：优先取 `context_window`，没有则降级到 `max_context_window`。
 
 ### 3.3 有效窗口计算
 
 **文件**: `core/src/session/turn_context.rs:122-129`
 
 ```rust
 pub(crate) fn model_context_window(&self) -> Option<i64> {
     let effective_percent = self.model_info.effective_context_window_percent;
     self.model_info
         .resolved_context_window()
         .map(|context_window| {
             context_window.saturating_mul(effective_percent) / 100
         })
 }
 ```
 
 实际效果：如果模型 `context_window = 200000`，`effective_context_window_percent = 95`，那么实际用于百分比计算的窗口为 `200000 x 95% = 190000`。预留 5% 是为了给系统提示、工具指令和模型输出留余量。
 
 ### 3.4 默认百分比
 
 **文件**: `protocol/src/openai_models.rs:276-278`
 
 ```rust
 const fn default_effective_context_window_percent() -> i64 {
     95
 }
 ```
 
 示例：`models-manager/models.json` 中模型定义
 
 ```json
 {
   "context_window": 272000,
   "max_context_window": 272000,
   "effective_context_window_percent": 95
 }
 ```
 
 ---
 
 ## 四、百分比计算核心逻辑
 
 ### 4.1 BASELINE_TOKENS 常量
 
 **文件**: `protocol/src/protocol.rs:1950`
 
 ```rust
 // 包含 prompts、tools 和 compact 所需的空间
 const BASELINE_TOKENS: i64 = 12000;
 ```
 
 作用：从上下文窗口和已用 token 中扣除系统固定开销（系统提示词、工具描述等），使百分比反映用户可影响的部分。
 
 ### 4.2 剩余百分比计算
 
 **文件**: `protocol/src/protocol.rs:1984-1995`
 
 ```rust
 pub fn percent_of_context_window_remaining(&self, context_window: i64) -> i64 {
     if context_window <= BASELINE_TOKENS {
         return 0;
     }
 
     let effective_window = context_window - BASELINE_TOKENS;
     let used = (self.tokens_in_context_window() - BASELINE_TOKENS).max(0);
     let remaining = (effective_window - used).max(0);
     ((remaining as f64 / effective_window as f64) * 100.0)
         .clamp(0.0, 100.0)
         .round() as i64
 }
 ```
 
 公式拆解：
 
 ```
 effective_window = context_window - 12000
 used = total_tokens - 12000（最小为 0）
 remaining = effective_window - used（最小为 0）
 percent = (remaining / effective_window) * 100
 ```
 
 设计意图：
 
 - 第一轮对话后，如果 `total_tokens` 接近 12000（系统开销），UI 显示接近 100% left
 - 随着对话增长，百分比逐渐向 0% 趋近
 - 用户看到的百分比变化更敏感，而非从一开始就显示很低的百分比
 
 ### 4.3 已用百分比
 
 **文件**: `tui/src/chatwidget.rs:8417-8420`
 
 ```rust
 fn status_line_context_used_percent(&self) -> Option<i64> {
     let remaining = self.status_line_context_remaining_percent().unwrap_or(100);
     Some((100 - remaining).clamp(0, 100))
 }
 ```
 
 逻辑：直接 `100 - remaining`，无独立计算。
 
 ### 4.4 使用的 TokenUsage 版本
 
 **文件**: `tui/src/chatwidget.rs:8400-8416`
 
 ```rust
 fn status_line_context_remaining_percent(&self) -> Option<i64> {
     let Some(context_window) = self.status_line_context_window_size() else {
         return Some(100);
     };
     let default_usage = TokenUsage::default();
     let usage = self
         .token_info
         .as_ref()
         .map(|info| &info.last_token_usage)
         .unwrap_or(&default_usage);
     Some(usage.percent_of_context_window_remaining(context_window).clamp(0, 100))
 }
 ```
 
 关键点：状态栏的 Context % 使用 `last_token_usage`（最近一次 API 响应的用量），而非 `total_token_usage`。`total_token_usage` 通过 `add_assign` 累加所有轮次，`last_token_usage` 保存的是最近一次响应的值。
 
 再看 `append_last_usage`：
 
 ```rust
 pub fn append_last_usage(&mut self, last: &TokenUsage) {
     self.total_token_usage.add_assign(last);
     self.last_token_usage = last.clone();
 }
 ```
 
 `last_token_usage` 实际是单次 API 响应的用量，不是跨轮次累积值。但上下文中实际 token 数还需加上本地新增项的估算。
 
 **文件**: `core/src/context_manager/history.rs:309-326`
 
 ```rust
 pub(crate) fn get_total_token_usage(&self, server_reasoning_included: bool) -> i64 {
     let last_tokens = self
         .token_info
         .as_ref()
         .map(|info| info.last_token_usage.total_tokens)
         .unwrap_or(0);
     let items_after_last_model_generated_tokens = self
         .items_after_last_model_generated_item()
         .iter()
         .map(estimate_item_token_count)
         .fold(0i64, i64::saturating_add);
     if server_reasoning_included {
         last_tokens.saturating_add(items_after_last_model_generated_tokens)
     } else {
         last_tokens
             .saturating_add(self.get_non_last_reasoning_items_tokens())
             .saturating_add(items_after_last_model_generated_tokens)
     }
 }
 ```
 
 这说明上下文中实际 token 数 = API 返回的 last_tokens + 本地新增项的估算 token。
 
 但在 UI 百分比计算中，直接使用 `last_token_usage.total_tokens`，这是 API 端返回的累积值（OpenAI Responses API 的 Responses 会维护会话状态，Anthropic/Chat 每次请求也会带上完整历史）。
 
 ---
 
 ## 五、三种 API 的 Token 映射
 
 三种 API 都将其响应中的 usage 字段映射到统一的 `TokenUsage` 结构体，但细节有差异。
 
 ### 5.1 Anthropic API
 
 **文件**: `anthropic-client/src/translate/response.rs:297-308`
 
 ```rust
 pub(crate) fn map_usage(usage: &UsageInfo) -> TokenUsage {
     let input = usage.input_tokens.unwrap_or(0);
     let output = usage.output_tokens.unwrap_or(0);
     let cached = usage.cache_creation_input_tokens.unwrap_or(0) 
                + usage.cache_read_input_tokens.unwrap_or(0);
     TokenUsage {
         input_tokens: input,
         cached_input_tokens: cached,
         output_tokens: output,
         reasoning_output_tokens: 0,
         total_tokens: input + output,
     }
 }
 ```
 
 Anthropic API 响应示例：
 
 ```json
 {
   "usage": {
     "input_tokens": 1500,
     "output_tokens": 300,
     "cache_creation_input_tokens": 100,
     "cache_read_input_tokens": 800
   }
 }
 ```
 
 映射结果：
 
 - `input_tokens` = 1500
 - `cached_input_tokens` = 100 + 800 = 900
 - `output_tokens` = 300
 - `reasoning_output_tokens` = 0
 - `total_tokens` = 1500 + 300 = 1800
 
 特点：
 
 - `total_tokens` 是手动相加的（input + output）
 - `reasoning_output_tokens` 始终为 0（Anthropic API 不单独返回推理 token 计数）
 - 缓存 token 分两类：cache_creation（写入缓存）和 cache_read（命中缓存），两者都计入 `cached_input_tokens`
 
 ### 5.2 OpenAI Chat Completions API
 
 **文件**: `openai-chat-client/src/translate/response.rs:304-319`
 
 ```rust
 pub(crate) fn map_usage(usage: &ChatUsage) -> TokenUsage {
     let prompt = usage.prompt_tokens.unwrap_or(0);
     let completion = usage.completion_tokens.unwrap_or(0);
     let cached = usage
         .prompt_tokens_details
         .as_ref()
         .and_then(|d| d.cached_tokens)
         .unwrap_or(0);
     let total = usage.total_tokens.unwrap_or(prompt + completion);
     TokenUsage {
         input_tokens: prompt,
         cached_input_tokens: cached,
         output_tokens: completion,
         reasoning_output_tokens: 0,
         total_tokens: total,
     }
 }
 ```
 
 Chat API 响应示例：
 
 ```json
 {
   "usage": {
     "prompt_tokens": 1500,
     "completion_tokens": 300,
     "total_tokens": 1800,
     "prompt_tokens_details": {
       "cached_tokens": 900
     }
   }
 }
 ```
 
 映射结果：
 
 - `input_tokens` = 1500
 - `cached_input_tokens` = 900
 - `output_tokens` = 300
 - `reasoning_output_tokens` = 0
 - `total_tokens` = 1800（优先取 API 值，缺省则 prompt + completion）
 
 特点：
 
 - `total_tokens` 优先使用 API 返回值，缺省时 fallback 为 prompt + completion
 - `reasoning_output_tokens` 始终为 0（即使 o1/o3 等推理模型的 thinking token 也不会在此字段体现）
 - 缓存 token 来自 `prompt_tokens_details.cached_tokens`
 
 ### 5.3 OpenAI Responses API
 
 **文件**: `agere-api/src/sse/responses.rs:141-167`
 
 ```rust
 struct ResponseCompletedUsage {
     input_tokens: i64,
     input_tokens_details: Option<ResponseCompletedInputTokensDetails>,
     output_tokens: i64,
     output_tokens_details: Option<ResponseCompletedOutputTokensDetails>,
     total_tokens: i64,
 }
 
 impl From<ResponseCompletedUsage> for TokenUsage {
     fn from(val: ResponseCompletedUsage) -> Self {
         TokenUsage {
             input_tokens: val.input_tokens,
             cached_input_tokens: val
                 .input_tokens_details
                 .map(|d| d.cached_tokens)
                 .unwrap_or(0),
             output_tokens: val.output_tokens,
             reasoning_output_tokens: val
                 .output_tokens_details
                 .map(|d| d.reasoning_tokens)
                 .unwrap_or(0),
             total_tokens: val.total_tokens,
         }
     }
 }
 ```
 
 Responses API 响应示例：
 
 ```json
 {
   "usage": {
     "input_tokens": 1500,
     "output_tokens": 500,
     "total_tokens": 2000,
     "input_tokens_details": {
       "cached_tokens": 900
     },
     "output_tokens_details": {
       "reasoning_tokens": 200
     }
   }
 }
 ```
 
 映射结果：
 
 - `input_tokens` = 1500
 - `cached_input_tokens` = 900
 - `output_tokens` = 500
 - `reasoning_output_tokens` = 200
 - `total_tokens` = 2000
 
 特点：
 
 - 唯一能捕获 `reasoning_output_tokens` 的 API 类型
 - `total_tokens` 直接取自 API 响应（API 已经包含了推理 token 在 total 中）
 - `output_tokens` 通常包含 reasoning tokens（即 output_tokens = text_tokens + reasoning_tokens）
 
 ---
 
 ## 六、三种 API 的差异对比
 
 ### 6.1 总览表
 
 | 对比维度 | Anthropic | Chat Completions | Responses API |
 |----------|-----------|------------------|---------------|
 | 输入 token 字段名 | input_tokens | prompt_tokens | input_tokens |
 | 输出 token 字段名 | output_tokens | completion_tokens | output_tokens |
 | total_tokens 来源 | 手动计算 input + output | API 值，fallback prompt + completion | 直接取 API 值 |
 | 缓存 token | cache_creation + cache_read | prompt_tokens_details.cached_tokens | input_tokens_details.cached_tokens |
 | reasoning_output_tokens | 始终 0 | 始终 0 | 从 API 捕获 |
 | reasoning 是否在 total 中 | 否（Anthropic 不区分） | 可能（取决于模型） | 是（API 的 total 包含） |
 
 ### 6.2 关键差异详解
 
 #### 差异 1: reasoning_output_tokens
 
 这是最显著的差异。只有 Responses API 能捕获推理 token 的数量。
 
 影响：
 
 - 使用 Anthropic 或 Chat API 调用 Claude/o1 等推理模型时，`reasoning_output_tokens` 字段始终为 0
 - 但 `total_tokens` 和 `output_tokens` 中可能已经包含了推理 token（取决于 API 端行为）
 - 这意味着 Context % 计算在三种 API 下是一致的（都基于 total_tokens），只是 Responses API 能额外区分出推理 token 的数量
 
 #### 差异 2: total_tokens 的计算方式
 
 | API | total_tokens 计算 | 是否可能遗漏 |
 |-----|-------------------|-------------|
 | Anthropic | input + output 手动相加 | 否（两个值都来自 API） |
 | Chat | 优先 API 的 total_tokens，无则 prompt + completion | 极低（OpenAI 总是返回 total） |
 | Responses | 直接取 API 的 total_tokens | 否 |
 
 结论：三种方式得到的 total_tokens 在正常情况下一致。
 
 #### 差异 3: 缓存 token 的拆分
 
 | API | 缓存字段 | 含义 |
 |-----|---------|------|
 | Anthropic | cache_creation + cache_read | 区分写入缓存和命中缓存 |
 | Chat | prompt_tokens_details.cached_tokens | 合并为单一值 |
 | Responses | input_tokens_details.cached_tokens | 合并为单一值 |
 
 影响：仅影响缓存命中率的统计展示，不影响 Context % 计算。
 
 ---
 
 ## 七、完整数据流追踪
 
 ```
 +-----------------------------------------------+
 |  1. API 响应 (SSE / HTTP)                      |
 |     Anthropic / Chat / Responses               |
 +----------------------+------------------------+
                        v
 +-----------------------------------------------+
 |  2. API Client 层: map_usage()                 |
 |     - anthropic-client/.../response.rs:297    |
 |     - openai-chat-client/.../response.rs:304  |
 |     - agere-api/.../responses.rs:151 (From)   |
 |                                                 |
 |     输出: TokenUsage { input, output, total }  |
 +----------------------+------------------------+
                        v
 +-----------------------------------------------+
 |  3. ResponseEvent::Completed                   |
 |     包含: token_usage: Option<TokenUsage>      |
 +----------------------+------------------------+
                        v
 +-----------------------------------------------+
 |  4. Core Session 事件处理                      |
 |     core/src/session/turn.rs                  |
 |     sess.update_token_usage_info()            |
 +----------------------+------------------------+
                        v
 +-----------------------------------------------+
 |  5. Session State 更新                         |
 |     core/src/session/mod.rs:2793-2798         |
 |     state.update_token_info_from_usage(       |
 |         token_usage,                           |
 |         turn_context.model_context_window()    |
 |     )                                          |
 +----------------------+------------------------+
                        v
 +-----------------------------------------------+
 |  6. ContextManager 更新                        |
 |     core/src/context_manager/history.rs:262   |
 |     TokenUsageInfo::new_or_append()           |
 |                                                 |
 |     - total_token_usage += last (累加)         |
 |     - last_token_usage = last (替换)           |
 +----------------------+------------------------+
                        v
 +-----------------------------------------------+
 |  7. TUI 获取 TokenUsageInfo                    |
 |     tui/src/chatwidget.rs                     |
 |     self.token_info.as_ref()                  |
 +----------------------+------------------------+
                        v
 +-----------------------------------------------+
 |  8. 百分比计算                                 |
 |     last_token_usage                          |
 |       .percent_of_context_window_remaining(   |
 |           model_context_window                 |
 |       )                                        |
 |                                                 |
 |     公式:                                      |
 |     effective_window = window - 12000          |
 |     used = total_tokens - 12000                |
 |     remaining = effective_window - used        |
 |     percent = (remaining / effective_window)   |
 |               * 100                            |
 +----------------------+------------------------+
                        v
 +-----------------------------------------------+
 |  9. UI 渲染                                    |
 |     状态栏: "Context 95% left"                |
 |     状态栏: "231K used"                       |
 |     /status: "95% left (12.5K used / 200K)"   |
 +-----------------------------------------------+
 ```
 
 ---
 
 ## 八、Context Manager 中的 Token 估算补充
 
 除了 API 返回的 TokenUsage，Context Manager 还会对本地新增的 items 进行 token 估算。
 
 **文件**: `core/src/context_manager/history.rs:309-326`
 
 ```rust
 pub(crate) fn get_total_token_usage(&self, server_reasoning_included: bool) -> i64 {
     let last_tokens = self.token_info
         .as_ref()
         .map(|info| info.last_token_usage.total_tokens)
         .unwrap_or(0);
 
     // 最近一次 API 响应之后本地新增的 items（如工具执行结果）
     let items_after_last = self.items_after_last_model_generated_item()
         .iter()
         .map(estimate_item_token_count)
         .fold(0i64, i64::saturating_add);
 
     if server_reasoning_included {
         last_tokens + items_after_last
     } else {
         last_tokens 
             + 非最后一轮的 reasoning items 估算
             + items_after_last
     }
 }
 ```
 
 说明：这个方法主要用于 compaction（上下文压缩）决策，不直接用于 UI 的 Context % 显示。UI 显示直接使用 `last_token_usage.total_tokens`。
 
 ---
 
 ## 九、总结
 
 ### 9.1 一致性结论
 
 三种 API 在 Context % 计算上完全一致，因为：
 
 1. 所有 API 的 usage 都映射到同一个 `TokenUsage` 结构体
 2. 百分比计算使用统一的 `percent_of_context_window_remaining()` 方法
 3. 分母（context window）来源相同，都经过 `effective_context_window_percent` 缩放
 4. 都使用 `BASELINE_TOKENS = 12000` 作为系统开销扣除
 
 ### 9.2 差异点总结
 
 | 差异 | 影响范围 | 是否影响 Context % |
 |------|---------|-------------------|
 | reasoning_output_tokens 仅 Responses API 捕获 | 推理 token 的细分统计 | 否（total 已包含） |
 | total_tokens 来源方式不同 | 极端情况下可能差几个 token | 几乎不影响 |
 | 缓存 token 拆分粒度不同 | 缓存命中率展示 | 不影响 |
 
 ### 9.3 关键设计要点
 
 1. BASELINE_TOKENS (12000): 让百分比反映用户可控部分，避免第一轮就显示低百分比
 2. effective_context_window_percent (95%): 预留 5% 给系统输出和缓冲
 3. last_token_usage vs total_token_usage: 百分比用 last（单次 API 响应的累积上下文），卡片总用量用 total（所有轮次之和）
 4. reasoning_output_tokens 差异: 仅 Responses API 捕获，不影响 Context % 计算但影响内部统计的精细度
 
 ### 9.4 数字示例
 
 假设使用 Claude 模型，`context_window = 200000`，`effective_context_window_percent = 95`：
 
 ```
 实际可用窗口 = 200000 x 0.95 = 190000
 有效分母 = 190000 - 12000 = 178000
 
 第 1 轮后: total_tokens = 15000
   已用 = 15000 - 12000 = 3000
   剩余 = 178000 - 3000 = 175000
   百分比 = 175000 / 178000 x 100 = 98% left
 
 第 10 轮后: total_tokens = 100000
   已用 = 100000 - 12000 = 88000
   剩余 = 178000 - 88000 = 90000
   百分比 = 90000 / 178000 x 100 = 51% left
 
 第 20 轮后: total_tokens = 185000
   已用 = 185000 - 12000 = 173000
   剩余 = 178000 - 173000 = 5000
   百分比 = 5000 / 178000 x 100 = 3% left
 ```
