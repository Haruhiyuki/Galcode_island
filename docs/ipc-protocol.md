# IPC 协议

前端与 Rust 后端通过 Tauri 的 `invoke()` 和 `listen()` 通信。本文档定义当前真实的契约（**以代码为准**，过时随时纠正）。

## 1. Commands（前端 → 后端）

### `start_agent`

启动一次 agent turn。中文任务 → 翻译 → 路由到对应 backend → 异步执行。

```ts
invoke<{ sessionId: string; status: AgentStatus }>("start_agent", {
  userInputZh: string,
  cwd?: string,           // 默认 "."
  agent?: AgentType,      // 默认 "claude-code"
})
```

`AgentType = "claude-code" | "opencode" | "codex"`

立即返回 `{ sessionId, status: "running" }`，真正的执行在后台异步。结果通过 `agent://session-complete` 事件返回。

### `stop_agent`

中断当前会话。`sessionId` 省略时停止 `active_session`（即"上一次启动的会话"）。

```ts
invoke("stop_agent", { sessionId?: string })
```

⚠️ 当前实现仅更新 SessionSnapshot 状态 + emit `agent://status-changed` (idle)，**不真正中断 backend turn**。要真正中断需要给每个 backend 加 abort 接口（OpenCode `POST /session/<sid>/abort`、Codex `interrupt` JSON-RPC、Claude 关 stdin）。

### `respond_permission`

回复审批请求。第一版全部 auto-approve，本命令是 stub（实际不会被调到）。

```ts
invoke<{ ok: boolean }>("respond_permission", {
  sessionId: string,
  toolUseId: string,
  decision: "approve" | "deny" | "session" | …,
})
```

### `get_session_logs`

读取一次会话的后端 log buffer（最多 500 行环形）。当前 backend 还没把流式日志写入 `AgentSession.logs`，主要保留作未来调试面板的入口。

```ts
invoke<string[]>("get_session_logs", { sessionId: string })
```

### `translate_only`

纯翻译工具，不启动 agent。

```ts
invoke<string>("translate_only", { textZh: string })
```

### `update_llm_settings`

更新全局 LLM 配置（持久化到内存单例）。

```ts
invoke("update_llm_settings", {
  baseUrl: string,
  apiKey: string,
  nickname: string,           // 凉宫春日的"部员称呼" → {{userName}}
  systemPrompt: string,        // 部员悄悄话（人设补充） → {{userWhisper}}
})
```

### `select_project_folder`

弹系统目录选择器，返回选中路径。

```ts
invoke<string | null>("select_project_folder")
```

### `set_click_through`

切换主窗口的鼠标穿透（桌面宠物模式）。

```ts
invoke("set_click_through", { enabled: boolean })
```

---

## 2. Events（后端 → 前端）

### `agent://status-changed`

```ts
{
  sessionId: string,
  status: "idle" | "starting" | "running" | "thinking" | "processing"
        | "waitingApproval" | "completed" | "error",
  toolName: string | null,
  toolDescription: string | null,
  percent: number | null,
}
```

驱动 PetCharacter 表情动画 + 状态条。当前 backend 在 `start_agent` / `stop_agent` 时各 emit 一次。

### `agent://session-complete`

最终结果（含 LLM 总结）：

```ts
{
  sessionId: string,
  mode: "normal" | "suggestion" | "error" | "complete" | null,
  emotion: string | null,                 // 凉宫春日台词（emotion_speech）
  summaryTranslation: string | null,      // 客观结果中文摘要
  resultRaw: string | null,                // CLI 英文原文
  resultZh: string | null,                 // 翻译后的中文
  suggestionOptions: string[] | null,      // 最多 2 个快捷建议按钮
}
```

驱动 ResultCard 渲染 + PetCharacter 模式切换。

### `agent://error`

非致命错误（fatal 错误也会同时 emit `session-complete` mode=error）。

```ts
{
  sessionId: string,
  message: string,
  code: string,         // CLAUDE_TURN_FAILED / OPENCODE_START_FAILED / CODEX_TURN_FAILED / 等
}
```

### `agent://cleanup`

后台清理循环每 2 分钟扫一次完成会话（>30 分钟无更新），把它们移除。

```ts
{ removedSessionIds: string[] }
```

### `galcode://cli-output`

CLI 流式输出（block 化）。当前后端 emit 但**前端尚未监听**——属于待补的链路（详见 §4 已知契约债）。

```ts
{
  streamId: string,
  backend: "claude" | "opencode" | "codex",
  channel: "stdout" | "stderr",
  line: string,                        // 原始 JSONL 行 / 块 JSON / 纯文本
  runId: string,                        // 多 tab 路由（当前未填，留空字符串）
}
```

`line` 是 JSON 时，常见 `block.type`：

- `text` — Agent 中间消息
- `command` — 工具执行的 shell 命令 + 输出
- `thought` — Agent 思考过程
- `todo` — TodoList
- `confirm` — 审批请求块（auto-approve 模式下也会经过）

---

## 3. SessionSnapshot 与 Manager 数据模型

`AgentSession`（`agent/manager.rs`）：

```rust
pub struct AgentSession {
    pub snapshot: Arc<Mutex<SessionSnapshot>>,
    pub logs: Arc<Mutex<Vec<String>>>,
    pub created_at: Instant,
    pub stream_id: String,    // 用于 `galcode://cli-output` 路由
}
```

`AgentManager`：

```rust
pub struct AgentManager {
    pub sessions: HashMap<String, AgentSession>,
    pending_permission: HashMap<(String, String), ()>,  // 审批 stub
    pub active_session: Option<String>,                  // 兼容无参 stop_agent
    pub last_session_per_context: HashMap<(String, String), String>,
    //  └── (agent_type, cwd) → 上次的 session_id / thread_id，下次自动 resume
}
```

`AgentStatus`（`session/state.rs`，唯一保留的 enum）：

```rust
enum AgentStatus {
    Idle, Starting, Running, Thinking, Processing,
    WaitingApproval, Completed, Error,
}
```

---

## 4. 已知契约债

| 问题 | 影响 | 处理方向 |
|---|---|---|
| `galcode://cli-output` 后端 emit 但前端没听 | 中间过程不可见 | 前端 useAgentIPC 加 listener + ResultCard 渲染 block 类型（text / command / thought / todo / confirm） |
| `stop_agent` 不真中断 turn | 用户点停止后 backend 仍在跑 | 给每个 backend 加 abort 接口 |
| `respond_permission` 是 stub | 无审批 UI | 第一版按 auto-approve 不影响；做审批 UI 时把后端真实 reply 接上（OpenCode `opencode_reply_permission` / Codex `write_codex_app_server_response`） |
| `runId` 在 `cli-output` 字段里固定为空字符串 | 多 tab 路由不可用 | 后端在创建 stream 时把 stream_id → run_id 映射，emit 时回填 |
| `last_session_per_context` 把 claude.session_id / codex.thread_id / opencode.session_id 都存为 `String` | 语义混淆 | 改成 enum 或每个 backend 自己存到 RuntimeState |
| `get_session_logs` 实际拿到空 buffer | 调试面板不可用 | backend stdout/stderr 解析时同时 push_log 到 `AgentSession.logs`（或彻底删掉这个命令） |
