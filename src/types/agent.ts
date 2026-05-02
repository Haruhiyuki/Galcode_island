export type AgentType =
  | "claude-code"
  | "opencode"
  | "codex"
  | "gemini"
  | "cursor";

export type AgentStatus = "idle" | "running" | "waiting" | "error";
export type AgentTab = "claude-code" | "opencode";

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
