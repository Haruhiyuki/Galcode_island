export interface StatusChangedPayload {
  sessionId: string;
  status: string;
  percent?: number;
  toolName?: string;
  toolDescription?: string;
}

export interface ToolUpdatePayload {
  sessionId: string;
  tool: string;
  description?: string;
}

export interface PermissionRequestPayload {
  sessionId: string;
  toolName: string;
  toolArgs: string;
}

export interface LogPayload {
  sessionId: string;
  level: string;
  message: string;
}

export interface SessionCompletePayload {
  sessionId: string;
  mode?: string;
  emotion?: string;
  summaryTranslation?: string;
  resultRaw?: string;
  resultZh?: string;
  suggestionOptions?: string[];
}

export interface ErrorPayload {
  sessionId?: string;
  message: string;
}

export interface AgentProgressPayload {
  sessionId?: string;
  stage?: string;
  percent?: number;
  message?: string;
  rawLine?: string;
}

export interface AgentDonePayload {
  sessionId?: string;
  resultZh?: string;
}

export interface SuggestionReadyPayload {
  sessionId?: string;
  options?: string[];
}
