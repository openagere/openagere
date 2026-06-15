# agere-exec-server

`agere-exec-server` is the library backing `agere exec-server`, a small
JSON-RPC server for spawning and controlling subprocesses through
`agere-utils-pty`.

It provides:

- a CLI entrypoint: `agere exec-server`
- a Rust client: `ExecServerClient`
- a small protocol module with shared request/response types

This crate owns the transport, protocol, and filesystem/process handlers. The
top-level `agere` binary owns hidden helper dispatch (including filesystem
helpers) as needed by the platform; Linux re-exec aliases use **`agere-linux-helper`**
(argv0 basename; see [`agere_exec_server::AGERE_LINUX_HELPER_ARG0`](src/runtime_paths.rs)).

## Transport

The server speaks the shared `agere-app-server-protocol` message envelope on
the wire.

The CLI entrypoint supports:

- `ws://IP:PORT` (default)

Wire framing:

- websocket: one JSON-RPC message per websocket text frame

## Lifecycle

Each connection follows this sequence:

1. Send `initialize`.
2. Wait for the `initialize` response.
3. Send `initialized`.
4. Call process or filesystem RPCs.

If the server receives any notification other than `initialized`, it replies
with an error using request id `-1`.

If the websocket connection closes, the server terminates any remaining managed
processes for that client connection.

## API

### `initialize`

Initial handshake request.

Request params:

```json
{
  "clientName": "my-client"
}
```

Response:

```json
{}
```

### `initialized`

Handshake acknowledgement notification sent by the client after a successful
`initialize` response.

Params are currently ignored. Sending any other notification method is treated
as an invalid request.

### `process/start`

Starts a new managed process.

Request params:

```json
{
  "processId": "proc-1",
  "argv": ["bash", "-lc", "printf 'hello\\n'"],
  "cwd": "/absolute/working/directory",
  "env": {
    "PATH": "/usr/bin:/bin"
  },
  "tty": true,
  "pipeStdin": false,
  "arg0": null
}
```

Field definitions:

- `processId`: caller-chosen stable id for this process within the connection.
- `argv`: command vector. It must be non-empty.
- `cwd`: absolute working directory used for the child process.
- `env`: environment variables passed to the child process.
- `tty`: when `true`, spawn a PTY-backed interactive process.
- `pipeStdin`: when `true`, keep non-PTY stdin writable via `process/write`.
- `arg0`: optional argv0 override forwarded to `agere-utils-pty`.

Response:

```json
{
  "processId": "proc-1"
}
```

Behavior notes:

- Reusing an existing `processId` is rejected.
- PTY-backed processes accept later writes through `process/write`.
- Non-PTY processes reject writes unless `pipeStdin` is `true`.
- Output is streamed asynchronously via `process/output`.
- Exit is reported asynchronously via `process/exited`.

### `process/read`

Reads buffered output and terminal state for a managed process.

Request params:

```json
{
  "processId": "proc-1",
  "afterSeq": null,
  "maxBytes": 65536,
  "waitMs": 1000
}
```

Field definitions:

- `processId`: managed process id returned by `process/start`.
- `afterSeq`: optional sequence number cursor; when present, only newer chunks
  are returned.
- `maxBytes`: optional response byte budget.
- `waitMs`: optional long-poll timeout in milliseconds.

Response:

```json
{
  "chunks": [],
  "nextSeq": 1,
  "exited": false,
  "exitCode": null,
  "closed": false,
  "failure": null
}
```

### `process/write`

Writes raw bytes to a running process stdin.

Request params:

```json
{
  "processId": "proc-1",
  "chunk": "aGVsbG8K"
}
```

`chunk` is base64-encoded raw bytes. In the example above it is `hello\n`.

Response:

```json
{
  "status": "accepted"
}
```

Behavior notes:

- Writes to an unknown `processId` are rejected.
- Writes to a non-PTY process are rejected unless it started with `pipeStdin`.

### `process/terminate`

Terminates a running managed process.

Request params:

```json
{
  "processId": "proc-1"
}
```

Response:

```json
{
  "running": true
}
```

If the process is already unknown or already removed, the server responds with:

```json
{
  "running": false
}
```

## Notifications

### `process/output`

Streaming output chunk from a running process.

Params:

```json
{
  "processId": "proc-1",
  "seq": 1,
  "stream": "stdout",
  "chunk": "aGVsbG8K"
}
```

Fields:

- `processId`: process identifier
- `seq`: per-process output sequence number
- `stream`: `"stdout"`, `"stderr"`, or `"pty"`
- `chunk`: base64-encoded output bytes

### `process/exited`

Final process exit notification.

Params:

```json
{
  "processId": "proc-1",
  "seq": 2,
  "exitCode": 0
}
```

### `process/closed`

Notification emitted after process output is closed and the process handle is
removed.

Params:

```json
{
  "processId": "proc-1"
}
```

## Filesystem RPCs

Filesystem methods use absolute paths and return JSON-RPC errors for invalid
or unavailable paths:

- `fs/readFile`
- `fs/writeFile`
- `fs/createDirectory`
- `fs/getMetadata`
- `fs/readDirectory`
- `fs/remove`
- `fs/copy`

Each filesystem request accepts an optional **`filesystemAccess`** JSON object (Rust field
`filesystem_access`, type `FileSystemAccessContext` from `agere-file-system`). When
present, it carries the resolved permission profile and working directory so the executor
can enforce the same filesystem envelope as the hosting Agere session. Embedded builds may
leave it unset when the exec server is used as a straight passthrough.

## Errors

The server returns JSON-RPC errors with these codes:

- `-32600`: invalid request
- `-32602`: invalid params
- `-32603`: internal error

Typical error cases:

- unknown method
- malformed params
- empty `argv`
- duplicate `processId`
- writes to unknown processes
- writes to non-PTY processes
- filesystem operations rejected by the active access policy

## Rust surface

The crate exports:

- `ExecServerClient`
- `ExecServerError`
- `ExecServerClientConnectOptions`
- `RemoteExecServerConnectArgs`
- protocol request/response structs for process and filesystem RPCs
- `DEFAULT_LISTEN_URL` and `ExecServerListenUrlParseError`
- `ExecServerRuntimePaths`
- `run_main()` for embedding the websocket server

Callers must pass `ExecServerRuntimePaths` to `run_main()`. The top-level
`agere exec-server` command builds these paths from the `agere` arg0 dispatch
state.

## Example session

Initialize:

```json
{"id":1,"method":"initialize","params":{"clientName":"example-client"}}
{"id":1,"result":{}}
{"method":"initialized","params":{}}
```

Start a process:

```json
{"id":2,"method":"process/start","params":{"processId":"proc-1","argv":["bash","-lc","printf 'ready\\n'; while IFS= read -r line; do printf 'echo:%s\\n' \"$line\"; done"],"cwd":"/tmp","env":{"PATH":"/usr/bin:/bin"},"tty":true,"pipeStdin":false,"arg0":null}}
{"id":2,"result":{"processId":"proc-1"}}
{"method":"process/output","params":{"processId":"proc-1","seq":1,"stream":"stdout","chunk":"cmVhZHkK"}}
```

Write to the process:

```json
{"id":3,"method":"process/write","params":{"processId":"proc-1","chunk":"aGVsbG8K"}}
{"id":3,"result":{"status":"accepted"}}
{"method":"process/output","params":{"processId":"proc-1","seq":2,"stream":"stdout","chunk":"ZWNobzpoZWxsbwo="}}
```

Terminate it:

```json
{"id":4,"method":"process/terminate","params":{"processId":"proc-1"}}
{"id":4,"result":{"running":true}}
{"method":"process/exited","params":{"processId":"proc-1","seq":3,"exitCode":0}}
{"method":"process/closed","params":{"processId":"proc-1"}}
```
