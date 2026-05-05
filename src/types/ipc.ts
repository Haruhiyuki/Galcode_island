// 后端 emit 的 agent 事件 payload。
//
// 多 tab 路由：每个事件都带 runId（后端 events.rs StatusChangedPayload 等
// 加了 #[serde(skip_serializing_if = "Option::is_none")]，所以前端可能
// 拿不到 — 兜底走 sessionId 反查 tab）。

export interface StatusChangedPayload {
  sessionId: string;
  runId?: string;
  status: string;
  percent?: number;
  toolName?: string;
  toolDescription?: string;
}

export interface SessionCompletePayload {
  sessionId: string;
  runId?: string;
  mode?: string;
  emotion?: string;
  summaryTranslation?: string;
  resultRaw?: string;
  resultZh?: string;
  suggestionOptions?: string[];
}

export interface ErrorPayload {
  sessionId?: string;
  runId?: string;
  message: string;
  code?: string;
}
