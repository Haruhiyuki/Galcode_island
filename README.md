# Galcode Island

桌面端宠物 Agent —— 让"凉宫春日"驱动 Claude Code / OpenCode / Codex 三种 CLI 替你写代码。

基于 Tauri 2 + React 19 + Rust。后端按各 CLI 原生协议直连：Claude 走 stream-json stdin/stdout、OpenCode 走 HTTP + SSE、Codex 走 JSON-RPC app-server。

## 快速开始（开发模式）

```bash
# 一次性
npm install

# 三个 CLI 至少装一个并登录（dev 模式从 PATH 找 binary）
npm i -g opencode-ai @openai/codex @anthropic-ai/claude-code
claude auth login   # 以及 codex login / opencode auth login（按需）

# 起 Tauri 桌面应用（自动起 Vite dev server）
npm run dev
```

如果 cargo 报 `requires rustc 1.88` 错，说明 PATH 上的 cargo 是 Homebrew 的旧版（`rust-toolchain.toml` 只对 rustup 的 cargo shim 生效）。

`package.json` 里的 `dev` / `build` / `check:rust` 等命令已经预置了 `PATH="$HOME/.cargo/bin:$PATH"`，正常 `npm run dev` 不需要额外操作。如果你直接调 `cargo` 也想稳，建议在 `~/.zshrc` 加：

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

## 常用命令

| 命令 | 作用 |
|---|---|
| `npm run dev` | 起 Tauri 桌面应用（含 Vite dev server） |
| `npm run dev:web` | 仅起前端（浏览器调试 UI） |
| `npm run build` | 完整打包桌面应用（自动跑 `prepare:runtime`） |
| `npm run prepare:runtime` | 拉三个 CLI prebuilt binary 到 `src-tauri/resources/runtime/<platform>-<arch>/` |
| `npm run typecheck` | TypeScript 类型检查 |
| `npm run check:rust` | Rust 编译检查 |
| `npm run fmt:rust` | rustfmt |
| `npm run lint:rust` | clippy（warnings as errors） |
| `npm run clean` | 清掉 dist + cargo target + bundled runtime |

## Bundle / 打包发布

`npm run build` 会自动跑 `prepare:runtime`：用 `npm install` 临时拉三个 CLI 的 npm 包，从 node_modules 挑出当前平台的 prebuilt binary 复制到 `src-tauri/resources/runtime/<platform>-<arch>/<kind>/<binary>`，然后 Tauri 把这些文件 bundle 进 .app/.dmg/.msi/.AppImage。

**已知体积**（macOS arm64）：
- claude: ~206 MB
- codex:  ~190 MB
- opencode: ~123 MB
- 总计    ~520 MB → .dmg 约 250 MB（压缩后）

可以用以下开关控制 bundle 内容：
```bash
node scripts/prepare-runtime.mjs --skip-claude    # Anthropic 专有许可证敏感时
node scripts/prepare-runtime.mjs --skip-opencode  # 让用户自己装 opencode
node scripts/prepare-runtime.mjs --skip-codex
```

被 skip 的 CLI 不在 bundle 里，运行时会回退到从系统 `PATH` 找。`src-tauri/resources/runtime/` 已经在 `.gitignore` 里——不入库，每次 build / CI 本地拉。

## 配置

复制 `.env.example` 为 `.env`，至少填一个 LLM API Key —— 用来做"中文 prompt → 英文 → CLI"的输入翻译和"CLI 输出 → 中文 + 凉宫春日台词"的总结。无 LLM 也能跑，但 CLI 输出会原样显示，没有中文化和宠物气泡。

## 文档

- [`docs/architecture.md`](docs/architecture.md) — 系统架构 + 三种 CLI 接入方式
- [`docs/ipc-protocol.md`](docs/ipc-protocol.md) — Tauri Commands / Events 契约
- [`docs/character-and-prompts.md`](docs/character-and-prompts.md) — 凉宫春日人设 + 三层提示词模板

## 目录结构

```
src/                  React 前端
src-tauri/src/
  agent/              三个 CLI 接入 (claude / opencode / codex) + manager
  ipc/                Tauri commands + events 桥
  llm/                输入/输出翻译 + 凉宫春日总结
  session/            会话快照状态机
src-tauri/resources/
  runtime/<key>/      bundle 时拉到这里（被 .gitignore，build 自动准备）
public/pet/           宠物动画资源
scripts/
  prepare-runtime.mjs 拉 CLI prebuilt binary 的脚本
```
