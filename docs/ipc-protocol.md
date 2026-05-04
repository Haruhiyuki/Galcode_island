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

`AgentType = "claude-code" | "opencode" | "codex" | "demo"`

立即返回 `{ sessionId, status: "running" }`，真正的执行在后台异步。结果通过 `agent://session-complete` 事件返回。

### `launch_agent`

`start_agent` 的显式版（必须传 `agent` 参数）。语义相同。

### `stop_agent`

中断当前会话。`sessionId` 省略时停止 `active_demo_session`（即"上一次启动的会话"，承载所有 backend）。

```ts
invoke("stop_agent", { sessionId?: string })
```

⚠️ 当前实现仅更新 SessionSnapshot 状态 + emit interrupted 事件，**不真正中断 backend turn**。后端注释里说"app 退出时统一清理"。要真正中断需要给每个 backend 加 abort 接口（OpenCode `POST /session/<sid>/abort`、Codex `interrupt` JSON-RPC、Claude 关 stdin）。

### `respond_permission`

回复审批请求。第一版全部 auto-approve，本命令是 stub（实际不会调到）。

```ts
invoke<{ ok: boolean }>("respond_permission", {
  sessionId: string,
  toolUseId: string,
  decision: "approve" | "deny" | "session" | …,
})
```

### `get_session_logs`

读取一次会话的后端 log buffer（最多 500 行环形）。

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
  nickname: string,           // 凉宫春日的"部员称呼"
  systemPrompt: string,        // userWhisper（人设补充指令）
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

驱动 PetCharacter 表情动画 + 状态条。

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
  code: string,         // CLAUDE_TURN_FAILED / OPENCODE_START_FAILED / 等
}
```

### `agent://log`

后端调试日志（按行）。

```ts
{
  sessionId: string,
  level: "info" | "debug" | "warn" | "error",
  message: string,
  timestamp: string,         // RFC3339
}
```

### `agent://tool-update` / `agent://tool-result`

工具调用进度（来自 SideEffect → reduce_event）。当前 hook 路径主要是 demo agent 用，新 backend 没产生这些事件（用 `galcode://cli-output` 替代，见下）。

### `agent://permission-request`

审批请求。第一版全 auto-approve，前端可以暂不监听。

### `agent://cleanup`

后台清理循环每 2 分钟扫一次完成会话（>30 分钟无更新），把它们移除。

```ts
{ removedSessionIds: string[] }
```

### `galcode://cli-output`

CLI 流式输出（block 化）。当前后端 emit 但**前端尚未监听**——属于待补的链路（详见 `architecture.md` 中的下一步）。

```ts
{
  streamId: string,
  backend: "claude" | "opencode" | "codex",
  channel: "stdout" | "stderr",
  line: string,                        // 原始 JSONL 行 / 块 JSON / 纯文本
  runId: string,                        // 多 tab 路由（当前未填，留空字符串）
}
```

block 类型枚举（在 `line` 解析为 JSON 后的 `block.type`）：

- `text` — Agent 中间消息
- `command` — 工具执行的 shell 命令 + 输出
- `thought` — Agent 思考过程
- `todo` — TodoList
- `confirm` — 审批请求（auto-approve 模式下也会经过）

### Legacy 兼容事件（即将废弃）

历史包袱，新前端代码不应该依赖：

- `agent-progress` — demo 路径的进度事件
- `agent-done` — `session-complete` 的旧版本
- `suggestion-ready` — `session-complete.suggestionOptions` 的旧版本
- `agent-error` — `agent://error` 的旧版本

---

## 3. SideEffect Reducer 模型

`session/state.rs::reduce_event(snapshot, hook_event) -> Vec<SideEffect>`

把 CLI 输出（HookEvent）归一成对 SessionSnapshot 的 mutation + 一组待 emit 的副作用。`ipc/events.rs::apply_side_effects` 负责把 SideEffect 转成对应的 `agent://*` 事件。

新 backend（claude/opencode/codex）当前**不走** reduce_event 路径，直接由 `agent::manager` emit `agent://session-complete`。reduce 路径只剩 demo 还在用。要让新 backend 也产出结构化中间状态（工具调用、思考过程），需要在每个 backend 的 stdout 解析处生成 HookEvent 并喂给 reduce_event——目前是直接 emit `galcode://cli-output` 让前端自己解析。

---

## 4. 已知契约债

| 问题 | 影响 | 处理方向 |
|---|---|---|
| 同时存在 `agent://*` 和 legacy `agent-*` 两套 | 前端要监听双份 | 收敛到 `agent://*`，删 legacy emit |
| `galcode://cli-output` 后端发了前端没听 | 中间过程不可见 | 前端 useAgentIPC 加 listener + ResultCard 渲染 block 类型 |
| `stop_agent` 不真中断 turn | 用户体验 bug | 给每个 backend 加 abort 接口 |
| `respond_permission` 是 stub | 无审批 UI | 第一版按 auto-approve 不影响，未来要做时把真实 reply 接上 |
| `runId` 在 `cli-output` 字段里固定为空字符串 | 多 tab 路由不可用 | 后端在创建 stream 时把 stream_id → run_id 映射，emit 时回填 |
| `last_session_per_context` 把 claude.session_id / codex.thread_id / opencode.session_id 都存为 `String` | 语义混淆 | 改成 enum 或每个 backend 自己存到 RuntimeState |
