export type AgentType =
  | "claude-code"
  | "opencode"
  | "codex"
  | "gemini"
  | "cursor";

export type AgentStatus =
  | "idle"
  | "starting"
  | "running"
  | "thinking"
  | "processing"
  | "waitingApproval"
  | "completed"
  | "error";

export type AgentTab = "claude-code" | "opencode";

export type UiState =
  | "idle"
  | "running"
  | "done"
  | "error"
  | "suggesting";

export type LastStage =
  | "default"
  | "init"
  | "thinking"
  | "working"
  | "done"
  | "error"
  | "suggest";

export interface TodoItem {
  id: string;
  content: string;
  status: "pending" | "in_progress" | "completed" | "error";
}

export interface LogEntry {
  timestamp: number;
  level: "info" | "warn" | "error";
  message: string;
  toolName?: string;
}
