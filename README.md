# galcode_island — 凉宫春日 AI 桌宠（黑客松原型）

Tauri v2 + React：模块化 Rust 后端（`agent` / `hook` / `session` / `ipc` / `llm`）、系统托盘、可选文件夹选择、Demo Agent（Python JSONL）、`agent://*` 事件流；Pixi 表情舞台（可选 Live2D）；可选 OpenAI 兼容 LLM（翻译、总结、情绪反馈、建议）。

## 环境

- Node.js LTS、Rust stable（Windows 需 VS C++ Build Tools / WebView2）
- Python 3.10+ 且在 `PATH` 中（`python -u` 跑 `scripts/demo_agent.py`）

## 开发

```bash
npm install
npm run tauri dev
```

仅前端（无桌面窗口能力）：

```bash
npm run dev
```

打包：

```bash
npm run tauri build
```

## 环境变量（LLM，可选）

未配置 `LLM_API_KEY` 时：跳过云端翻译/总结中的模型调用；Demo 仍运行；界面使用占位总结/建议。

| 变量 | 说明 |
|------|------|
| `LLM_API_KEY` | OpenAI 兼容 API Key |
| `LLM_BASE_URL` | 默认 `https://api.openai.com/v1` |
| `LLM_MODEL` | 默认 `gpt-4o-mini` |
| `PYTHON` | 解释器，默认 Windows 为 `python` |
| `AGENT_SCRIPT` | 覆盖 Agent 脚本路径 |
| `GALCODE_HOOK_LOG_PATH` | （可选）供 `notify` 监视的 hook 日志路径占位 |

可参考 [.env.example](.env.example)。

## IPC（前后端）

### 黑客松计划核心约定（兼容）

| invoke | 参数 | 说明 |
|--------|------|------|
| `start_agent` | `{ userInputZh, cwd? }` | 中文需求 → 可选 LLM 中译英 → `python -u` Demo；返回 `{ sessionId, status }` |
| `stop_agent` | `{ sessionId? }` | 省略 `sessionId` 时停止当前活动 Demo 会话 |
| `translate_only` | `{ textZh }` | 调试翻译（需 Key） |

| emit | payload 要点 |
|------|----------------|
| `agent-progress` | `stage`, `message`, `percent`, `sessionId`；无法解析的 stdout 行带 `rawLine` |
| `agent-done` | `resultRaw`, `resultZh`, `sessionId` |
| `agent-error` | `message`, `sessionId` |
| `suggestion-ready` | `textZh`, `sessionId` |

扩展命令与 `agent://*` 事件见下表。

### Commands（invoke）

| 命令 | 参数 | 说明 |
|------|------|------|
| `select_project_folder` | `()` | 弹出目录选择，返回路径或 `null` |
| `start_agent` | `{ userInputZh, cwd? }` | 推荐入口（同上） |
| `launch_agent` | `{ agent, cwd, taskZh? }` | 当前仅 `agent: "demo"`；`taskZh` 必填 |
| `stop_agent` | `{ sessionId? }` | 终止子进程；省略参数则停当前活动会话 |
| `respond_permission` | `{ sessionId, toolUseId, decision }` | 占位 Stub，记录日志 |
| `get_session_logs` | `{ sessionId }` | 返回该会话累积日志行 |
| `translate_only` | `{ textZh }` | 需 `LLM_API_KEY` |
| `set_click_through` | `{ enabled }` | Windows：扩展窗口样式（演示用） |

`launch_agent` 返回：`{ sessionId, status }`（camelCase）。

### Events（listen）

主通道前缀 **`agent://`**：

- `agent://status-changed`：`sessionId`, `status`, `toolName`, `toolDescription`, `percent`
- `agent://log`：`sessionId`, `level`, `message`, `timestamp`
- `agent://tool-update` / `agent://tool-result` / `agent://permission-request`：按载荷扩展
- `agent://session-complete`：`sessionId`, `summary`, `emotion`, `resultRaw`, `resultZh`, `suggestionZh`
- `agent://error`：`sessionId`, `message`, `code`
- `agent://cleanup`：空闲会话清理通知（低频）

兼容旧前端：`agent-progress`、`agent-done`（含 `sessionId`）、`agent-error`、`suggestion-ready` 仍会发送。

### Hook JSON（Demo）

进度行含 `stage` / `message` / `percent`；结束行：`{"type":"result","output_en":"..."}`。

## Live2D（可选）

模型放入 `public/models/haruhi/haruhi.model3.json` 并遵守 Cubism Web SDK 许可；无模型时使用 Pixi emoji 占位。

## 参考

- [Tauri v2](https://v2.tauri.app/)
- [CodeIsland（事件面板思路）](https://github.com/wxtsky/CodeIsland)
