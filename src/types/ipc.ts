export interface AgentStatusPayload {
  status:
    | "idle"
    | "starting"
    | "running"
    | "thinking"
    | "processing"
    | "waitingApproval"
    | "completed"
    | "error";
}

export interface AgentToolPayload {
  tool: string;
  description?: string;
}

export interface SessionCompletePayload {
  summary: string;
  emotion?: string;
}
