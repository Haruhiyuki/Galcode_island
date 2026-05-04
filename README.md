# Galcode Island

桌面端宠物 Agent —— 让"凉宫春日"驱动 Claude Code / OpenCode / Codex 三种 CLI 替你写代码。

基于 Tauri 2 + React 19 + Rust。后端按各 CLI 原生协议直连：Claude 走 stream-json stdin/stdout、OpenCode 走 HTTP + SSE、Codex 走 JSON-RPC app-server。

## 快速开始

```bash
# 一次性
npm install

# 三个 CLI 至少装一个，并完成登录
brew install claude        # 或 npm i -g @anthropic-ai/claude-code
brew install opencode-ai/tap/opencode
npm install -g @openai/codex
claude auth login           # codex login / opencode auth login

# 开发：一键起 Vite + Tauri
npm run dev
```

如果 cargo 报 `requires rustc 1.88` 错，说明 PATH 上的 cargo 是 Homebrew 的旧版。`rust-toolchain.toml` 已锁 stable，确保用 rustup（`~/.cargo/bin/cargo`）。

## 常用命令

| 命令 | 作用 |
|---|---|
| `npm run dev` | 起 Tauri 桌面应用（含 Vite dev server） |
| `npm run dev:web` | 仅起前端（浏览器调试 UI） |
| `npm run build` | 完整打包桌面应用 |
| `npm run typecheck` | TypeScript 类型检查 |
| `npm run check:rust` | Rust 编译检查 |
| `npm run fmt:rust` | rustfmt 格式化 |
| `npm run lint:rust` | clippy（warnings as errors） |
| `npm run clean` | 清掉 dist + cargo target |

## 配置

复制 `.env.example` 为 `.env`，至少填一个 LLM API Key —— 用来做"中文 prompt → 英文 → CLI"的输入翻译和"CLI 输出 → 中文 + 凉宫春日台词"的总结。无 LLM 也能跑，只是 CLI 输出会原样显示，没有中文化和宠物气泡。

## 文档

- [`docs/architecture.md`](docs/architecture.md) — 系统架构 + 三种 CLI 接入方式
- [`docs/ipc-protocol.md`](docs/ipc-protocol.md) — Tauri Commands / Events 契约
- [`docs/character-and-prompts.md`](docs/character-and-prompts.md) — 凉宫春日人设 + 三层提示词模板

## 目录结构

```
src/                  React 前端
src-tauri/src/
  agent/              三个 CLI 接入 (claude / opencode / codex) + manager
  hook/               CLI 输出事件解析
  ipc/                Tauri commands + events 桥
  llm/                输入/输出翻译 + 凉宫春日总结
  session/            会话快照状态机
public/pet/           宠物动画资源
scripts/              demo agent (python，纯烟测用)
```
