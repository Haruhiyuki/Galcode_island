// 三个 CLI backend 的状态/审核结果类型，与 Rust 端 codex_status / claude_status /
// opencode_status / codex_verify / claude_verify 命令的返回值对齐。

export interface ClaudeStatus {
  installed: boolean;
  version?: string;
  binary: string;
  loggedIn: boolean;
  loginStatus: string;
  authMethod?: string;
  defaultModel?: string;
  defaultEffort?: string;
}

export interface CodexStatus {
  installed: boolean;
  version?: string;
  binary: string;
  loggedIn: boolean;
  loginStatus: string;
  authMethod?: string;
  defaultModel?: string;
  defaultReasoningEffort?: string;
}

export interface OpencodeStatus {
  installed: boolean;
  version?: string;
  running: boolean;
  managed: boolean;
  binary: string;
  port: number;
  projectDir: string;
  sessionId?: string;
}

export interface VerifyResult {
  ok: boolean;
  message: string;
}
