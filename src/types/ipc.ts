export interface StatusChangedPayload {
  sessionId: string;
  status: string;
  percent?: number;
  toolName?: string;
  toolDescription?: string;
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

