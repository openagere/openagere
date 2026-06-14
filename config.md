# OpenRouter Provider 配置指南

## 快速开始

在 `~/.openagere/config.toml` 中配置。所有字段均为**顶层字段**，不要放在自定义段下。

## 两种工作模式

本项目通过 `wire_api` 字段选择协议格式，决定了请求发到 OpenRouter 的哪个端点。

### 模式一：`responses`（OpenAI 兼容格式）— 推荐

发送请求到 `{base_url}/responses`，使用 OpenAI Responses API 格式。

```toml
model = "tencent/hy3-preview:free"
model_provider = "openrouter"

[model_providers.openrouter]
name = "OpenRouter"
base_url = "https://openrouter.ai/api/v1"   # responses 模式可以带 /v1
wire_api = "responses"
experimental_bearer_token = "sk-or-v1-你的key"
stream_idle_timeout_ms = 30000
request_max_retries = 3
```

**适用场景：** OpenRouter 上的所有模型（包括非 Claude 模型），原生接口，兼容性最好。

### 模式二：`anthropic`（Anthropic Messages API 格式）

发送请求到 `{base_url}/v1/messages`，使用 Anthropic Messages API SSE 格式。

```toml
model = "anthropic/claude-sonnet-4-5-20250514"
model_provider = "openrouter"

[model_providers.openrouter]
name = "OpenRouter (Anthropic)"
base_url = "https://openrouter.ai/api"       # 注意：不能带 /v1！
wire_api = "anthropic"
experimental_bearer_token = "sk-or-v1-你的key"
stream_idle_timeout_ms = 30000
request_max_retries = 3
```

**适用场景：** 仅推荐用于真正的 Claude 模型（如 `anthropic/claude-*`）。

---

## 注意事项

### 1. `base_url` 陷阱（最常见的错误）

Anthropic 模式内部硬编码了路径 `v1/messages`（`anthropic-client/src/client.rs:156`），拼接规则是 `{base_url}/{path}`：

| base_url | 最终请求路径 | 结果 |
|----------|-------------|------|
| `https://openrouter.ai/api/v1` | `https://openrouter.ai/api/v1/v1/messages` | ❌ 404，流立即关闭 |
| `https://openrouter.ai/api` | `https://openrouter.ai/api/v1/messages` | ✅ 正确 |

**规则：`anthropic` 模式的 `base_url` 不要带 `/v1` 后缀，只写到 `/api` 即可。**

Responses 模式没有这个问题，因为它拼的路径是 `responses`，没有 `/v1` 前缀。

### 2. `model` 和 `model_provider` 必须是顶层字段

错误写法（放在自定义段下，会被忽略）：
```toml
[anthropic]          # ❌ ConfigToml 没有 anthropic 字段，整个段被忽略
model = "..."
```

正确写法：
```toml
model = "..."                    # ✅ 顶层
model_provider = "openrouter"    # ✅ 顶层
```

### 3. API Key 的推荐写法

`experimental_bearer_token` 将 key 明文写入配置文件，安全风险较高。推荐改用环境变量：

```toml
[model_providers.openrouter]
name = "OpenRouter"
base_url = "https://openrouter.ai/api/v1"
wire_api = "responses"
env_key = "OPENROUTER_API_KEY"   # ✅ 从环境变量读取
```

然后在环境中设置：
```bash
export OPENROUTER_API_KEY="sk-or-v1-你的key"
```

### 4. Model metadata not found 警告

使用 OpenRouter 上的模型（如 `tencent/hy3-preview:free`）时，会出现：
```
⚠ Model metadata for `tencent/hy3-preview:free` not found.
Defaulting to fallback metadata; this can degrade performance and cause issues.
```

这是**警告而非错误**，不影响运行。代码在内置模型目录中找不到该模型，会使用 fallback 值（context window、输出 token 上限等）。可通过 `model_catalog_json` 字段加载自定义模型目录来消除。

### 5. `stream closed before message_delta` 错误

这是流在收到任何 SSE 事件前就关闭了。常见原因：
- `base_url` 多写了 `/v1`（见第 1 条）
- OpenRouter 对非 Claude 模型的 Anthropic 翻译层存在 bug
- 模型推理超时或免费模型排队导致连接断开

**排查方法：** 先切换到 `responses` 模式确认链路是否通，排除密钥和网络问题。

---

## 完整字段参考

`ModelProviderInfo` 支持的字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | String | 显示名称（必填） |
| `base_url` | String | API 基础 URL（必填） |
| `wire_api` | String | `"responses"` 或 `"anthropic"`（必填） |
| `experimental_bearer_token` | String | 明文 API key |
| `env_key` | String | 环境变量名（与 bearer_token 二选一） |
| `stream_idle_timeout_ms` | Number | 流空闲超时（毫秒），默认 300000 |
| `request_max_retries` | Number | 请求最大重试次数，默认 4 |
| `stream_max_retries` | Number | 流重连最大次数，默认 5 |
| `query_params` | Object | 附加查询参数 |
| `http_headers` | Object | 附加 HTTP 头 |
| `requires_provider_auth` | Boolean | 是否需要提供者侧认证登录，默认 false |
| `supports_websockets` | Boolean | 是否支持 WebSocket，默认 false |

---

## OpenRouter 端点说明

OpenRouter 作为代理层，提供两种端点：

| 端点 | 用途 | 兼容性 |
|------|------|--------|
| `/api/v1/responses` | OpenAI Responses API 格式 | 原生支持，所有模型都可用 |
| `/api/v1/messages` | Anthropic Messages API 格式 | 翻译层，仅 Claude 模型稳定 |

OpenRouter 的 `/v1/messages` 端点会将请求翻译为底层模型的格式。对于非 Claude 模型（如腾讯混元、DeepSeek），这个翻译层可能不完整或返回空流。

---

## 推荐配置方案

### 方案 A：通用 OpenRouter（推荐）

适合在 OpenRouter 上使用各种模型：

```toml
model = "tencent/hy3-preview:free"
model_provider = "openrouter"

[model_providers.openrouter]
name = "OpenRouter"
base_url = "https://openrouter.ai/api/v1"
wire_api = "responses"
env_key = "OPENROUTER_API_KEY"
```

### 方案 B：OpenRouter Anthropic

仅用于 Claude 模型：

```toml
model = "anthropic/claude-sonnet-4-5-20250514"
model_provider = "openrouter"

[model_providers.openrouter]
name = "OpenRouter (Anthropic)"
base_url = "https://openrouter.ai/api"
wire_api = "anthropic"
env_key = "OPENROUTER_API_KEY"
```

### 方案 C：直连 Anthropic 官方 API

```toml
model = "claude-sonnet-4-6"
model_provider = "anthropic-official"

[model_providers.anthropic-official]
name = "Anthropic Official"
base_url = "https://api.anthropic.com"
wire_api = "anthropic"
env_key = "ANTHROPIC_API_KEY"
```

### 方案 D：多 provider 切换

```toml
model = "tencent/hy3-preview:free"
model_provider = "openrouter-responses"

[model_providers.openrouter-responses]
name = "OpenRouter (Responses)"
base_url = "https://openrouter.ai/api/v1"
wire_api = "responses"
env_key = "OPENROUTER_API_KEY"

[model_providers.openrouter-anthropic]
name = "OpenRouter (Anthropic)"
base_url = "https://openrouter.ai/api"
wire_api = "anthropic"
env_key = "OPENROUTER_API_KEY"

[model_providers.anthropic-direct]
name = "Anthropic Direct"
base_url = "https://api.anthropic.com"
wire_api = "anthropic"
env_key = "ANTHROPIC_API_KEY"
```

切换时只需修改 `model_provider` 的值。

---

## 从 config.toml 到大模型 API 的完整请求链路

以如下配置为例，完整追踪从 `config.toml` 解析到 HTTP 请求发出的每一步：

```toml
model = "tencent/hy3-preview:free"
model_provider = "openrouter"

[model_providers.openrouter]
name = "OpenRouter (Anthropic)"
base_url = "https://openrouter.ai/api"
wire_api = "anthropic"
experimental_bearer_token = "sk-or-v1-xxx"
stream_idle_timeout_ms = 30000
request_max_retries = 3
```

### 第 1 步：TOML 解析 → `ConfigToml`

**文件：** `config/src/config_toml.rs:62-414`

`toml::from_str()` 将 config.toml 反序列化为 `ConfigToml` 结构体：
```rust
pub struct ConfigToml {
    pub model: Option<String>,               // "tencent/hy3-preview:free"
    pub model_provider: Option<String>,      // "openrouter"
    pub model_providers: HashMap<String, ModelProviderInfo>,  // {"openrouter": ...}
    pub stream_idle_timeout_ms: Option<u64>, // 30000
    // ... 100+ 个其他字段
}
```

`ModelProviderInfo` 的解析：
- `name = "OpenRouter (Anthropic)"` → `ModelProviderInfo.name`
- `base_url = "https://openrouter.ai/api"` → `ModelProviderInfo.base_url`
- `wire_api = "anthropic"` → `WireApi::Anthropic`（通过 `serde` 的自定义 `deserialize`）
- `experimental_bearer_token = "sk-or-v1-xxx"` → `ModelProviderInfo.experimental_bearer_token`
- `stream_idle_timeout_ms = 30000` → `ModelProviderInfo.stream_idle_timeout_ms`
- `request_max_retries = 3` → `ModelProviderInfo.request_max_retries`

### 第 2 步：Provider 合并 → 选择 `model_provider_id`

**文件：** `core/src/config/mod.rs:2227-2245`

```rust
// 合并内置 provider + 自定义 provider
let model_providers = merge_configured_model_providers(
    built_in_model_providers(openai_base_url),  // {"openai": ..., "amazon-bedrock": ...}
    cfg.model_providers                         // {"openrouter": ...}
)?;
// 结果: {"openai": ..., "amazon-bedrock": ..., "openrouter": ...}

// 选择 model_provider_id（按优先级取第一个非 None）
let model_provider_id = model_provider              // None（CLI 未指定）
    .or(config_profile.model_provider)              // None（profile 未指定）
    .or(cfg.model_provider)                         // Some("openrouter")
    .unwrap_or_else(|| "openai".to_string());       // 最终: "openrouter"

// 从 HashMap 中查找
let model_provider = model_providers.get(&model_provider_id).unwrap().clone();
// 此时 model_provider 是 ModelProviderInfo 类型
```

**内置 provider 合并规则：** `model-provider-info/src/lib.rs:428-459`
- 用户定义的 provider 通过 `entry(key).or_insert(provider)` 插入
- 不会覆盖内置 provider（`openai`、`amazon-bedrock`）
- `amazon-bedrock` 特殊处理：只允许覆盖 `aws.profile` 和 `aws.region`

### 第 3 步：`ModelProviderInfo` → 运行时 `SharedModelProvider`

**文件：** `model-provider/src/provider.rs:130-141`

```rust
pub fn create_model_provider(
    provider_info: ModelProviderInfo,  // "openrouter" 的 ModelProviderInfo
    auth_manager: Option<Arc<AuthManager>>,
) -> SharedModelProvider {
    if provider_info.is_anthropic() {  // wire_api == WireApi::Anthropic → true
        Arc::new(AnthropicModelProvider::new(provider_info))
    } else if provider_info.is_amazon_bedrock() {
        Arc::new(AmazonBedrockModelProvider::new(provider_info))
    } else {
        Arc::new(ConfiguredModelProvider::new(provider_info, auth_manager))
    }
}
```

因为 `wire_api = "anthropic"`，创建的是 `AnthropicModelProvider`。

### 第 4 步：`AnthropicModelProvider` 的 `api_auth()` 解析认证

**文件：** `model-provider/src/anthropic/mod.rs:102-108`

```rust
async fn api_auth(&self) -> Result<SharedAuthProvider> {
    let token = resolve_api_key(&self.info)?;  // 解析 API key
    let auth = AgereAuth::from_api_key(&token);
    Ok(auth_provider_from_auth(&auth))
}
```

**`resolve_api_key`（同上文件第 24-53 行）按优先级读取 key：**
1. 先尝试 `env_key` 环境变量：`std::env::var("OPENROUTER_API_KEY")` → 未设置
2. 再尝试 `experimental_bearer_token`：`"sk-or-v1-xxx"` → 找到，返回

**`auth_provider_from_auth`（`model-provider/src/auth.rs:61-67`）创建 `BearerAuthProvider`：**
```rust
pub fn auth_provider_from_auth(auth: &AgereAuth) -> SharedAuthProvider {
    Arc::new(BearerAuthProvider {
        token: auth.get_token().ok(),     // "sk-or-v1-xxx"
        account_id: auth.get_account_id(), // None
        is_fedramp_account: auth.is_fedramp_account(), // false
    })
}
```

**`resolve_provider_auth` 中的 `bearer_auth_for_provider` 优先级更高（`model-provider/src/auth.rs:46-58`）：**
```rust
fn bearer_auth_for_provider(provider: &ModelProviderInfo) -> Result<Option<BearerAuthProvider>> {
    if let Some(api_key) = provider.api_key()? {  // 先试 env_key → None
        return Ok(Some(BearerAuthProvider::new(api_key)));
    }
    if let Some(token) = provider.experimental_bearer_token.clone() {  // "sk-or-v1-xxx"
        return Ok(Some(BearerAuthProvider::new(token)));
    }
    Ok(None)
}
```

最终返回 `BearerAuthProvider { token: Some("sk-or-v1-xxx"), ... }`。

### 第 5 步：Session 初始化 → `ModelClientSession`

**文件：** `core/src/session/mod.rs:602` → `core/src/client.rs:312`

```rust
let session_configuration = SessionConfiguration {
    provider: config.model_provider.clone(),  // ModelProviderInfo
    model_info: model_info.clone(),           // 模型元数据
    // ...
};

// 在 ModelClient::new 中：
let model_provider = create_model_provider(provider_info, auth_manager);
// → SharedModelProvider = Arc<dyn ModelProvider>
```

### 第 6 步：发送请求 → `stream()` 调度

**文件：** `core/src/client.rs:1603-1665`

当用户发送消息触发推理时：

```rust
pub async fn stream(
    &mut self,
    prompt: &Prompt,
    model_info: &ModelInfo,
    session_telemetry: &SessionTelemetry,
    effort: Option<ReasoningEffortConfig>,
    summary: ReasoningSummaryConfig,
    service_tier: Option<ServiceTier>,
    turn_metadata_header: Option<&str>,
    inference_trace: &InferenceTraceContext,
) -> Result<ResponseStream> {
    let wire_api = self.client.state.provider.info().wire_api;  // WireApi::Anthropic
    match wire_api {
        WireApi::Responses => {
            self.stream_responses_api(...).await  // Responses 模式走这里
        }
        WireApi::Anthropic => {
            self.stream_anthropic(...).await      // Anthropic 模式走这里 ← 我们走这条
        }
    }
}
```

### 第 7 步：Anthropic 模式 → `stream_anthropic()`

**文件：** `core/src/client.rs:1514-1592`

```rust
async fn stream_anthropic(
    &self,
    prompt: &Prompt,
    model_info: &ModelInfo,
    session_telemetry: &SessionTelemetry,
    effort: Option<ReasoningEffortConfig>,
    summary: ReasoningSummaryConfig,
    inference_trace: &InferenceTraceContext,
) -> Result<ResponseStream> {
    let _ = summary;

    // 7.1 创建 HTTP 客户端
    let client_setup = self.client.current_client_setup().await?;
    // client_setup.api_provider = Provider { base_url: "https://openrouter.ai/api", ... }
    // client_setup.api_auth = BearerAuthProvider { token: "sk-or-v1-xxx", ... }
    let transport = ReqwestTransport::new(build_reqwest_client());

    // 7.2 构建 Anthropic 消息格式（将内部 ResponseItem 翻译为 Anthropic Messages 格式）
    let messages = build_anthropic_messages_from_response_items(&prompt.input);

    // 7.3 构建工具定义
    let tools_json = create_tools_json_for_responses_api(&prompt.tools)?;
    let tools: Vec<AnthropicToolDef> = tools_json.into_iter()
        .filter_map(...).collect();

    // 7.4 转换 reasoning effort
    let thinking_effort = effort.map(|e| { ... });  // None/Minimal/Low/Medium/High/XHigh

    // 7.5 构建默认 Anthropic 选项
    let system = &prompt.base_instructions.text;  // 系统提示
    let options = default_anthropic_options();    // max_tokens = 4096, compression = None
    let model = &model_info.slug;                 // "tencent/hy3-preview:free"

    // 7.6 创建 AnthropicClient
    let client = AnthropicClient::new(transport, client_setup.api_provider, client_setup.api_auth);

    // 7.7 发送请求
    let stream_result = client
        .stream_request_with_messages(model, system, messages, &tools, thinking_effort, options)
        .await;

    // 7.8 处理结果
    match stream_result {
        Ok(stream) => { ... Ok(stream) }
        Err(err) => { Err(map_api_error(err)) }
    }
}
```

### 第 8 步：`AnthropicClient.send_request()` — 构建 HTTP 请求

**文件：** `anthropic-client/src/client.rs:98-189`

#### 8.1 构建请求体 `MessagesRequest`

```rust
let request_body = MessagesRequest {
    model: "tencent/hy3-preview:free",      // 模型名
    messages: [...],                         // 消息列表
    system: Some(SystemPrompt::Text(...)),   // 系统提示
    max_tokens: 4096,                        // default_anthropic_options() 的值
    temperature: None,
    top_p: None,
    top_k: None,
    stop_sequences: None,
    thinking: to_anthropic_thinking(thinking_effort),  // None 或 ThinkingConfig
    tools: [...],                            // 工具列表
    tool_choice: None,
    stream: true,                            // 始终为 true（强制 SSE）
    metadata: None,
};
```

#### 8.2 构建 HTTP Headers

```rust
let mut headers = options.extra_headers;  // 空 HeaderMap

// 8.2.1 Anthropic 版本头
headers.insert("anthropic-version",
    HeaderValue::from_static("2023-06-01"));  // anthropic-client/src/config.rs:2

// 8.2.2 Beta 功能头
// 默认值: "prompt-caching-2024-07-31,token-efficient-tools-2025-02-19"
let beta = beta_header(&options.beta_features);  // 默认值拼接
headers.insert("anthropic-beta", HeaderValue::from_str(&beta)?);

// 8.2.3 Accept 头
headers.insert(
    http::header::ACCEPT,
    HeaderValue::from_static("text/event-stream"));
```

#### 8.3 构建 Request 对象

```rust
let mut req = self.provider.build_request(Method::POST, "v1/messages");
// provider.base_url = "https://openrouter.ai/api"
// path = "v1/messages"
// url_for_path 拼接: "https://openrouter.ai/api" + "/" + "v1/messages"
//                     = "https://openrouter.ai/api/v1/messages"
req.headers.extend(headers.clone());
req.body = Some(RequestBody::Json(body_json.clone()));
req.compression = compression;  // RequestCompression::None
```

**URL 拼接逻辑**（`agere-api/src/provider.rs:53-75`）：
```rust
pub fn url_for_path(&self, path: &str) -> String {
    let base = self.base_url.trim_end_matches('/');       // "https://openrouter.ai/api"
    let path = path.trim_start_matches('/');              // "v1/messages"
    let mut url = format!("{base}/{path}");               // "https://openrouter.ai/api/v1/messages"

    // 附加 query_params（如果有的话）
    if let Some(params) = &self.query_params && !params.is_empty() {
        url.push('?');
        url.push_str(&params.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join("&"));
    }
    url
}
```

### 第 9 步：重试循环 + 认证注入 + 传输层

**文件：** `anthropic-client/src/client.rs:163-183`

```rust
let retry_policy = self.provider.retry.to_policy();
// retry_policy = RetryPolicy {
//     max_attempts: 3,          // 来自 config request_max_retries
//     base_delay: 1s,
//     retry_429: true,
//     retry_5xx: true,
//     retry_transport: true,
// }

let stream_response = agere_client::run_with_retry(retry_policy, make_req, |req, attempt| {
    let auth = auth.clone();  // BearerAuthProvider { token: "sk-or-v1-xxx" }
    async move {
        // 9.1 注入认证头
        let req = auth.apply_auth(req).await?;
        // 在 req.headers 中添加: Authorization: Bearer sk-or-v1-xxx

        // 9.2 发送流请求
        transport.stream(req).await
    }
}).await?;
```

**`BearerAuthProvider.add_auth_headers`**（`model-provider/src/bearer_auth_provider.rs:32-46`）：
```rust
fn add_auth_headers(&self, headers: &mut HeaderMap) {
    if let Some(token) = self.token.as_ref() {
        headers.insert(
            http::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}"))?);  // "Bearer sk-or-v1-xxx"
    }
    // account_id 和 fedramp 都是 None/false，不添加
}
```

### 第 10 步：`ReqwestTransport.stream()` — 底层 HTTP 传输

**文件：** `agere-client/src/transport.rs:90-155`

```rust
impl HttpTransport for ReqwestTransport {
    async fn stream(&self, req: Request) -> Result<StreamResponse, TransportError> {
        let url = req.url.clone();  // "https://openrouter.ai/api/v1/messages"
        let builder = self.build(req)?;

        // 内部构建 reqwest::RequestBuilder:
        //   reqwest::Client.request(Method::POST, "https://openrouter.ai/api/v1/messages")
        //     .headers(headers)           // 所有 headers
        //     .json(body_json)            // JSON body
        //     .compression(compression)   // None

        let resp = builder.send().await.map_err(Self::map_error)?;
        // 最终调用: reqwest::RequestBuilder.send()

        // 返回流式响应
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = Box::pin(resp.bytes_stream()...);

        Ok(StreamResponse { status, headers, bytes })
    }
}
```

**`AgereRequestBuilder.send()`**（`agere-client/src/default_client.rs:113-141`）：
```rust
pub async fn send(self) -> Result<Response, reqwest::Error> {
    // 注入 OpenTelemetry 追踪头
    let headers = trace_headers();
    // 通过 opentelemetry propagator 注入 traceparent 等头

    // 最终的 reqwest 调用
    match self.builder.headers(headers).send().await {
        Ok(response) => { /* 记录日志 */ Ok(response) }
        Err(error) => { /* 记录日志 */ Err(error) }
    }
}
```

**`build_reqwest_client()`**（`login/src/auth/default_client.rs:211-224`）：
```rust
pub fn build_reqwest_client() -> reqwest::Client {
    let mut builder = reqwest::Client::builder()
        .default_headers(default_headers());
        // default_headers: User-Agent, originator, etc.
    // + SSL_CERT_FILE 自定义 CA 证书支持
    // + 沙盒环境 proxy 设置
    builder.build()
}
```

### 第 11 步：SSE 流解析

**文件：** `anthropic-client/src/sse.rs:17-85`

```rust
tokio::spawn(process_anthropic_sse(
    stream_response.bytes,  // ByteStream = BoxStream<'static, Result<Bytes, TransportError>>
    tx_event,               // mpsc channel → ResponseStream.rx_event
    self.provider.stream_idle_timeout,  // 30000ms
));
```

**SSE 解析流程：**
```rust
pub async fn process_anthropic_sse(stream: ByteStream, tx_event, idle_timeout: Duration) {
    let mut sse_stream = stream.eventsource();  // 解析 SSE 格式

    loop {
        // 超时控制
        let response = timeout(idle_timeout, sse_stream.next()).await;

        match response {
            Ok(Some(Ok(sse))) => {
                // 解析 SSE event JSON
                let event: SseEvent = serde_json::from_str(&sse.data)?;
                // 处理事件并翻译为 ResponseEvent
                match handle_sse_event(&event, &mut state) {
                    Some(Ok(response_event)) => {
                        tx_event.send(Ok(response_event)).await;
                        if is_completed { return; }
                    }
                    Some(Err(error)) => { response_error = Some(error); }
                    None => {}  // ping、message_start、message_stop 被跳过
                }
            }
            Ok(None) => {
                // 流结束（response body 关闭，没有更多事件）
                let error = response_error.unwrap_or_else(||
                    ApiError::Stream("stream closed before message_delta".into()));
                // ← 这就是你看到的错误！
                tx_event.send(Err(error)).await;
                return;
            }
            Err(_) => {
                // idle_timeout 超时
                tx_event.send(Err(ApiError::Stream("idle timeout waiting for SSE".into()))).await;
                return;
            }
        }
    }
}
```

### 第 12 步：SSE 事件翻译 → 返回给调用方

**文件：** `anthropic-client/src/translate/response.rs:77-278`

```rust
pub fn handle_sse_event(event: &SseEvent, state: &mut SseState) -> Option<Result<ResponseEvent, ApiError>> {
    match event {
        SseEvent::MessageStart { message } => {
            // 记录 response_id, server_model
            None  // 不产生 ResponseEvent
        }
        SseEvent::ContentBlockStart { index, content_block } => {
            match content_block {
                ContentBlockStartInfo::Text { text } => {
                    Some(Ok(ResponseEvent::OutputItemAdded(Message { ... })))
                }
                ContentBlockStartInfo::ToolUse { id, name } => {
                    Some(Ok(ResponseEvent::OutputItemAdded(FunctionCall { ... })))
                }
                ContentBlockStartInfo::Thinking { thinking } => { ... }
                ContentBlockStartInfo::RedactedThinking { data } => { ... }
            }
        }
        SseEvent::ContentBlockDelta { index, delta } => {
            match delta {
                Delta::TextDelta { text } => Some(Ok(ResponseEvent::OutputTextDelta(text))),
                Delta::InputJsonDelta { partial_json } => Some(Ok(ResponseEvent::ToolCallInputDelta { ... })),
                Delta::ThinkingDelta { thinking } => Some(Ok(ResponseEvent::ReasoningContentDelta { ... })),
                Delta::SignatureDelta { signature } => { ... },
            }
        }
        SseEvent::MessageDelta { delta, usage } => {
            Some(Ok(ResponseEvent::Completed {
                response_id,
                token_usage: Some(map_usage(usage)),
                end_turn: map_stop_reason_to_end_turn(delta.stop_reason),
            }))
        }
        SseEvent::Error { error } => {
            Some(Err(map_anthropic_error(&error.error_type, &error.message)))
        }
        // Ping, MessageStop → None
    }
}
```

---

## 完整请求链路图

```
config.toml
  │
  ├─ [model_provider = "openrouter"]
  ├─ [model_providers.openrouter.base_url = "https://openrouter.ai/api"]
  ├─ [model_providers.openrouter.wire_api = "anthropic"]
  └─ [model_providers.openrouter.experimental_bearer_token = "sk-or-v1-xxx"]
       │
       ▼
  ┌─────────────────────────────────────────┐
  │ 步骤 1: TOML 反序列化                     │
  │ config/src/config_toml.rs:62            │
  │ toml::from_str() → ConfigToml           │
  └─────────────────┬───────────────────────┘
                    │
       ┌────────────┴────────────┐
       │ model: "tencent/hy3-preview:free" │
       │ model_provider: "openrouter"      │
       │ model_providers: HashMap          │
       │   "openrouter": ModelProviderInfo │
       │     wire_api: Anthropic           │
       │     base_url: https://.../api     │
       │     experimental_bearer_token: ...│
       │     stream_idle_timeout_ms: 30000 │
       │     request_max_retries: 3        │
       └────────────┬────────────┘
                    │
       ┌────────────▼────────────┐
       │ 步骤 2: Provider 合并     │
       │ core/src/config/mod.rs: │
       │ 2227-2245                │
       │ built_in + custom        │
       │ 选择 model_provider_id   │
       │ "openrouter"             │
       └────────────┬────────────┘
                    │
       ┌────────────▼────────────┐
       │ 步骤 3: 创建运行时        │
       │ ModelProvider            │
       │ model-provider/src/      │
       │ provider.rs:130-141      │
       │ is_anthropic() → true    │
       │ → AnthropicModelProvider │
       └────────────┬────────────┘
                    │
       ┌────────────▼────────────┐
       │ 步骤 4: 解析认证          │
       │ model-provider/src/      │
       │ anthropic/mod.rs:102     │
       │ resolve_api_key:         │
       │  1. env_key → None       │
       │  2. experimental_bearer  │
       │     → "sk-or-v1-xxx"     │
       │ → BearerAuthProvider     │
       └────────────┬────────────┘
                    │
       ┌────────────▼────────────┐
       │ 步骤 5-6: Session 初始化  │
       │ → ModelClientSession     │
       │ → stream() dispatch      │
       │ wire_api == Anthropic    │
       │ → stream_anthropic()     │
       └────────────┬────────────┘
                    │
       ┌────────────▼────────────┐
       │ 步骤 7: stream_anthropic │
       │ core/src/client.rs:1514  │
       │ - build_reqwest_client() │
       │ - build messages from    │
       │   ResponseItems          │
       │ - AnthropicClient::new() │
       └────────────┬────────────┘
                    │
       ┌────────────▼────────────┐
       │ 步骤 8: send_request     │
       │ anthropic-client/src/    │
       │ client.rs:98             │
       │                          │
       │ Headers 构建:             │
       │  1. anthropic-version:   │
       │     2023-06-01           │
       │  2. anthropic-beta:      │
       │     prompt-caching-...,  │
       │     token-efficient-...  │
       │  3. Accept: text/event-  │
       │     stream               │
       │                          │
       │ URL 构建:                 │
       │  base_url + "v1/messages"│
       │  = https://openrouter.ai │
       │    /api/v1/messages      │
       │                          │
       │ Body:                    │
       │  {                       │
       │    model: "tencent/...", │
       │    messages: [...],      │
       │    system: "...",        │
       │    max_tokens: 4096,     │
       │    stream: true,         │
       │    tools: [...]          │
       │  }                       │
       └────────────┬────────────┘
                    │
       ┌────────────▼────────────┐
       │ 步骤 9: 重试 + 认证注入   │
       │ run_with_retry(          │
       │   max_attempts: 3,       │
       │   retry_429: true,       │
       │   retry_5xx: true        │
       │ )                        │
       │ auth.apply_auth(req)     │
       │ → Authorization: Bearer  │
       │   sk-or-v1-xxx           │
       └────────────┬────────────┘
                    │
       ┌────────────▼────────────┐
       │ 步骤 10: ReqwestTransport│
       │ agere-client/src/        │
       │ transport.rs:90          │
       │                          │
       │ Reqwest::RequestBuilder  │
       │ .method(POST)            │
       │ .url("https://.../v1/    │
       │   messages")             │
       │ .headers(ALL HEADERS)    │
       │ .json(BODY)              │
       │ .send()                  │
       │ → reqwest::Client.send() │
       └────────────┬────────────┘
                    │
       ┌────────────▼────────────┐
       │ 步骤 11: SSE 解析        │
       │ process_anthropic_sse()  │
       │ - eventsource() 解析 SSE │
       │ - 超时: 30000ms          │
       │ - 期望事件顺序:           │
       │   message_start →        │
       │   content_block_start →  │
       │   content_block_delta →  │
       │   content_block_stop →   │
       │   message_delta →        │
       │   message_stop           │
       └────────────┬────────────┘
                    │
       ┌────────────▼────────────┐
       │ 步骤 12: 翻译为          │
       │ ResponseEvent            │
       │ 通过 mpsc channel 发回   │
       │ ResponseStream.rx_event  │
       └─────────────────────────┘
```

### 最终发出的 HTTP 请求

```
POST https://openrouter.ai/api/v1/messages
Content-Type: application/json
anthropic-version: 2023-06-01
anthropic-beta: prompt-caching-2024-07-31,token-efficient-tools-2025-02-19
Accept: text/event-stream
Authorization: Bearer sk-or-v1-xxx
User-Agent: <默认值>
traceparent: <OpenTelemetry trace header>

{
  "model": "tencent/hy3-preview:free",
  "messages": [{"role": "user", "content": [...]}],
  "system": [{"type": "text", "text": "..."}],
  "max_tokens": 4096,
  "stream": true,
  "tools": [...]
}
```

### 为什么 `base_url` 不能带 `/v1`

AnthropicClient 硬编码了相对路径 `v1/messages`：
```rust
self.provider.build_request(Method::POST, "v1/messages")
```

`Provider.url_for_path` 的拼接逻辑：
```rust
format!("{base}/{path}")  // 直接拼接，无去重处理
```

所以：
- `base_url = "https://openrouter.ai/api"` → URL = `"https://openrouter.ai/api/v1/messages"` ✅
- `base_url = "https://openrouter.ai/api/v1"` → URL = `"https://openrouter.ai/api/v1/v1/messages"` ❌

Responses 模式用的是 `"responses"` 路径（无 `/v1`），所以即使 `base_url` 带 `/v1` 也不会重复：
- `base_url = "https://openrouter.ai/api/v1"` → URL = `"https://openrouter.ai/api/v1/responses"` ✅