# 架构

## 一页图

```
┌─────────────────────────────────────────────────────────────┐
│  React Frontend (src/)                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ PetCharacter │  │ ChatBubble / │  │  AgentSelector   │   │
│  │ (live2d)     │  │ ResultCard   │  │  / Settings      │   │
│  └──────────────┘  └──────────────┘  └──────────────────┘   │
│         ▲                  ▲                   ▲            │
│         └──── Zustand store (useAppStore) ─────┘            │
│                       ▲          ▲                          │
│              listen() │          │ invoke()                 │
└───────────────────────┼──────────┼──────────────────────────┘
                        │          │
                        │   Tauri IPC                         │
                        │          │
┌───────────────────────┼──────────┼──────────────────────────┐
│  Rust Backend (src-tauri/src/)                              │
│  ┌────────────────────┴──────────┴────────────────────┐     │
│  │  ipc::commands  (start_agent / stop_agent / …)     │     │
│  │  ipc::events    (apply_side_effects → emit)        │     │
│  └─────────────────────────┬──────────────────────────┘     │
│                            ▼                                │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  agent::manager  ←── LLM 翻译/总结管线                │   │
│  │   - launch_claude_agent / launch_opencode_agent /    │   │
│  │     launch_codex_agent / launch_demo_agent           │   │
│  │   - finalize_session (translate + summary + emit)    │   │
│  │   - last_session_per_context (会话续接)               │   │
│  └────────┬───────────┬──────────┬──────────┬───────────┘   │
│           ▼           ▼          ▼          ▼               │
│      claude.rs   opencode.rs  codex.rs   demo (launcher.rs) │
│      stream-json  HTTP+SSE    JSON-RPC   python script      │
│           │           │          │                          │
│           ▼           ▼          ▼                          │
│      sysutils.rs  (binary 解析 / 进程管理 / 代理 / …)         │
│      runtime.rs   (RuntimeState: per-tab HashMap)           │
└─────────────────────────┬───────────────────────────────────┘
                          ▼
              ┌──────────────────────┐
              │  External CLI        │
              │  claude / opencode / │
              │  codex / python      │
              └──────────────────────┘
```

## 三种 CLI 接入方式

| Backend | 协议 | spawn 命令 | 客户端复用 | 会话续接 |
|---|---|---|---|---|
| **Claude Code** | stream-json (stdin/stdout JSONL) | `claude -p --input-format stream-json --output-format stream-json --verbose --replay-user-messages --include-partial-messages --permission-mode acceptEdits` | 按 (cwd, model, effort, proxy, session) 复用长进程 | `--resume <session_id>` |
| **OpenCode** | HTTP + SSE | `opencode serve --hostname 127.0.0.1 --port <P>` (per-tab 端口从 4096 起线性分配) | per-tab 独立子进程 | 同一 cwd 复用 `session_id` |
| **Codex** | JSON-RPC (stdin/stdout) | `codex app-server` (全局共享单实例) | `CODEX_SHARED_KEY` 全局共享，避免多进程抢 `~/.codex/auth.json` | `thread/resume` 用 `thread_id` |
| **Demo** | stdout 行 | `python -u scripts/demo_agent.py --task <text>` | 不复用 | 不支持 |

详见各模块顶部注释（`agent/{claude,opencode,codex}.rs`）。

## 会话生命周期

每个 `start_agent` 调用：

1. **创建 SessionSnapshot** —— `agent::manager::AgentSession::new()`，分配 `session_id` (UUID) 和 `stream_id`
2. **emit `agent://status-changed`** (Running) —— 前端切换到工作状态
3. **后台 spawn 一个 task**（`tokio::spawn` 或 `std::thread::spawn`），其内部：
   - LLM 翻译：中文 prompt → 英文（如配置了 LLM Key）
   - 调对应 backend 的 `run_*_turn()`，阻塞等结果
   - LLM 翻译：英文输出 → 中文
   - LLM 总结：调 `generate_agent_summary` 拿到 `mode/emotion_speech/summary_translation/next_options`
   - emit `agent://session-complete` 含完整 payload
4. **会话记忆**：成功后把 backend 返回的 session_id / thread_id 存到 `last_session_per_context[(agent_type, cwd)]`，下次同 (agent_type, cwd) 提交时自动 resume

## RuntimeState 多 tab 路由

```rust
RuntimeState {
    opencode: Mutex<HashMap<run_id, OpencodeState>>,    // per-tab serve 子进程
    codex:    Mutex<HashMap<run_id, CodexAppServerState>>, // 实际只有 CODEX_SHARED_KEY 一项
    claude:   Mutex<HashMap<run_id, ClaudeStreamState>>, // per-tab stream client
    port_pool: Mutex<HashSet<u16>>,                      // OpenCode 端口分配
}
```

当前前端只有单会话，所有调用 `run_id = "default"`。后端已为多 tab 留好接口，前端扩 tab 时不需改后端。

## LLM 翻译/总结管线

LLM 调用是**独立的三次**（详见 `docs/character-and-prompts.md`）：

```
用户中文 prompt
    │
    ├── translate_zh_to_en ──► 英文 prompt ──► CLI backend ──► 英文输出
    │                                                              │
    │                                                              ▼
    │                                                    translate_en_to_zh
    │                                                              │
    │                                                              ▼
    └─────────────► generate_agent_summary ◄───── 中文输出 ─────────┘
                            │
                            ▼
            { mode, emotion_speech, summary_translation, next_options }
                            │
                            ▼
                  agent://session-complete
```

LLM 客户端在 `llm/client.rs`，单例 `OnceLock<Mutex<GlobalLlmSettings>>`。无 LLM 配置时跳过翻译，直接把英文原文当结果发出。

## Auto-approve 策略

第一版**全部审批自动放行**，不阻塞用户：

- **Claude**: spawn 时 `--permission-mode acceptEdits`，CLI 自己处理
- **Codex**: `codex_should_auto_approve_request` 收到 `item/fileChange/applyPatchApproval` 时校验 `grant_root` 在 `working_dir` 内才放行；命令执行 / 额外权限第一版全自动 approve
- **OpenCode**: `spawn_opencode_auto_approve_poller` 每 900ms 拉取待审批列表，新出现的一律 reply `always`

未来要做交互审批：把这些自动放行的代码路径切成 emit 让前端弹卡片，再走 `respond_permission` 命令回写。

## 进程管理

参见 `agent/sysutils.rs`：

- **启动孤儿回收**: `cleanup_stale_runtime_orphans` 在 app 启动时扫描 `ppid==1` 且 cmdline 含 opencode/codex/claude binary 路径的进程，全部 SIGTERM→SIGKILL
- **子进程树清理**: `kill_child_descendants` 用 `pgrep -P` 递归收集子孙进程，先广播 SIGTERM 给 120ms 缓冲再 SIGKILL（Windows 用 `taskkill /T /F` 一步到位）
- **OpenCode 端口冲突**: 启动前 `lsof` / `netstat` 探活，cmdline 含 `opencode` 才 kill 释放
- **CLI 版本检测缓存**: 5 分钟 TTL，避免 Windows Defender 反复扫描 `--version` 导致的 1.5s 启动延迟
