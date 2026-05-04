// CLI 流事件的 block 类型 —— 后端 emit `galcode://cli-output` 的 line 字段是
// JSON 字符串，反序列化后有两种顶层格式：
//   1. { type: "galcode.block", block: {...} }   ← Claude/Codex/OpenCode 通用块
//   2. { type: "opencode.tool|file|status|error", ... }  ← OpenCode 专属
//
// 前端 useCliStream 把两种格式归一成下面的 CliBlock 统一类型，按 id 去重（同一
// id 的后续事件是 update：text 增量、command output delta、todo 状态变化等）。

export interface CliStreamEvent {
  streamId: string;
  backend: string;        // "claude" | "codex" | "opencode"
  channel: string;        // "stdout" | "stderr"
  line: string;            // JSONL / block JSON / 纯文本
  runId: string;
}

export type CliBlockType =
  | "text"
  | "thought"
  | "command"
  | "todo"
  | "confirm"
  | "tool"
  | "file"
  | "diff"
  | "status"
  | "error"
  | "stderr";

export interface CliTodoItem {
  id: string;
  label: string;
  status: string;         // "pending" | "running" | "success" | ...
}

export interface CliBlock {
  id: string;
  type: CliBlockType;
  backend?: string;
  suppressLogLine?: boolean;

  // text / thought / status / error
  content?: string;
  message?: string;
  tone?: string;          // text (file 标记)

  // command
  command?: string;
  output?: string;
  status?: string;        // running / success / error / completed

  // todo
  title?: string;
  items?: CliTodoItem[];

  // confirm (审批)
  interactive?: boolean;
  note?: string;
  approvalId?: string;

  // tool / file / diff
  tool?: string;
  path?: string;
  detail?: string;

  // diff (Claude Edit/MultiEdit/Write)
  diff?: string;          // unified-ish: 行首 +/-/空格

  // stderr (raw)
  channel?: string;       // "stderr"

  // 当用户切换会话时丢弃旧 block 用
  streamId?: string;
}
