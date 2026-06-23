# OpenAgere App Server 客户端对接指南（中文）

本文是 `agere app-server` 客户端对接的工程化中文指南。`app-server/README.md` 是英文权威说明，本文与之互补，重点在于：

- 系统化梳理协议、连接、握手、对话编排、错误恢复等接入流程。
- 把零散的 schema 信息（位于 `app-server-protocol/schema/typescript/v2/`）整合成参数/响应/字段说明表，便于一次性查阅。
- 给出可直接复制的最小可运行客户端骨架（Node.js / Python / Go / Rust），覆盖 stdio、WebSocket、Unix Socket 三种传输。
- 对每一类 RPC（thread、turn、item、command/exec、fs、account、config、mcp、plugin、apps、device-key、experimental）给出参数表与典型时序。

## 目录

- [1. 总览与术语](#1-总览与术语)
- [2. 启动与传输](#2-启动与传输)
  - [2.1 启动命令与常见参数](#21-启动命令与常见参数)
  - [2.2 stdio 传输](#22-stdio-传输)
  - [2.3 WebSocket 传输](#23-websocket-传输)
  - [2.4 Unix Socket 传输](#24-unix-socket-传输)
  - [2.5 in-process / off 传输](#25-in-process--off-传输)
- [3. JSON-RPC 线路格式](#3-json-rpc-线路格式)
- [4. 连接生命周期与握手](#4-连接生命周期与握手)
- [5. 核心概念：Thread / Turn / Item](#5-核心概念thread--turn--item)
- [6. 最小客户端骨架](#6-最小客户端骨架)
  - [6.1 Node.js（stdio）](#61-nodejsstdio)
  - [6.2 Python（stdio）](#62-pythonstdio)
  - [6.3 Go（WebSocket）](#63-gowebsocket)
  - [6.4 Rust（in-process）](#64-rustin-process)
- [7. Thread API 全集](#7-thread-api-全集)
- [8. Turn API 全集](#8-turn-api-全集)
- [9. Item 类型与流式渲染](#9-item-类型与流式渲染)
- [10. 审批与服务端发起的请求](#10-审批与服务端发起的请求)
- [11. Command Exec（独立命令执行）](#11-command-exec独立命令执行)
- [12. Filesystem API](#12-filesystem-api)
- [13. Account / 鉴权 API](#13-account--鉴权-api)
- [14. Config / 配置](#14-config--配置)
- [15. Skills / Apps / Plugins / Marketplace](#15-skills--apps--plugins--marketplace)
- [16. MCP 集成](#16-mcp-集成)
- [17. Device Key（设备签名）](#17-device-key设备签名)
- [18. 实时（Realtime）API（实验）](#18-实时realtime-api实验)
- [19. 错误模型](#19-错误模型)
- [20. 限流与背压](#20-限流与背压)
- [21. 实验性 API 与能力协商](#21-实验性-api-与能力协商)
- [22. 通知 opt-out 与降噪](#22-通知-opt-out-与降噪)
- [23. 重连、订阅与清理](#23-重连订阅与清理)
- [24. 安全模型](#24-安全模型)
- [25. 生成强类型 SDK](#25-生成强类型-sdk)
- [26. 可观测性与调试](#26-可观测性与调试)
- [27. 常见 FAQ / 陷阱](#27-常见-faq--陷阱)
- [附录 A：客户端可调用方法速查表](#附录-a客户端可调用方法速查表)
- [附录 B：服务端通知速查表](#附录-b服务端通知速查表)
- [附录 C：服务端发起的请求方法速查表](#附录-c服务端发起的请求方法速查表)
- [附录 D：典型时序图](#附录-d典型时序图)

## 1. 总览与术语

`agere app-server` 是 OpenAgere 的 RPC 控制面。客户端（IDE 插件、桌面 GUI、Web 前端、CLI 工具、本地脚本等）通过它来：

- 管理会话（创建、恢复、分叉、归档、滚动回退等）；
- 驱动一轮“用户输入 → Agent 思考/执行/输出”的过程；
- 接收增量事件以做流式渲染；
- 处理审批、动态工具、MCP elicitation 等服务端反向请求；
- 管理账号、配置、模型、Skills、Apps、Plugins、MCP；
- 暴露受限的文件系统/命令执行能力供客户端直接复用。

| 术语 | 含义 |
| --- | --- |
| `agere` | 项目主二进制。`agere app-server` 是其子命令。 |
| AGERE_HOME | 用户级数据目录，默认 `$HOME/.openagere`；通过 `initialize` 响应可得到绝对路径。 |
| Thread | 一次完整会话；持久化为 sqlite + rollout JSONL。 |
| Turn | 一轮“用户输入 → Agent 输出”的过程。 |
| Item | Turn 内部最小可渲染单元（消息、命令、文件改动等）。 |
| RPC v1 | 旧兼容层（`getConversationSummary`、`fuzzyFileSearch*` 等），新客户端尽量避免。 |
| RPC v2 | 当前所有新功能均添加到 v2。本文几乎只覆盖 v2。 |
| ExperimentalApi | 客户端在 `initialize.capabilities.experimentalApi=true` 时才暴露的方法/字段/通知。 |

协议线缆使用 **JSON-RPC 2.0 的最小子集**：

- 不发送、也不期望 `"jsonrpc": "2.0"` 字段；
- 一次只交换一个 JSON 对象；
- 双向：客户端和服务端都可以发起请求/通知。

## 2. 启动与传输

### 2.1 启动命令与常见参数

`agere app-server` 由主二进制分发，常见用法：

```bash
agere app-server                       # 默认 stdio
agere app-server --listen ws://127.0.0.1:7842
agere app-server --listen unix://
agere app-server --listen unix:///abs/path.sock
agere app-server --listen off          # 完全不暴露传输（仅用于嵌入）

# 桥接 unix socket 到 stdio（适合 CLI/桥接进程）
agere app-server proxy
agere app-server proxy --sock /abs/path.sock
```

| 参数 | 说明 |
| --- | --- |
| `--listen <URL>` | `stdio://`（默认）/`unix://[PATH]` / `ws://IP:PORT` / `off`。 |
| `--session-source <SOURCE>` | 会话来源：`vscode`（默认）/`cli`/`exec`/`subAgent`/`appServer` 等，影响产品策略与日志归类。 |
| `--strict-config` | `config.toml` 含未知字段时立即报错（默认仅警告）。 |
| `--ws-auth …` | 仅 `ws://` 生效，详见 [2.3](#23-websocket-传输)。 |
| `RUST_LOG` 环境变量 | 控制 server 日志级别（如 `info`、`debug,agere_app_server=trace`）。 |
| `LOG_FORMAT=json` | 切换 stderr 日志为 NDJSON（便于接入日志系统）。 |
| `AGERE_HOME` | 重写默认的 `$HOME/.openagere`。 |
| `AGERE_APP_SERVER_DISABLE_MANAGED_CONFIG=1` | 测试场景禁用受管 config（debug 构建生效）。 |

`agere app-server generate-ts --out DIR [--experimental]` 与 `agere app-server generate-json-schema --out DIR [--experimental]` 会输出与**当前 server 版本**完全一致的 TypeScript / JSON Schema，请将此输出纳入客户端的发布流程，详见 [§25](#25-生成强类型-sdk)。

### 2.2 stdio 传输

**线缆**：JSONL（每条 JSON 一行，`\n` 分隔），客户端在子进程 stdout 上读、stdin 上写。stderr 通常是日志，**不能用来解析协议**。

**适用场景**：本地 IDE 插件（VS Code、JetBrains）、桌面应用 sidecar、单元测试、CI 等。最简单、零端口冲突、与多用户/多实例共存最好。

**注意点**：

- 子进程退出前必须 `await flush(stdin)`；否则最后一条请求可能未抵达 server；
- 子进程崩溃时父进程要捕获 stderr 用于诊断；
- 不要在同一个 stdio 连接上并行启动两个客户端（共享 stdin/stdout 会撕裂帧）。

### 2.3 WebSocket 传输

**线缆**：每个 JSON 用 **一个独立的文本帧**（WebSocket text frame）。同一个连接的所有帧严格按到达顺序处理。

**适用场景**：远程开发（SSH tunnel）、Web 前端 / Electron renderer。

**HTTP 探针**（与 ws 监听共享同一端口）：

- `GET /readyz` → 监听就绪即 `200 OK`；
- `GET /healthz` → 无 `Origin` 头：`200 OK`；任何携带 `Origin` 的请求 → `403 Forbidden`（同时也作用于 ws 握手，避免被浏览器跨域劫持）。

**鉴权（仅 ws）**：握手时通过 `Authorization: Bearer <token>` 头携带，认证在 JSON-RPC `initialize` 之前完成。配置任意一种：

| 模式 | 启动参数 | 说明 |
| --- | --- | --- |
| Capability Token（token 明文） | `--ws-auth capability-token --ws-token-file /abs/path` | 推荐：文件权限 `0400` 由 server 进程用户拥有。 |
| Capability Token（仅哈希） | `--ws-auth capability-token --ws-token-sha256 <HEX>` | server 不需要明文，但 hash 会进入进程参数。 |
| Signed Bearer Token | `--ws-auth signed-bearer-token --ws-shared-secret-file /abs/path [--ws-issuer …] [--ws-audience …] [--ws-max-clock-skew-seconds N]` | HMAC-SHA 签名的短期 JWT/JWS，适合多客户端、可吊销场景。 |

**关键约束**：非 loopback 监听**强制**配置认证；loopback（`127.0.0.1`、`::1`）可以无认证但仍**强烈建议**开启。WebSocket 当前在 OpenAgere 中标记为实验性，请勿用于生产关键链路。

### 2.4 Unix Socket 传输

**线缆**：在 Unix 域套接字之上跑 HTTP Upgrade → WebSocket 协议栈（与 ws 同构）。`unix://` 不带路径时默认为 `$AGERE_HOME/app-server-control/app-server-control.sock`。

**适用场景**：

- 同机控制面：CLI、其他守护进程通过 socket 触发 `command/exec`、`thread/start` 等；
- 桥接：`agere app-server proxy` 把 socket 桥成 stdin/stdout 流，便于嵌入到 OpenSSH `RemoteCommand`、Tunnel、Sidecar 中。

**权限模型**：socket 文件默认 0600，仅当前用户可读写；server 启动时持有 `app-server-startup.lock` 文件锁，避免同一 AGERE_HOME 下并行启动多个 server。

### 2.5 in-process / off 传输

- `--listen off`：不开放任何外部传输；适合通过 `agere-app-server` crate 在自家进程内嵌时；外部仅依赖 `in_process::*` API。
- in-process：把 server 作为库使用，所有消息通过内存 channel 投递；这种用法允许 `device/key/*` 等受限 API（详见 [§17](#17-device-key设备签名)）。

## 3. JSON-RPC 线路格式

所有通讯都是单个 JSON 对象。**不要**附带 `"jsonrpc":"2.0"` 字段。

### 3.1 客户端 → 服务端：请求

```jsonc
{
  "id": 17,                       // string 或 integer；连接内唯一
  "method": "turn/start",         // 见附录 A
  "params": { /* ... */ },         // 可选；无参方法可省略
  "trace": { /* W3C Trace Context */ }   // 可选；用于分布式追踪
}
```

### 3.2 服务端 → 客户端：响应

```jsonc
{ "id": 17, "result": { /* ... */ } }
```

错误响应：

```jsonc
{
  "id": 17,
  "error": { "code": -32602, "message": "Invalid params: …", "data": null }
}
```

| code | 名称 | 说明 |
| --- | --- | --- |
| -32600 | Invalid Request | 协议错误（如未握手就发其它请求、重复 `initialize`、方法不允许等） |
| -32602 | Invalid Params | 字段缺失/类型错误/互斥参数同时出现/路径非绝对 |
| -32603 | Internal Error | 服务端内部错误，详见 `data` / 日志 |
| -32001 | Overloaded | 队列饱和，客户端应退避重试 |

### 3.3 服务端 → 客户端：通知

```jsonc
{ "method": "item/agentMessage/delta", "params": { /* ... */ } }
```

无 `id`、不需要回包。通知按 `params` 内的 `threadId` / `turnId` / `itemId` 关联到具体上下文。

### 3.4 服务端 → 客户端：服务端发起的请求

```jsonc
{ "id": 200, "method": "item/commandExecution/requestApproval", "params": { /* ... */ } }
```

客户端必须按相同 `id` 回包 `result` 或 `error`，否则该 turn 会一直挂起。详见 [§10](#10-审批与服务端发起的请求)。

### 3.5 客户端 → 服务端：通知

目前只有一种：

```jsonc
{ "method": "initialized" }
```

## 4. 连接生命周期与握手

每个**物理连接**都必须独立完成一次握手；reconnect 必须重新走一遍。

1. 建立传输连接（stdio/ws/unix）。
2. 发送 **唯一一次** `initialize`：

   ```jsonc
   {
     "id": 0,
     "method": "initialize",
     "params": {
       "clientInfo": {
         "name": "my_client",       // 用于 OpenAI 合规日志识别；企业级集成请联系 OpenAI 加入白名单
         "title": "My Client",
         "version": "0.1.0"
       },
       "capabilities": {
         "experimentalApi": false,
         "optOutNotificationMethods": []  // 可精确抑制通知方法名
       }
     }
   }
   ```

3. 接收响应：

   ```jsonc
   {
     "id": 0,
     "result": {
       "userAgent": "agere/<version> (…)",
       "agereHome": "/Users/me/.openagere",
       "platformFamily": "unix",        // unix | windows
       "platformOs": "macos"            // macos | linux | windows
     }
   }
   ```

4. 发送 `initialized` 通知，告诉 server“可以开始下发通知/反向请求了”：

   ```jsonc
   { "method": "initialized" }
   ```

握手前后行为差异：

| 状态 | 行为 |
| --- | --- |
| 握手前 | 任何非 `initialize` 请求 → `-32600 "Not initialized"`；不会下发通知。 |
| 握手中 | 已收到 `initialize` 响应但未发 `initialized`：server 暂不广播；客户端必须尽快发 `initialized`。 |
| 握手完成 | 全部 API 可用（按 `experimentalApi` 取舍）；server 会立即下发当前 `account/updated`、`remoteControl/status/changed` 等状态快照。 |
| 重复 `initialize` | `-32600 "Already initialized"`。 |

## 5. 核心概念：Thread / Turn / Item

三层数据结构（参见 `app-server-protocol/schema/typescript/v2/Thread.ts`、`Turn.ts`、`ThreadItem.ts`）：

```text
Thread
  ├─ id, modelProvider, cwd, createdAt, updatedAt, status, name, ...
  └─ turns[]                       ← 仅在某些接口/参数下完整返回
        └─ Turn
             ├─ id, status, startedAt, completedAt, durationMs, error?, ...
             └─ items[]            ← 仅在 resume/fork 时回放完整
                  └─ ThreadItem    ← tagged union by `type`
```

- **Thread**：长生命周期会话，持久化在 sqlite 元数据 + rollout JSONL 中。`thread/list` 时仅返回轻量元数据；`thread/read` / `thread/resume` / `thread/fork` 在 `includeTurns` 或默认情形下返回 `turns[]`，每个 turn 默认含 `items[]`。
- **Turn**：`TurnStatus = "completed" | "interrupted" | "failed" | "inProgress"`；失败时 `error: {message, agereErrorInfo?, additionalDetails?}`。
- **Item**：`ThreadItem` 是带 `type` 判别的 union，详见 [§9](#9-item-类型与流式渲染)。
- **ThreadStatus**：`{ "type": "notLoaded" | "idle" | "systemError" | "active", activeFlags?: ThreadActiveFlag[] }`。客户端可借此渲染“当前会话是否在跑”。

**会话状态变更**通过下列通知传递：

- `thread/started`：thread 被新建/加载、客户端已订阅。
- `thread/status/changed`：thread 状态机变化（不含初始 `thread/started`，那条已经携带 status）。
- `thread/tokenUsage/updated`：token usage 累计变化（per-turn 维度）。
- `thread/closed`：server 卸载 thread 时发出（30 分钟无订阅者 + 无活动后）。

## 6. 最小客户端骨架

### 6.1 Node.js（stdio）

```ts
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";

type RpcResponse = { id: number | string; result?: any; error?: { code: number; message: string } };

function startAgere() {
  const child = spawn("agere", ["app-server"], { stdio: ["pipe", "pipe", "inherit"] });
  const rl = createInterface({ input: child.stdout! });
  return { child, rl };
}

class AgereClient {
  private nextId = 1;
  private pending = new Map<number, (r: RpcResponse) => void>();
  private notifyHandlers: ((msg: any) => void)[] = [];
  private serverReqHandlers: ((msg: any) => Promise<any>)[] = [];

  constructor(private child: ReturnType<typeof startAgere>["child"], rl: ReturnType<typeof startAgere>["rl"]) {
    rl.on("line", (line) => this.onLine(line));
    child.on("exit", (code) => console.error("agere exited", code));
  }

  private onLine(line: string) {
    if (!line.trim()) return;
    const msg = JSON.parse(line);
    if (msg.id !== undefined && (msg.result !== undefined || msg.error !== undefined)) {
      this.pending.get(msg.id)?.(msg);
      this.pending.delete(msg.id);
    } else if (msg.id !== undefined && msg.method) {
      this.handleServerRequest(msg).catch(console.error);
    } else if (msg.method) {
      this.notifyHandlers.forEach((h) => { try { h(msg); } catch (e) { console.error(e); } });
    }
  }

  private async handleServerRequest(req: { id: number | string; method: string; params: any }) {
    for (const h of this.serverReqHandlers) {
      const r = await h(req);
      if (r !== undefined) { this.send({ id: req.id, result: r }); return; }
    }
    this.send({ id: req.id, error: { code: -32601, message: `unhandled server request: ${req.method}` } });
  }

  onNotification(h: (msg: any) => void) { this.notifyHandlers.push(h); }
  onServerRequest(h: (msg: any) => Promise<any | undefined>) { this.serverReqHandlers.push(h); }

  send(obj: object) { this.child.stdin!.write(JSON.stringify(obj) + "\n"); }

  request<T = any>(method: string, params?: any): Promise<T> {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, (r) => (r.error ? reject(r.error) : resolve(r.result)));
      this.send({ id, method, params });
    });
  }
}

(async () => {
  const { child, rl } = startAgere();
  const c = new AgereClient(child, rl);

  c.onNotification((m) => console.log("notify", m.method, m.params));
  c.onServerRequest(async (req) => {
    if (req.method === "item/commandExecution/requestApproval") return { decision: "decline" };
    if (req.method === "item/fileChange/requestApproval") return { decision: "decline" };
    return undefined;
  });

  await c.request("initialize", {
    clientInfo: { name: "demo_node", title: "Demo", version: "0.1.0" },
    capabilities: { experimentalApi: false },
  });
  c.send({ method: "initialized" });

  const start = await c.request("thread/start", {
    cwd: process.cwd(),
    approvalPolicy: "unlessTrusted",
    accessMode: "workspace-write",
  });

  const turn = await c.request("turn/start", {
    threadId: start.thread.id,
    input: [{ type: "text", text: "请总结这个仓库" }],
  });

  console.log("turn started", turn.turn.id);
})();
```

### 6.2 Python（stdio）

```python
import json, subprocess, threading, queue, sys

class Agere:
    def __init__(self):
        self.p = subprocess.Popen(["agere", "app-server"], stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=sys.stderr, bufsize=0)
        self.next_id = 1
        self.pending: dict[int, queue.Queue] = {}
        self.notif_q: queue.Queue = queue.Queue()
        threading.Thread(target=self._reader, daemon=True).start()

    def _reader(self):
        for line in self.p.stdout:
            line = line.decode("utf-8").rstrip("\n")
            if not line: continue
            msg = json.loads(line)
            if "id" in msg and ("result" in msg or "error" in msg):
                q = self.pending.pop(msg["id"], None)
                if q: q.put(msg)
            else:
                self.notif_q.put(msg)

    def _send(self, obj): self.p.stdin.write((json.dumps(obj) + "\n").encode())

    def request(self, method, params=None):
        rid = self.next_id; self.next_id += 1
        q: queue.Queue = queue.Queue(); self.pending[rid] = q
        self._send({"id": rid, "method": method, "params": params})
        resp = q.get(timeout=120)
        if "error" in resp: raise RuntimeError(resp["error"])
        return resp["result"]

    def notify(self, method, params=None): self._send({"method": method, "params": params})

a = Agere()
a.request("initialize", {"clientInfo": {"name": "demo_py", "title": "Demo", "version": "0.1.0"},
                         "capabilities": {"experimentalApi": False}})
a.notify("initialized")
thr = a.request("thread/start", {"cwd": "/Users/me/project", "approvalPolicy": "unlessTrusted"})
a.request("turn/start", {"threadId": thr["thread"]["id"],
                         "input": [{"type": "text", "text": "Hello"}]})
```

### 6.3 Go（WebSocket）

```go
package main

import (
    "context"
    "encoding/json"
    "fmt"
    "log"
    "net/http"

    "github.com/coder/websocket" // 任意 ws 库均可
)

type rpcMsg struct {
    ID     any             `json:"id,omitempty"`
    Method string          `json:"method,omitempty"`
    Params json.RawMessage `json:"params,omitempty"`
    Result json.RawMessage `json:"result,omitempty"`
    Error  *struct {
        Code    int    `json:"code"`
        Message string `json:"message"`
    } `json:"error,omitempty"`
}

func main() {
    ctx := context.Background()
    c, _, err := websocket.Dial(ctx, "ws://127.0.0.1:7842",
        &websocket.DialOptions{HTTPHeader: http.Header{"Authorization": {"Bearer xxx"}}})
    if err != nil { log.Fatal(err) }
    defer c.Close(websocket.StatusNormalClosure, "")

    send := func(v any) {
        b, _ := json.Marshal(v)
        if err := c.Write(ctx, websocket.MessageText, b); err != nil { log.Fatal(err) }
    }

    send(map[string]any{"id": 1, "method": "initialize", "params": map[string]any{
        "clientInfo":   map[string]any{"name": "demo_go", "title": "Demo", "version": "0.1.0"},
        "capabilities": map[string]any{"experimentalApi": false},
    }})
    send(map[string]any{"method": "initialized"})

    for {
        _, data, err := c.Read(ctx)
        if err != nil { log.Fatal(err) }
        var m rpcMsg
        _ = json.Unmarshal(data, &m)
        fmt.Printf("recv: id=%v method=%s err=%+v\n", m.ID, m.Method, m.Error)
    }
}
```

### 6.4 Rust（in-process）

对于希望深度内嵌的客户端（例如做一个新的 TUI / GUI），可以直接依赖 `agere-app-server` crate 并使用 in-process 入口：

```toml
[dependencies]
agere-app-server = { path = "../app-server" }
agere-app-server-protocol = { path = "../app-server-protocol" }
tokio = { version = "1", features = ["full"] }
```

```rust
use agere_app_server::in_process::{start_in_process, InProcessHandle};
use agere_app_server_protocol::JSONRPCMessage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let InProcessHandle { mut from_server, to_server, .. } = start_in_process(Default::default()).await?;

    // 1) 发起 initialize
    let init = serde_json::json!({
        "id": 1, "method": "initialize",
        "params": { "clientInfo": {"name":"demo_rs","title":"","version":"0.1.0"} }
    });
    to_server.send(serde_json::from_value::<JSONRPCMessage>(init)?).await?;

    while let Some(msg) = from_server.recv().await {
        println!("{:?}", msg);
    }
    Ok(())
}
```

具体 API 名称以仓库当前版本为准（见 `app-server/src/in_process.rs`），但通讯模型与 stdio/ws 完全一致。

## 7. Thread API 全集

所有 v2 thread 方法。除非显式标注，否则均为稳定 API。

| 方法 | params 关键字段 | 返回 | 说明 |
| --- | --- | --- | --- |
| `thread/start` | `model?`、`modelProvider?`、`cwd?`、`approvalPolicy?`、`approvalsReviewer?`、`accessMode?`、`personality?`、`config?`、`baseInstructions?`、`developerInstructions?`、`serviceTier?`、`serviceName?`、`ephemeral?`、`sessionStartSource?`；实验性：`dynamicTools?`、`environments?`、`persistExtendedHistory?`、`outputSchema?` 等 | `{ thread, model, modelProvider, serviceTier, cwd, instructionSources, approvalPolicy, approvalsReviewer, accessPolicy, reasoningEffort }` | 新建 thread 并自动订阅；同时下发 `thread/started`。`accessMode` 与 `permissionProfile` 互斥。 |
| `thread/resume` | `threadId`（必填）、可选与 `thread/start` 一致的覆盖项、`excludeTurns?` | 同 `thread/start` 响应 | 恢复存档；含 turn 历史；持久化 token usage 会立即通过 `thread/tokenUsage/updated` 回放。 |
| `thread/fork` | `threadId`、上面同款覆盖项、`ephemeral?`、`excludeTurns?` | 同 `thread/start` 响应 | 复制历史到新 thread；下发 `thread/started`；若源 thread 正在跑则按 `interrupt` 行为快照。 |
| `thread/read` | `threadId`、`includeTurns: boolean` | `{ thread }` | 只读读取；不订阅、不加载到内存。 |
| `thread/list` | `cursor?`、`limit?`、`sortKey?`(`created_at`\|`updated_at`)、`sortDirection?`、`modelProviders?`、`sourceKinds?`、`archived?`、`cwd?`（`string` 或 `string[]`）、`useStateDbOnly?`、`searchTerm?` | `{ data, nextCursor, backwardsCursor }` | `data[].turns` 始终空数组；`status` 默认为 `{type:"notLoaded"}`。 |
| `thread/loaded/list` | — | `{ data: string[] }` | 内存中已加载的 thread id。 |
| `thread/turns/list` | `threadId`、`cursor?`、`limit?`、`sortDirection?` | `{ data, nextCursor, backwardsCursor }` | 不恢复 thread 的前提下分页拉取 turns。 |
| `thread/metadata/update` | `threadId`、`gitInfo?` 等 | `{ thread }` | 仅更新 sqlite 元数据（如 `gitInfo`）。 |
| `thread/provider/update` | `threadId` | `{ thread }` | 同步运行时 provider 到最新 config。 |
| `thread/archive` / `unarchive` | `{ threadId }` | `{}` / `{ thread }` | 归档/取消归档；归档后默认不在 `thread/list` 出现，除非 `archived: true`。 |
| `thread/name/set` | `threadId` 或 `path`、`name` | `{}` | 自定义 thread 名称；下发 `thread/name/updated`。 |
| `thread/unsubscribe` | `threadId` | `{ status: "unsubscribed" \| "notSubscribed" \| "notLoaded" }` | 取消该连接的订阅；最后一个订阅者退出后 30 分钟内卸载 thread，下发 `thread/closed`。 |
| `thread/compact/start` | `threadId` | `{}` | 手动触发历史压缩；通过 `item/started/completed`(contextCompaction) 渲染进度。 |
| `thread/shellCommand` | `threadId`、`command` | `{}` | TUI `!` 指令等价；**以宿主完全权限运行**，不继承 thread 限制。 |
| `thread/rollback` | `threadId`、`numTurns` 等 | `{ thread }` | 删除最后 N 个 turn 并记录回滚标记。 |
| `thread/inject_items` | `threadId`、`items: ResponsesItem[]` | `{}` | 注入原始 Responses API items，不触发 turn。 |
| `thread/goal/set` | `threadId`、`objective?`、`status?`、`tokenBudget?`、`replaceExisting?` | `{ goal }` | 单 thread 的目标管理；同时下发 `thread/goal/updated`。 |
| `thread/goal/get` | `threadId` | `{ goal: ThreadGoal \| null }` | 查询当前目标。 |
| `thread/goal/clear` | `threadId` | `{ cleared: boolean }` | 清空目标；变化时下发 `thread/goal/cleared`。 |
| `thread/approveGuardianDeniedAction` | `threadId`、`event` | `{}` | 兜底审批被 Guardian 拒绝的操作。 |
| `thread/memoryMode/set`（⚠️） | `threadId`、`mode: "enabled"\|"disabled"` | `{}` | 控制 thread 是否参与记忆生成。 |
| `thread/backgroundTerminals/clean`（⚠️） | `threadId` | `{}` | 清理 thread 下后台终端。 |
| `thread/realtime/*`（⚠️） | 详见 [§18](#18-实时realtime-api实验) | … | 实时音视频会话。 |

### `ThreadStartParams` 全字段说明

| 字段 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `model` | `string \| null` | 否 | 覆盖默认模型 id（如 `gpt-5.4`）。 |
| `modelProvider` | `string \| null` | 否 | 覆盖 provider（如 `openai`、`anthropic`、`openrouter` 等，取决于 config）。 |
| `serviceTier` | `ServiceTier \| null` | 否 | 服务等级（standard / priority 等）。 |
| `cwd` | `string \| null` | 否 | thread 默认 cwd；建议绝对路径，决定权限与上下文。 |
| `approvalPolicy` | `AskForApproval \| null` | 否 | `untrusted` / `on-failure` / `on-request` / `never` / `{ granular: {…} }`（granular 为实验）。 |
| `approvalsReviewer` | `"user" \| "auto_review" \| "guardian_subagent"` | 否 | 审批路由：`user` 用户自己批；`auto_review` 交给 Agent 子代理评估；`guardian_subagent` 为旧别名。 |
| `accessMode` | `"read-only" \| "workspace-write" \| "danger-full-access"` | 否 | 与 `permissionProfile` 互斥。 |
| `config` | `Record<string, JsonValue>` | 否 | 覆盖 config.toml 中的字段（snake_case）。 |
| `serviceName` | `string \| null` | 否 | 仅用于指标 / 计费标签。 |
| `baseInstructions` | `string \| null` | 否 | 替换默认系统提示词。 |
| `developerInstructions` | `string \| null` | 否 | 覆盖 developer 指令；`null` + 选择某 collaboration mode 时使用内置默认。 |
| `personality` | `"none" \| "friendly" \| "pragmatic"` | 否 | `none` 表示完全不注入 personality 占位。 |
| `ephemeral` | `bool \| null` | 否 | `true` 时仅内存中，不写盘；`thread.path` 返回 `null`。 |
| `sessionStartSource` | `"startup" \| "clear"` | 否 | 影响 `SessionStart` hooks 看到的 source。 |

### 典型示例

```jsonc
// 启动一个仅在内存中的会话，使用受限的 read-only 模式：
{
  "id": 100, "method": "thread/start",
  "params": {
    "cwd": "/Users/me/project",
    "approvalPolicy": "untrusted",
    "accessMode": "read-only",
    "ephemeral": true,
    "personality": "pragmatic"
  }
}
```

```jsonc
// 恢复一个 thread，但不要历史 turns（自己再分页拉）：
{ "id": 101, "method": "thread/resume", "params": { "threadId": "thr_123", "excludeTurns": true } }
```

```jsonc
// 分叉一个 ephemeral 子线程，做沙盒实验：
{ "id": 102, "method": "thread/fork", "params": { "threadId": "thr_123", "ephemeral": true } }
```

## 8. Turn API 全集

| 方法 | params | 返回 | 说明 |
| --- | --- | --- | --- |
| `turn/start` | `threadId`、`input: UserInput[]`、可选 `cwd/approvalPolicy/approvalsReviewer/accessPolicy/model/modelProvider/serviceTier/effort/summary/personality/outputSchema`、实验：`environments?` | `{ turn }` | 同步返回新 turn 对象（`status: "inProgress"`），随后通过通知流推送 Item 与 `turn/completed`。 |
| `turn/steer` | `threadId`、`input: UserInput[]`、`expectedTurnId` | `{ turnId }` | 在不开启新 turn 的前提下，追加用户输入到当前活跃的常规 turn；review / 手动 compact 拒绝。 |
| `turn/interrupt` | `threadId`、`turnId` | `{}` | 请求中断；turn 以 `interrupted` 收尾。 |
| `review/start` | `threadId`、`target: ReviewTarget`、`delivery?: "inline"\|"detached"` | `{ turn, reviewThreadId }` | 触发自动 code review。`target` 见下文。 |

`UserInput` 的全部变体（来自 `UserInput.ts`）：

```ts
type UserInput =
  | { type: "text",        text: string, text_elements: TextElement[] }
  | { type: "image",       url: string }
  | { type: "localImage",  path: string }     // 绝对路径
  | { type: "skill",       name: string, path: string }     // 触发 $<skill> 时建议加上
  | { type: "mention",     name: string, path: string };    // app://<connector-id> 或 plugin://<plugin>@<marketplace>
```

`TextElement` 用于在 UI 上标记 `text` 中某段 byte range 作为可点击/可编辑的“富元素”占位。

`ReviewTarget`：

```ts
type ReviewTarget =
  | { type: "uncommittedChanges" }
  | { type: "baseBranch",  branch: string }
  | { type: "commit",      sha: string, title: string | null }
  | { type: "custom",      instructions: string };
```

`AccessPolicy`（同 `thread/start.accessMode` 的全功能形态）：

```ts
type AccessPolicy =
  | { type: "dangerFullAccess" }
  | { type: "readOnly",       networkAccess: boolean }
  | { type: "external",       networkAccess: NetworkAccess }
  | { type: "workspaceWrite", writableRoots: string[], networkAccess: boolean,
                              excludeTmpdirEnvVar: boolean, excludeSlashTmp: boolean };
```

`AskForApproval`：

```ts
type AskForApproval =
  | "untrusted" | "on-failure" | "on-request" | "never"
  | { granular: { access_approval: boolean, rules: boolean, skill_approval: boolean,
                  request_permissions: boolean, mcp_elicitations: boolean } };   // experimental
```

### Turn 生命周期事件（按时间顺序）

1. `turn/start` 响应（同步） → 客户端拿到 `turn.id`、`turn.status === "inProgress"`。
2. `turn/started`（通知）→ `{ threadId, turn, modelContextWindow }`，真正开始模型推理时下发。
3. 多次 `item/started` → `item/<type>/delta` * N → `item/completed` 的循环。
4. 可能交错的：`turn/diff/updated`、`turn/plan/updated`、`thread/tokenUsage/updated`、`thread/rateLimit/waiting`、`hook/started` / `hook/completed`、`error` 等。
5. 服务端发起的请求（审批 / 工具调用 / MCP elicitation）→ 客户端必须回包，期间会有 `serverRequest/resolved` 提示。
6. `turn/completed`（通知）→ 最终 `{ threadId, turn }`，`turn.status` 为 `completed` / `interrupted` / `failed`；失败时 `turn.error` 包含 `agereErrorInfo`（见 [§19](#19-错误模型)）。

示例（`turn/start` 包含 `outputSchema` 实验字段）：

```jsonc
{
  "id": 200, "method": "turn/start",
  "params": {
    "threadId": "thr_123",
    "input": [{ "type": "text", "text": "请把 README 改成英文" }],
    "approvalPolicy": "on-request",
    "accessPolicy": { "type": "workspaceWrite", "writableRoots": ["/Users/me/project"],
                      "networkAccess": false, "excludeTmpdirEnvVar": false, "excludeSlashTmp": false },
    "model": "gpt-5.4",
    "effort": "medium",
    "summary": "concise",
    "personality": "friendly",
    "outputSchema": {
      "type": "object",
      "properties": { "answer": { "type": "string" } },
      "required": ["answer"],
      "additionalProperties": false
    }
  }
}
```

## 9. Item 类型与流式渲染

`ThreadItem` 是带 `type` 判别的 union（参见 `ThreadItem.ts`）。客户端必须以 `(threadId, turnId, item.id)` 为主键做幂等 upsert。

| `type` | 关键字段 | 增量通知 | 完成通知 |
| --- | --- | --- | --- |
| `userMessage` | `content: UserInput[]` | — | `item/started` + `item/completed` |
| `hookPrompt` | `fragments` | — | 同上 |
| `agentMessage` | `text`、`phase`、`memoryCitation` | `item/agentMessage/delta` `{itemId,delta}` | `item/completed` |
| `plan` | `text` | `item/plan/delta`（⚠️） | `item/completed` |
| `reasoning` | `summary[]`、`content[]` | `item/reasoning/summaryTextDelta`、`summaryPartAdded`、`textDelta` | `item/completed` |
| `commandExecution` | `command`、`cwd`、`processId`、`source`、`status`、`commandActions`、`aggregatedOutput`、`exitCode`、`durationMs` | `item/commandExecution/outputDelta`、`terminalInteraction` | `item/completed`（带最终 `status` & `exitCode`） |
| `fileChange` | `changes: FileUpdateChange[]`、`status` | `item/fileChange/patchUpdated`、`outputDelta` | `item/completed` |
| `mcpToolCall` | `server`、`tool`、`arguments`、`status`、`mcpAppResourceUri?`、`result?`、`error?`、`durationMs?` | `item/mcpToolCall/progress` | `item/completed` |
| `dynamicToolCall` | `namespace?`、`tool`、`arguments`、`status`、`contentItems?`、`success?`、`durationMs?` | 由服务端发起的 `item/tool/call` 请求驱动 | `item/completed` |
| `collabAgentToolCall` | `tool`、`status`、`senderThreadId`、`receiverThreadIds[]`、`prompt?`、`model?`、`reasoningEffort?`、`agentsStates` | — | `item/completed` |
| `webSearch` | `query`、`action?` | — | `item/completed` |
| `imageView` | `path` | — | `item/completed` |
| `imageGeneration` | `status`、`revisedPrompt?`、`result`、`savedPath?` | — | `item/completed` |
| `enteredReviewMode` / `exitedReviewMode` | `review` | — | `item/completed` |
| `contextCompaction` | `id` | — | `item/completed` |

**渲染建议**：

- 收到 `item/started` 立即在 UI 上插入“占位条目”，根据 `type` 选择渲染组件。
- 增量通知按 `itemId` 累加（如 agentMessage 的 `delta` 是 incremental text）。
- 收到 `item/completed` 后用 `item` 的最终状态覆盖（特别是 `commandExecution.aggregatedOutput`、`fileChange.status`、`mcpToolCall.result` 等）。
- 注意 `item/completed.isInterim` 字段：当为 `true` 时表示仍可能有后续内容，不要立刻终结动画。
- 长输出条目（command exec / file change）建议使用 `<details>` 折叠，仅展开当前活跃条目，避免 UI 抖动。

## 10. 审批与服务端发起的请求

下列方法都是 server → client 的请求；客户端按相同 `id` 回包 `result` 或 `error`。

| 方法 | params | 客户端响应 |
| --- | --- | --- |
| `item/commandExecution/requestApproval` | `threadId`、`turnId`、`itemId`、`approvalId?`、`reason?`、`networkApprovalContext?`、`command?`、`cwd?`、`commandActions?`、`proposedExecpolicyAmendment?`、`proposedNetworkPolicyAmendments?`、（⚠️）`additionalPermissions?`、`availableDecisions?` | `{ decision: CommandExecutionApprovalDecision }` |
| `item/fileChange/requestApproval` | `threadId`、`turnId`、`itemId`、`reason?`、`grantRoot?` | `{ decision: FileChangeApprovalDecision }` |
| `item/permissions/requestApproval` | `threadId`、`turnId`、`itemId`、`cwd`、`reason?`、`permissions: RequestPermissionProfile` | `{ permissions, scope: "turn"\|"session", strictAutoReview? }` |
| `item/tool/requestUserInput`（⚠️） | `threadId`、`turnId`、`itemId`、`questions: ToolRequestUserInputQuestion[]` | `{ answers: Record<questionId, ToolRequestUserInputAnswer> }` |
| `item/tool/call`（⚠️ 动态工具） | `threadId`、`turnId`、`callId`、`namespace?`、`tool`、`arguments` | `{ contentItems: DynamicToolCallOutputContentItem[], success: boolean }` |
| `mcpServer/elicitation/request` | `threadId`、`turnId?`、`serverName`、form / url 模式 | `{ action: McpServerElicitationAction, content: JsonValue \| null, _meta? }` |
| `account/chatgptAuthTokens/refresh` | 内部使用 | 见 schema |

`CommandExecutionApprovalDecision`：

```ts
type CommandExecutionApprovalDecision =
  | "accept" | "acceptForSession" | "decline" | "cancel"
  | { acceptWithExecpolicyAmendment:   { execpolicy_amendment: ExecPolicyAmendment } }
  | { applyNetworkPolicyAmendment:     { network_policy_amendment: NetworkPolicyAmendment } };
```

`FileChangeApprovalDecision`：

```ts
type FileChangeApprovalDecision = "accept" | "acceptForSession" | "decline" | "cancel";
```

`McpServerElicitationAction`：`"accept" | "decline" | "cancel"`。

**典型时序（命令审批）**：

```
S→C: item/started                          (commandExecution, inProgress, command, cwd)
S→C: item/commandExecution/requestApproval (id=N, command, cwd, commandActions, reason)
C→S: { id=N, result: { decision: "acceptForSession" } }
S→C: serverRequest/resolved                (threadId, requestId=N)
S→C: item/commandExecution/outputDelta * K (base64 stream)
S→C: item/completed                        (commandExecution, completed, exitCode, durationMs)
```

**典型时序（文件改动审批）**：

```
S→C: item/started                  (fileChange, inProgress, changes[])
S→C: item/fileChange/patchUpdated  (intermediate parsed snapshots, 若开启 features.apply_patch_streaming_events)
S→C: item/fileChange/requestApproval (id=M, reason?, grantRoot?)
C→S: { id=M, result: { decision: "accept" } }
S→C: serverRequest/resolved
S→C: item/fileChange/outputDelta * K
S→C: item/completed                (fileChange, completed/failed/declined)
```

**典型时序（permissions 请求扩权）**：

```
S→C: item/permissions/requestApproval (id=K, cwd, permissions: {network?, fileSystem?, …})
C→S: { id=K, result: { permissions: {fileSystem:{write:[...]}}, scope: "session" } }
S→C: serverRequest/resolved
# 后续同一 turn 内的相关命令复用该授权，不再弹窗。
```

**重要**：

- `serverRequest/resolved` 也会在 turn 中断 / 完成 / 客户端长时间无应答时被服务端主动清理时下发；客户端应用此通知释放对应的对话框。
- 若客户端不打算处理某类请求，回包 `error: { code: -32601, message: "unhandled" }` 让 server 默认按 `decline` 处理（除少数实验项外）。
- 实验性字段（如 `CommandExecutionRequestApprovalParams.additionalPermissions`）仅在 `experimentalApi=true` 时下发，否则会被 server 主动剥除。

## 11. Command Exec（独立命令执行）

`command/exec` 不依赖 thread/turn，可视为 server 给客户端复用的“受限 shell 调用”。

| 方法 | 用途 |
| --- | --- |
| `command/exec` | 启动一条命令（argv 向量），返回 `{ exitCode, stdout, stderr }`。 |
| `command/exec/write` | 给会话的 stdin 写 base64 字节，或 `closeStdin: true` 关闭。 |
| `command/exec/resize` | 调整 PTY 尺寸。 |
| `command/exec/terminate` | 终止会话。 |
| 通知 `command/exec/outputDelta` | 流式推送 stdout/stderr（base64）。 |

`CommandExecParams` 关键字段：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `command` | `string[]` | argv，**禁止为空**。 |
| `processId` | `string?` | 客户端自定义、连接内唯一；启用 `tty/streamStdin/streamStdoutStderr` 与后续 write/resize/terminate 都需要它。 |
| `tty` | `bool?` | 启用 PTY；隐式打开 `streamStdin` 和 `streamStdoutStderr`。 |
| `streamStdin` | `bool?` | 允许后续 `command/exec/write`。 |
| `streamStdoutStderr` | `bool?` | 通过通知流推送；不再在最终响应里重复。 |
| `outputBytesCap` | `number?` | 每路流的截断阈值；不能与 `disableOutputCap` 同时设置。 |
| `disableOutputCap` | `bool?` | 关闭截断（**会读完整输出，谨慎用**）。 |
| `disableTimeout` | `bool?` | 完全关闭超时；与 `timeoutMs` 互斥。 |
| `timeoutMs` | `number?` | 单次超时；默认按服务端配置。 |
| `cwd` | `string?` | 默认 server cwd。 |
| `env` | `Record<string, string \| null>?` | merge 进环境变量；值为 `null` 即从继承环境中 unset。 |
| `size` | `{ rows, cols }?` | 仅 `tty: true` 时有效。 |
| `accessPolicy` | 同 [§8](#8-turn-api-全集) `AccessPolicy` | 与 `permissionProfile` 互斥。 |
| `permissionProfile` | `PermissionProfile?` | 推荐方式；细粒度限制文件/网络/socket。 |

示例（buffered + 一次性输出）：

```jsonc
{ "id": 300, "method": "command/exec",
  "params": {
    "command": ["ls", "-la"],
    "cwd": "/Users/me/project",
    "timeoutMs": 5000
  } }
// → { "id": 300, "result": { "exitCode": 0, "stdout": "…", "stderr": "" } }
```

流式 + PTY：

```jsonc
{ "id": 310, "method": "command/exec",
  "params": { "command": ["bash","-i"], "processId":"bash-1", "tty": true, "outputBytesCap": 32768 } }
// notify: { "method":"command/exec/outputDelta",
//           "params":{"processId":"bash-1","stream":"stdout","deltaBase64":"…","capReached":false} }
// → { "id":310, "result":{ "exitCode":137, "stdout":"", "stderr":"" } } (在所有 outputDelta 发完之后)
```

`command/exec/outputDelta` 是**连接级**通知；连接断开 = server 终止该进程。

## 12. Filesystem API

所有路径必须是**绝对路径**；二进制读写采用 base64。

| 方法 | params | 返回 |
| --- | --- | --- |
| `fs/readFile` | `{ path }` | `{ dataBase64 }` |
| `fs/writeFile` | `{ path, dataBase64 }` | `{}` |
| `fs/createDirectory` | `{ path, recursive? = true }` | `{}` |
| `fs/getMetadata` | `{ path }` | `{ isDirectory, isFile, isSymlink, createdAtMs, modifiedAtMs }`（时间戳不可用时为 `0`） |
| `fs/readDirectory` | `{ path }` | `{ entries: [{ fileName, isDirectory, isFile }] }` |
| `fs/remove` | `{ path, recursive? = true, force? = true }` | `{}` |
| `fs/copy` | `{ sourcePath, destinationPath, recursive? }` | `{}` |
| `fs/watch` | `{ watchId, path }` | `{ path }`（已规范化） |
| `fs/unwatch` | `{ watchId }` | `{}` |
| 通知 `fs/changed` | — | `{ watchId, changedPaths }` |

`watchId` 由客户端生成、连接内唯一。客户端断开 = 所有 watch 释放。建议使用 UUIDv4 / v7。

## 13. Account / 鉴权 API

| 方法 | 说明 |
| --- | --- |
| `account/read` | 查询：`{ account: null \| {type:"apiKey"} \| {type:"chatgpt", email, planType} \| {type:"amazonBedrock"}, requiresProviderAuth }`。`refreshToken: true` 可强制刷新。 |
| `account/login/start` | 启动登录，按 `params.type` 分四种：`apiKey` / `chatgpt` / `chatgptDeviceCode` / `chatgptAuthTokens`（外部宿主托管）。 |
| `account/login/cancel` | 取消未完成的 ChatGPT 登录（按 `loginId`）。 |
| `account/logout` | 注销当前账户；下发 `account/updated` 把 `authMode` 置 `null`。 |
| `account/rateLimits/read` | 读取 ChatGPT 限速窗口；状态变化通过 `account/rateLimits/updated` 通知。 |
| `account/sendAddCreditsNudgeEmail` | 触发后端给工作区所有者发提醒邮件。 |

**LoginAccountParams** 全形态：

```ts
type LoginAccountParams =
  | { type: "apiKey", apiKey: string }
  | { type: "chatgpt" }
  | { type: "chatgptDeviceCode" }
  | { type: "chatgptAuthTokens", accessToken: string, chatgptAccountId: string, chatgptPlanType?: string | null };
```

**LoginAccountResponse**：

```ts
type LoginAccountResponse =
  | { type: "apiKey" }
  | { type: "chatgpt",            loginId: string, authUrl: string }
  | { type: "chatgptDeviceCode",  loginId: string, verificationUrl: string, userCode: string }
  | { type: "chatgptAuthTokens" };
```

**通知**：

- `account/login/completed`：`{ loginId, success, error }`。
- `account/updated`：`{ authMode: "apikey"|"chatgpt"|"agentIdentity"|"chatgptAuthTokens"|null, planType?: PlanType|null }`。
- `account/rateLimits/updated`：`{ rateLimits }`。

**登录时序（ChatGPT 浏览器流）**：

```
C→S: account/login/start { type: "chatgpt" }
S→C: result { type:"chatgpt", loginId, authUrl }   ← 客户端打开 authUrl
# 用户在浏览器完成；server 内置本地 callback 监听。
S→C: account/login/completed { loginId, success: true }
S→C: account/updated { authMode: "chatgpt", planType: "plus" }
```

**登录时序（Device Code）**：

```
C→S: account/login/start { type: "chatgptDeviceCode" }
S→C: result { type:"chatgptDeviceCode", loginId, verificationUrl, userCode }
# 客户端把 verificationUrl/userCode 显示给用户。
S→C: account/login/completed { loginId, success: true }
S→C: account/updated { authMode: "chatgpt", planType }
```

## 14. Config / 配置

| 方法 | 说明 |
| --- | --- |
| `config/read` | `{ includeLayers, cwd? }`；返回 `{ config, origins, layers? }`。`origins` 标注每个字段最终来源（user / project / managed / env / runtime）。 |
| `config/value/write` | 单 key 写 user 级 `config.toml`。 |
| `config/batchWrite` | 原子写多 key；`reloadUserConfig: true` 时热加载已加载 thread。 |
| `configRequirements/read` | 读取 `requirements.toml` / MDM 约束（许可的审批/访问/搜索模式、网络白名单、托管 hooks 等）。 |
| `config/mcpServer/reload` | 热加载 MCP server 配置；下一次该 thread 活跃 turn 时生效。 |
| `experimentalFeature/list` | 列出 feature flag（含 `beta/underDevelopment/stable` 阶段、是否启用、是否默认启用）；支持游标分页。 |
| `experimentalFeature/enablement/set` | 进程内运行时 override（不写盘）；支持的键见 README，影响优先级低于云端策略与启动参数。 |
| `externalAgentConfig/detect` | 探测可迁移的外部 agent 资产（Claude/Codex 等）。 |
| `externalAgentConfig/import` | 执行迁移；带 plugin 的迁移完成后异步下发 `externalAgentConfig/import/completed`。 |

注意：**`config/*` 系列的字段使用 snake_case**（与 config.toml 同形），其余 v2 API 一律 camelCase。

## 15. Skills / Apps / Plugins / Marketplace

| 方法 | 说明 |
| --- | --- |
| `skills/list` | `{ cwds: string[], forceReload?, perCwdExtraUserRoots? }`；返回各 cwd 的 skill 列表（含 `enabled`、`interface.iconSmall/...`）。 |
| `skills/config/write` | 启停 skill（按 `path` 或 `name`）。 |
| 通知 `skills/changed` | 本地 skill 文件变动时下发。 |
| `app/list` | `{ cursor?, limit?, threadId?, forceRefetch? }`；返回 connector / app 列表与 `isAccessible/isEnabled`。 |
| 通知 `app/list/updated` | accessible apps / directory apps 任一刷新完成时下发。 |
| `marketplace/add/remove/upgrade` | 用户级 marketplace 管理（Git / GitHub shorthand）。 |
| `plugin/list` / `plugin/read` | 浏览 marketplace 内 plugin，含 `marketplaceLoadErrors`、`featuredPluginIds`。 |
| `plugin/install` / `plugin/uninstall` | 本地 plugin 安装/卸载；远程 ChatGPT plugin 走 backend forwarding。 |

**触发 skill / app / plugin** 都通过 `turn/start.input` 中的 mention：

```ts
// Skill: $<skill-name> + 推荐附 skill item
{ type: "text",    text: "$skill-creator 给 flaky CI triaging 加一个 skill" }
{ type: "skill",   name: "skill-creator", path: "/abs/path/SKILL.md" }

// App: $<app-slug> + mention(app://…)
{ type: "text",    text: "$demo-app Summarize updates" }
{ type: "mention", name: "Demo App", path: "app://demo-app" }

// Plugin: @<plugin> + mention(plugin://…)
{ type: "text",    text: "@sample Summarize updates" }
{ type: "mention", name: "Sample Plugin", path: "plugin://sample@test" }
```

## 16. MCP 集成

| 方法 | 说明 |
| --- | --- |
| `mcpServerStatus/list` | 枚举所有配置的 MCP server，带 `tools[]`、auth 状态、（detail=full 时）resources/templates。 |
| `mcpServer/oauth/login` | 启动 server 的 OAuth；返回 `authorization_url`；完成后下发 `mcpServer/oauthLogin/completed`。 |
| `mcpServer/resource/read` | 读取 server 上的 resource。`threadId` 缺省时从最新 config 读取。 |
| `mcpServer/tool/call` | 调用某 thread 配置下的某 server 的某个 tool；返回 RMCP 风格的 `result/content`。 |
| 通知 `mcpServer/startupStatus/updated` | server 启动状态机：`starting / ready / failed / cancelled`。 |
| 通知 `mcpServer/elicitation/request` | server 反向请求 user 输入（form 或 url 模式）；客户端按服务端发起请求处理。 |
| 通知 `item/mcpToolCall/progress` | tool call 的进度增量。 |

## 17. Device Key（设备签名）

`device/key/create`、`device/key/public`、`device/key/sign` **仅在本地传输（stdio / in-process）下可用**。远程 ws / unix 远程控制 / remote-control 连接会被 server 拒绝（参见 `transport/mod.rs:ConnectionOrigin::allows_device_key_requests`）。

- 推荐使用硬件保护类（TPM / Secure Enclave）；
- 仅当 `protectionPolicy: "allow_os_protected_nonextractable"` 时允许 OS 保护的非可导出软件密钥；
- `device/key/sign` 仅接受结构化 payload `remoteControlClientConnection`，**不是**任意字节签名。

## 18. 实时（Realtime）API（实验）

所有 `thread/realtime/*` 方法和通知均需要 `capabilities.experimentalApi = true`。

| 方法 | 说明 |
| --- | --- |
| `thread/realtime/start` | `{ threadId, outputModality: "text"\|"audio", prompt?, sessionId?, transport? }`。默认 WebSocket 模式；`transport: {type:"webrtc", sdp}` 时由 server 与 OpenAI Realtime 建立 WebRTC，answer SDP 通过 `thread/realtime/sdp` 通知下发。 |
| `thread/realtime/appendAudio` | 追加输入音频。 |
| `thread/realtime/appendText` | 追加文本输入。 |
| `thread/realtime/stop` | 停止实时会话。 |
| `thread/realtime/listVoices` | 列出可用 voice。 |

通知：`thread/realtime/started`、`itemAdded`、`transcript/delta`、`transcript/done`、`outputAudio/delta`、`sdp`、`error`、`closed`。

`thread/realtime/outputAudio/delta` 体积较大，UI 不需要音频时建议放入 `optOutNotificationMethods`。

## 19. 错误模型

**协议层错误**（JSON-RPC error 响应）：

- `-32600 Invalid Request`：握手缺失 / 重复初始化 / 方法不允许（如远程连接调用 `device/key/*`） / 实验性 API 未开能力位。
- `-32602 Invalid Params`：字段缺失、类型错误、`accessMode` 与 `permissionProfile` 同时存在、`outputBytesCap` 与 `disableOutputCap` 互斥冲突、路径非绝对等。
- `-32603 Internal Error`：服务端 panic / 不可恢复错误；通常应记录 `RUST_LOG=debug` 复现。
- `-32001 Overloaded`：队列满；指数退避后重试。

**Turn 失败**：`turn/completed.turn.status === "failed"`，`turn.error`：

```ts
type TurnError = { message: string, agereErrorInfo: AgereErrorInfo | null, additionalDetails: string | null };

type AgereErrorInfo =
  | "contextWindowExceeded" | "usageLimitExceeded" | "serverOverloaded" | "cyberPolicy"
  | { httpConnectionFailed:           { httpStatusCode: number | null } }
  | { responseStreamConnectionFailed: { httpStatusCode: number | null } }
  | "internalServerError" | "unauthorized" | "badRequest" | "threadRollbackFailed" | "accessError"
  | { responseStreamDisconnected:     { httpStatusCode: number | null } }
  | { responseTooManyFailedAttempts:  { httpStatusCode: number | null } }
  | { activeTurnNotSteerable:         { turnKind: NonSteerableTurnKind } }
  | "other";
```

**Error 通知**：`error` 在 turn 期间任何不可重试错误发生时下发；`{ error, willRetry, threadId, turnId }`。`willRetry: true` 时通常紧跟 `thread/rateLimit/waiting` 表明正在排队重试。

**客户端处理建议**：

- `contextWindowExceeded`：提示用户使用 `thread/compact/start` 压缩，或 `thread/rollback`。
- `usageLimitExceeded`：触发 `account/sendAddCreditsNudgeEmail` 或引导切换 API key。
- `httpConnectionFailed{httpStatusCode}`：按状态码区分；4xx 通常是配置问题，5xx 与网络层抖动。
- `activeTurnNotSteerable`：当前 turn 是 review/compact 等不可 steer 类型；UI 需提示用户改用新 turn。
- `unauthorized`：检查 `account/read.requiresProviderAuth` 后引导 `account/login/start`。

## 20. 限流与背压

- **协议级背压**：入站请求队列满 → `-32001`。
- **模型 / 后端限流**：`thread/rateLimit/waiting` 通知：

  ```ts
  type ThreadRateLimitWaitingNotification = {
    threadId: string,
    turnId: string,
    attempt: number,
    maxAttempts: number,   // 0 表示无限重试
    resumeAt: number,      // Unix seconds
    waitSeconds: number,
    reason: string,
  };
  ```

- **ChatGPT 速率**：通过 `account/rateLimits/read` + `account/rateLimits/updated` 跟踪。
- **客户端节流**：建议客户端自己维持 `inFlightRequests` 计数，超过软上限（如 64）时暂缓发起读类 RPC（`thread/list`、`fs/*`、`mcpServerStatus/list`），不要阻塞 turn 流。

## 21. 实验性 API 与能力协商

- 默认（`experimentalApi: false`）：server 把所有实验性方法/字段/枚举变体/通知**裁剪**掉，调用会被 `-32600 "<descriptor> requires experimentalApi capability"` 拒绝。
- `descriptor` 形如：
  - `mock/experimentalMethod`（整方法）
  - `thread/start.mockExperimentalField`（字段）
  - `askForApproval.granular`（枚举变体）
- 一旦开启，**整个连接**都会暴露实验项；不能逐请求开关。
- 即使开启，仍要做好向后不兼容的心理准备：实验项语义、字段名、wire 格式可能在小版本里变化。

**当前主要实验项目（非完整列表）**：

- 方法：`collaborationMode/list`、`thread/memoryMode/set`、`memory/reset`、`thread/backgroundTerminals/clean`、`thread/realtime/*`、`fuzzyFileSearch/sessionStart/Update/Stop`、`thread/increment_elicitation`、`thread/decrement_elicitation`、`mock/experimentalMethod`。
- 字段：`thread/start.persistExtendedHistory`、`thread/start.dynamicTools`、`turn/start.environments`、`turn/start.outputSchema`、`CommandExecutionRequestApprovalParams.additionalPermissions`、`AskForApproval.granular`。
- 通知：`thread/realtime/*`、`item/plan/delta`、`fuzzyFileSearch/session*`。

## 22. 通知 opt-out 与降噪

在 `initialize.capabilities.optOutNotificationMethods` 传入**精确**方法名列表：

```jsonc
{
  "id": 0, "method": "initialize",
  "params": {
    "clientInfo": { "name": "low_noise", "title": "", "version": "0.1.0" },
    "capabilities": {
      "experimentalApi": false,
      "optOutNotificationMethods": [
        "item/reasoning/summaryTextDelta",
        "item/reasoning/textDelta",
        "thread/tokenUsage/updated",
        "thread/realtime/outputAudio/delta"
      ]
    }
  }
}
```

- 不支持通配符 / 前缀匹配；未知方法名被静默忽略；
- 仅影响通知，不影响请求 / 响应 / 服务端发起的请求；
- 客户端可在不同连接上选不同 opt-out 列表（例如背景守护连接只订阅 `account/*`、`app/*`，UI 连接订阅 `item/*`、`turn/*`）。

## 23. 重连、订阅与清理

- 一个连接 = 一组订阅；连接关闭立即解除所有订阅（`fs/watch`、`command/exec` 流、thread 订阅、device key 会话等）。
- 客户端遇到 `-32001` 或网络断开 → 重连 → 重新 `initialize` → 对关心的 thread 调用 `thread/resume` 或 `thread/read` → 通过 `thread/turns/list` 拉缺失的历史。
- `thread/unsubscribe` 不会立刻卸载 thread：最后一个订阅者退出后 30 分钟无活动才卸载，并下发 `thread/closed` + `thread/status/changed { type:"notLoaded" }`。
- `command/exec` 处于 PTY 模式的会话**严格连接级**：断开 = server 杀掉进程。
- 长生命周期的桌面客户端建议两条连接：一条做 UI 流（订阅 thread）；一条做后台维护（账户、配置、apps 列表），断重连互不影响。

## 24. 安全模型

- **远程暴露**：默认仅 loopback 与 unix socket 安全；公网 / Tailscale / 跳板机访问必须启用 `--ws-auth`，并把 `Authorization: Bearer …` 视为机密。
- **审批**：客户端应在 UI 上**完整展示** server 提供的 `command`、`cwd`、`commandActions`、`reason`，避免把审批简化成“一键允许”。
- **`thread/shellCommand`**：以**完全宿主权限**运行，不走 thread 的 `accessPolicy` / `permissionProfile`。客户端**不能**用它来执行用户没有亲自键入的命令。
- **路径**：所有 `fs/*` 与 `cwd` 必须绝对路径；客户端在向 server 传入路径前应自行 normalize、resolve symlink，避免 server 端报 `-32602`。
- **device key**：仅本地传输可用；客户端不要尝试在 ws / remote control 上调用。
- **Origin/CSRF**：`ws://` 携带 `Origin` 头会被 server 直接 403；浏览器 / Web 前端接入建议使用 native WebSocket（不发 Origin）或者通过 Tauri / Electron native bridge 调用。
- **secret 注入**：避免把 API Key 放到 `turn/start.input.text` 里；密钥优先通过 `account/login/start` 或 config.toml 注入。

## 25. 生成强类型 SDK

`agere app-server` 提供自描述 schema 导出，与正在运行的 server 版本**完全一致**：

```bash
# TypeScript（默认只导出稳定 API）
agere app-server generate-ts --out ./generated/ts
agere app-server generate-ts --out ./generated/ts --experimental

# JSON Schema 包
agere app-server generate-json-schema --out ./generated/jsonschema
agere app-server generate-json-schema --out ./generated/jsonschema --experimental
```

输出与本仓库内置 schema 一致：

- `app-server-protocol/schema/typescript/v2/*.ts`：每个 params/response/notification/payload 一个文件。
- `app-server-protocol/schema/json/v2/*.json`：JSON Schema；`ClientRequest.json`、`ServerNotification.json`、`ServerRequest.json` 是聚合 schema。
- `app-server-protocol/schema/json/agere_app_server_protocol.v2.schemas.json`：v2 总包。

**推荐工作流**：

1. 客户端发布时锁定一个 `agere` 版本。
2. 在 CI 中 `agere app-server generate-ts --out ./src/generated`（必要时加 `--experimental`）。
3. 客户端代码以 `import type { TurnStartParams } from "./generated/v2/TurnStartParams"` 形式做强类型。
4. 升级 `agere` 时重跑 generate 并 review diff。

其它语言（Go / Python / Rust）建议从 JSON Schema 生成：

- Go：`oapi-codegen`（把 JSON Schema 包成 OpenAPI 定义）或 `go-jsonschema`。
- Python：`datamodel-code-generator --input-file-type jsonschema`。
- Rust：可直接复用 `agere-app-server-protocol` crate；外部生成可用 `typify`。

## 26. 可观测性与调试

| 信号 | 来源 | 用途 |
| --- | --- | --- |
| stderr 日志（NDJSON / 文本） | `RUST_LOG` + `LOG_FORMAT=json` | server 内部状态。 |
| `configWarning` / `warning` / `guardianWarning` / `deprecationNotice` 通知 | server → client | 配置与运行期警告，建议在 UI 角落集中展示。 |
| `thread/rateLimit/waiting` 通知 | 模型 backend | 渲染倒计时；不要立刻报错。 |
| `model/rerouted`、`model/verification` 通知 | server | 通知用户当前路由变化 / 额外身份验证。 |
| `serverRequest/resolved` 通知 | server | 清理对应审批/工具弹窗。 |
| `trace` 字段（请求） | 客户端发起 | 注入 W3C Trace Context，方便在自家 APM 关联调用。 |

**离线复现**：

- `agere app-server --listen stdio:// 2> server.log` 捕获 stderr。
- 客户端层把所有 stdin/stdout 帧 mirror 到一个 JSONL 文件 → 对比 `tests/suite/v2/*.rs` 里官方 fixture。
- 服务端集成测试入口 `app-server/tests/suite/v2/*.rs` 是最权威的 RPC 行为参考。

## 27. 常见 FAQ / 陷阱

- **`Not initialized` 错误**：每个连接都要 `initialize`；不要复用上一进程已经握手过的客户端状态。
- **`Already initialized`**：同一连接发了两次 `initialize`，断重连解决；服务端不会原地“重置”。
- **审批后 turn 卡住**：客户端忘了对 `requestApproval` 回包；`id` 必须用 server 发来的那个。
- **大文件 `fs/readFile` OOM**：单次返回上限取决于 server 默认（约 64MB 量级），建议自己分块读或直接走 `command/exec` 走 `cat | head -c N`。
- **WebSocket 403**：客户端发了 `Origin` 头；改用 native WebSocket / 后端代理。
- **`device/key/*` Forbidden**：你在 ws/remote 连接上调用了它；改为 stdio / unix socket / in-process。
- **同一 thread 多个 turn 并发**：服务端串行处理；并发 `turn/start` 会被拒绝或排队，UI 上应在 `turn/completed` 之前禁用提交按钮。
- **`AccessMode` vs `PermissionProfile`**：互斥；不要同时传；新代码推荐 `permissionProfile`。
- **`outputBytesCap` 与 `disableOutputCap`**：互斥；同时传 = `-32602`。
- **Realtime audio 通知 OOM**：音频通知体积大，UI 不需要时务必 opt-out。
- **重连后想跟随之前的 turn**：`thread/resume` 即可拿到当前活跃 turn；订阅会重新建立，后续 `item/*` 与 `turn/completed` 通知正常下发。

---

## 附录 A：客户端可调用方法速查表

按字典序排列；⚠️ 表示需要 `capabilities.experimentalApi = true`；🔒 表示仅本地传输（stdio / in-process）可用。

| 方法 | 备注 |
| --- | --- |
| `account/login/cancel` | |
| `account/login/start` | 多变体 params |
| `account/logout` | |
| `account/rateLimits/read` | |
| `account/read` | |
| `account/sendAddCreditsNudgeEmail` | |
| `app/list` | |
| `collaborationMode/list` | ⚠️ |
| `command/exec` | |
| `command/exec/resize` | 需 PTY |
| `command/exec/terminate` | |
| `command/exec/write` | |
| `config/batchWrite` | snake_case |
| `config/mcpServer/reload` | |
| `config/read` | snake_case |
| `config/value/write` | snake_case |
| `configRequirements/read` | |
| `device/key/create` | 🔒 |
| `device/key/public` | 🔒 |
| `device/key/sign` | 🔒，仅 `remoteControlClientConnection` payload |
| `experimentalFeature/enablement/set` | |
| `experimentalFeature/list` | |
| `externalAgentConfig/detect` | |
| `externalAgentConfig/import` | |
| `feedback/upload` | |
| `fs/copy` | 绝对路径，目录需 `recursive:true` |
| `fs/createDirectory` | |
| `fs/getMetadata` | |
| `fs/readDirectory` | |
| `fs/readFile` | base64 |
| `fs/remove` | 默认 `recursive:true`、`force:true` |
| `fs/unwatch` | |
| `fs/watch` | 客户端提供 `watchId` |
| `fs/writeFile` | base64 |
| `fuzzyFileSearch` | 旧兼容 |
| `fuzzyFileSearch/sessionStart` | ⚠️ |
| `fuzzyFileSearch/sessionStop` | ⚠️ |
| `fuzzyFileSearch/sessionUpdate` | ⚠️ |
| `initialize` | 握手 |
| `marketplace/add` | |
| `marketplace/remove` | |
| `marketplace/upgrade` | |
| `mcpServer/oauth/login` | |
| `mcpServer/resource/read` | |
| `mcpServer/tool/call` | |
| `mcpServerStatus/list` | |
| `memory/reset` | ⚠️ |
| `model/list` | |
| `modelProvider/capabilities/read` | |
| `plugin/install` | |
| `plugin/list` | |
| `plugin/read` | |
| `plugin/uninstall` | |
| `review/start` | |
| `skills/config/write` | |
| `skills/list` | |
| `thread/approveGuardianDeniedAction` | |
| `thread/archive` | |
| `thread/backgroundTerminals/clean` | ⚠️ |
| `thread/compact/start` | |
| `thread/decrement_elicitation` | ⚠️ |
| `thread/fork` | |
| `thread/goal/clear` | |
| `thread/goal/get` | |
| `thread/goal/set` | |
| `thread/increment_elicitation` | ⚠️ |
| `thread/inject_items` | |
| `thread/list` | |
| `thread/loaded/list` | |
| `thread/memoryMode/set` | ⚠️ |
| `thread/metadata/update` | |
| `thread/name/set` | |
| `thread/provider/update` | |
| `thread/read` | |
| `thread/realtime/appendAudio` | ⚠️ |
| `thread/realtime/appendText` | ⚠️ |
| `thread/realtime/listVoices` | ⚠️ |
| `thread/realtime/start` | ⚠️ |
| `thread/realtime/stop` | ⚠️ |
| `thread/resume` | |
| `thread/rollback` | |
| `thread/shellCommand` | 完全宿主权限 |
| `thread/start` | |
| `thread/turns/list` | |
| `thread/unarchive` | |
| `thread/unsubscribe` | |
| `turn/interrupt` | |
| `turn/start` | |
| `turn/steer` | |
| `usage/read` | |

## 附录 B：服务端通知速查表

| 通知 | 说明 |
| --- | --- |
| `account/login/completed` | 登录完成（成功/失败）。 |
| `account/rateLimits/updated` | ChatGPT 速率窗口变化。 |
| `account/updated` | 认证状态切换；含 `authMode` 与 `planType`。 |
| `app/list/updated` | apps 列表刷新；常用来驱动 UI 重渲染。 |
| `command/exec/outputDelta` | 独立命令流式输出。 |
| `configWarning` | 配置警告 `{summary, details?, path?, range?}`。 |
| `deprecationNotice` | 客户端使用了即将废弃的方法/字段。 |
| `error` | turn 中不可恢复错误；`willRetry` 区分是否会自动重试。 |
| `externalAgentConfig/import/completed` | 迁移结束（含异步）。 |
| `fs/changed` | 监听到的文件变化。 |
| `fuzzyFileSearch/sessionCompleted` | ⚠️ 模糊搜索 session 完成。 |
| `fuzzyFileSearch/sessionUpdated` | ⚠️ 模糊搜索增量结果。 |
| `guardianWarning` | Guardian / auto-review 警告。 |
| `hook/started` | 配置的 hooks 开始执行。 |
| `hook/completed` | hooks 结束。 |
| `item/agentMessage/delta` | agent 文本增量。 |
| `item/autoApprovalReview/started` | [unstable] auto-review 开始。 |
| `item/autoApprovalReview/completed` | [unstable] auto-review 结束。 |
| `item/commandExecution/outputDelta` | 命令输出增量。 |
| `item/commandExecution/terminalInteraction` | terminal 交互事件。 |
| `item/completed` | item 完成（authoritative state）。 |
| `item/fileChange/outputDelta` | apply_patch 输出增量。 |
| `item/fileChange/patchUpdated` | apply_patch 结构化快照（需开 feature）。 |
| `item/mcpToolCall/progress` | MCP 工具调用进度。 |
| `item/plan/delta` | ⚠️ plan 增量。 |
| `item/reasoning/summaryPartAdded` | reasoning summary 分段边界。 |
| `item/reasoning/summaryTextDelta` | reasoning summary 增量。 |
| `item/reasoning/textDelta` | reasoning raw 增量（OSS 模型常见）。 |
| `item/started` | item 创建（render 占位）。 |
| `mcpServer/oauthLogin/completed` | OAuth 完成。 |
| `mcpServer/startupStatus/updated` | MCP server 启动状态机。 |
| `model/rerouted` | 模型被路由到其它实例。 |
| `model/verification` | 后端要求额外身份验证。 |
| `rawResponseItem/completed` | 内部使用（Agere Cloud）。 |
| `remoteControl/status/changed` | remote-control 状态机。 |
| `serverRequest/resolved` | 服务端请求被回包或被生命周期清理。 |
| `skills/changed` | 监控到本地 skill 文件变化。 |
| `thread/archived` / `unarchived` | 归档状态切换。 |
| `thread/closed` | thread 已卸载。 |
| `thread/compacted` | DEPRECATED，改用 `contextCompaction` item。 |
| `thread/goal/cleared` / `updated` | goal 变更。 |
| `thread/name/updated` | 名称更新。 |
| `thread/rateLimit/waiting` | turn 因 429 等待重试。 |
| `thread/realtime/closed` | ⚠️ 实时会话关闭。 |
| `thread/realtime/error` | ⚠️ 实时会话错误。 |
| `thread/realtime/itemAdded` | ⚠️ 实时原始 item（handoff_request 等）。 |
| `thread/realtime/outputAudio/delta` | ⚠️ 输出音频；体积大，建议 opt-out。 |
| `thread/realtime/sdp` | ⚠️ WebRTC SDP answer。 |
| `thread/realtime/started` | ⚠️ 实时会话开始。 |
| `thread/realtime/transcript/delta` / `done` | ⚠️ 转写流。 |
| `thread/started` | thread 加入；含初始 status 快照。 |
| `thread/status/changed` | 状态机切换；首次 `notLoaded → idle/active` 不补发。 |
| `thread/tokenUsage/updated` | token usage 累计变化。 |
| `turn/completed` | turn 终态。 |
| `turn/diff/updated` | turn 级 unified diff 快照。 |
| `turn/plan/updated` | turn 内 plan 步骤变化。 |
| `turn/started` | turn 开始模型推理。 |
| `warning` | 通用警告。 |
| `windows/worldWritableWarning` | Windows 上目录权限不安全的提示。 |

## 附录 C：服务端发起的请求方法速查表

| 方法 | 客户端期望响应 |
| --- | --- |
| `item/commandExecution/requestApproval` | `{ decision: CommandExecutionApprovalDecision }` |
| `item/fileChange/requestApproval` | `{ decision: FileChangeApprovalDecision }` |
| `item/permissions/requestApproval` | `{ permissions, scope, strictAutoReview? }` |
| `item/tool/requestUserInput`（⚠️） | `{ answers: Record<questionId, ToolRequestUserInputAnswer> }` |
| `item/tool/call`（⚠️） | `{ contentItems: DynamicToolCallOutputContentItem[], success: boolean }` |
| `mcpServer/elicitation/request` | `{ action: "accept"\|"decline"\|"cancel", content: JsonValue \| null, _meta? }` |
| `account/chatgptAuthTokens/refresh` | `ChatgptAuthTokensRefreshResponse`（仅 `chatgptAuthTokens` 模式） |
| `execCommandApproval`（v1 兼容） | `{ decision: ReviewDecision }` |
| `applyPatchApproval`（v1 兼容） | `{ decision: ReviewDecision }` |

## 附录 D：典型时序图

### D.1 完整一轮对话（含命令审批 + 文件改动审批）

```
C→S: initialize / initialized
C→S: thread/start { cwd }
S→C: result { thread }
S→C: thread/started { thread }
C→S: turn/start { threadId, input }
S→C: result { turn (inProgress) }
S→C: turn/started { threadId, turn }
S→C: item/started     (reasoning)
S→C: item/reasoning/summaryTextDelta * K
S→C: item/completed   (reasoning)
S→C: item/started     (commandExecution, command="rg TODO")
S→C: item/commandExecution/requestApproval [id=A]
C→S: { id=A, result: { decision: "acceptForSession" } }
S→C: serverRequest/resolved
S→C: item/commandExecution/outputDelta * K
S→C: item/completed   (commandExecution, exitCode=0)
S→C: item/started     (fileChange, in-progress)
S→C: item/fileChange/requestApproval [id=B]
C→S: { id=B, result: { decision: "accept" } }
S→C: serverRequest/resolved
S→C: item/fileChange/outputDelta * K
S→C: item/completed   (fileChange, completed)
S→C: turn/diff/updated { diff }
S→C: item/started + item/agentMessage/delta * K + item/completed   (agentMessage)
S→C: thread/tokenUsage/updated
S→C: turn/completed   (status: "completed")
```

### D.2 用户在 turn 中追加输入（steer）

```
S→C: turn/started
# 用户在 UI 上点了“追加”
C→S: turn/steer { threadId, expectedTurnId: <活跃 turnId>, input }
S→C: result { turnId }
# Item 流继续在同一 turn 下推
...
S→C: turn/completed
```

### D.3 中断

```
S→C: turn/started + item/started ... (running)
C→S: turn/interrupt { threadId, turnId }
S→C: result {}
# server 取消正在跑的工具，回滚 item，最终：
S→C: turn/completed { turn: { status: "interrupted" } }
```

### D.4 重连恢复

```
# 旧连接掉线 → 客户端发起新的物理连接
C→S: initialize / initialized
C→S: thread/resume { threadId, excludeTurns: true }
S→C: result { thread, model, ... }
S→C: thread/started + thread/tokenUsage/updated
# 如果 server 还在跑该 thread 的 turn：
S→C: thread/status/changed { status: { type: "active", activeFlags: [...] } }
S→C: item/started ... item/completed ... turn/completed
# 客户端按需拉历史：
C→S: thread/turns/list { threadId, limit: 50 }
S→C: result { data, nextCursor }
```

---

> 协议如有差异，请以 `app-server-protocol/src/protocol/{common,v1,v2}.rs` 与 `app-server-protocol/schema/typescript/v2/`、`app-server-protocol/schema/json/v2/` 中**当前版本**的 schema 为准；英文权威说明见 `app-server/README.md`。
