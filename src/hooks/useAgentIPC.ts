import { useEffect, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../stores/useAppStore";
import type {
  AgentDonePayload,
  AgentProgressPayload,
  ErrorPayload,
  LogPayload,
  PermissionRequestPayload,
  SessionCompletePayload,
  StatusChangedPayload,
  SuggestionReadyPayload,
  ToolUpdatePayload,
} from "../types/ipc";

function mapAgentStatusToStage(st: string): "default" | "init" | "thinking" | "working" | "done" | "error" {
  const s = st.toLowerCase();
  if (s === "thinking") return "thinking";
  if (s === "processing") return "working";
  if (s === "completed") return "done";
  if (s === "starting" || s === "running") return "init";
  if (s === "waitingapproval") return "thinking";
  if (s === "error") return "error";
  return "default";
}

function mapIpcStatusToAgentStatus(st: string): import("../types/agent").AgentStatus {
  const s = st.toLowerCase();
  if (s === "thinking") return "thinking";
  if (s === "processing") return "processing";
  if (s === "completed") return "completed";
  if (s === "running") return "running";
  if (s === "starting") return "starting";
  if (s === "waitingapproval") return "waitingApproval";
  if (s === "error" || s === "idle") return s === "error" ? "error" : "idle";
  return "running";
}

function appliesToSession(targetSid: string | undefined, current: string | null): boolean {
  if (!targetSid) return false;
  if (targetSid.startsWith("opencode-")) return true;
  return targetSid === current;
}

export function useAgentIPC(): void {
  const storeRef = useRef(useAppStore.getState());
  const sessionRef = useRef<string | null>(null);
  /** 避免 React StrictMode 等导致同一 `agent://error` 入队两次、日志重复。 */
  const lastErrorRef = useRef({ message: "", at: 0 });

  useEffect(() => {
    const unsub = useAppStore.subscribe((state) => {
      storeRef.current = state;
      sessionRef.current = state.sessionId;
    });
    return unsub;
  }, []);

  useEffect(() => {
    const unsubs: UnlistenFn[] = [];

    const run = async () => {
      unsubs.push(
        await listen<StatusChangedPayload>("agent://status-changed", (e) => {
          const p = e.payload;
          if (!appliesToSession(p?.sessionId, sessionRef.current)) return;
          const store = storeRef.current;
          if (p?.sessionId?.startsWith("opencode-") && !store.sessionId) {
            store.setSessionId(p.sessionId);
          }
          if (p?.sessionId?.startsWith("opencode-")) {
            store.setMode("working");
          }
          store.setUiState("running");
          store.setAgentStatus(mapIpcStatusToAgentStatus(String(p.status)));
          store.setLastStage(mapAgentStatusToStage(p.status));
          if (typeof p.percent === "number") {
            store.setPercent(Math.max(0, Math.min(100, p.percent)));
          }
          const hint = p.toolDescription ?? p.toolName;
          if (hint) store.setBubble(String(hint));
        }),
      );

      unsubs.push(
        await listen<LogPayload>("agent://log", (e) => {
          const p = e.payload;
          if (!appliesToSession(p?.sessionId, sessionRef.current)) return;
          storeRef.current.addLogEntry({
            timestamp: Date.now(),
            level: (p.level as "info" | "warn" | "error") ?? "info",
            message: p.message,
          });
        }),
      );

      unsubs.push(
        await listen<ToolUpdatePayload>("agent://tool-update", (e) => {
          const p = e.payload;
          if (!appliesToSession(p?.sessionId, sessionRef.current)) return;
          const id = `${p.sessionId}-${p.tool}-${Date.now()}`;
          storeRef.current.appendTodo({
            id,
            content: p.description ? `${p.tool}: ${p.description}` : p.tool,
            status: "in_progress",
          });
        }),
      );

      unsubs.push(
        await listen<PermissionRequestPayload>("agent://permission-request", (e) => {
          const p = e.payload;
          if (!appliesToSession(p?.sessionId, sessionRef.current)) return;
          if (!p?.toolUseId) return;
          if (p.toolName === "AskUserQuestion") return;
          storeRef.current.setPendingPermission({
            sessionId: p.sessionId,
            toolName: p.toolName,
            toolDescription: p.toolDescription,
            toolUseId: p.toolUseId,
            rawInput: p.rawInput,
          });
        }),
      );

      unsubs.push(
        await listen<SessionCompletePayload>("agent://session-complete", (e) => {
          const p = e.payload;
          if (!appliesToSession(p?.sessionId, sessionRef.current)) return;
          const store = storeRef.current;
          store.setUiState("done");
          store.setPercent(100);
          store.setLastStage("done");
          store.setAgentStatus("completed");
          store.setMode(p.mode ?? "complete");
          store.setResultZh(p.resultZh ?? "");
          store.setSummaryTranslation(p.summaryTranslation ?? "");
          store.setEmotionText(p.emotion ?? "");
          store.setSuggestionOptions(p.suggestionOptions ?? []);
          store.setBubble(p.emotion || "任务完成！");
          store.setPendingPermission(null);
          store.addLogEntry({
            timestamp: Date.now(),
            level: "info",
            message: `[session-complete] ${(p.summaryTranslation ?? "").slice(0, 320)}`,
          });
        }),
      );

      unsubs.push(
        await listen<ErrorPayload>("agent://error", (e) => {
          const p = e.payload;
          if (p?.sessionId && !appliesToSession(p.sessionId, sessionRef.current)) return;
          const store = storeRef.current;
          const msg = p?.message ?? String(e.payload ?? "未知错误");
          const now = Date.now();
          if (
            msg === lastErrorRef.current.message &&
            now - lastErrorRef.current.at < 900
          ) {
            return;
          }
          lastErrorRef.current = { message: msg, at: now };
          store.setUiState("error");
          store.setLastStage("error");
          store.setAgentStatus("error");
          store.setBubble(msg);
          store.setPendingPermission(null);
          store.addLogEntry({
            timestamp: Date.now(),
            level: "error",
            message: `[agent://error] ${msg}`,
          });
        }),
      );

      unsubs.push(
        await listen<AgentProgressPayload>("agent-progress", (e) => {
          const p = e.payload;
          if (p?.sessionId && !appliesToSession(p.sessionId, sessionRef.current)) return;
          const store = storeRef.current;
          store.setUiState("running");
          if (p?.sessionId?.startsWith("opencode-") && !store.sessionId) {
            store.setSessionId(p.sessionId);
          }
          if (p?.stage) store.setLastStage(p.stage as "init" | "thinking" | "working" | "done" | "error");
          if (typeof p?.percent === "number") {
            store.setPercent(Math.max(0, Math.min(100, p.percent)));
          }
          if (p?.message) store.setBubble(p.message);
          if (p?.rawLine) {
            store.addLogEntry({
              timestamp: Date.now(),
              level: "info",
              message: p.rawLine,
            });
          }
        }),
      );

      unsubs.push(
        await listen<AgentDonePayload>("agent-done", (e) => {
          const p = e.payload;
          if (p?.sessionId && !appliesToSession(p.sessionId, sessionRef.current)) return;
          const zh = p?.resultZh ?? "";
          if (zh) storeRef.current.setResultZh(zh);
        }),
      );

      unsubs.push(
        await listen<SuggestionReadyPayload>("suggestion-ready", (e) => {
          const p = e.payload;
          if (p?.sessionId && !appliesToSession(p.sessionId, sessionRef.current)) return;
          const opts = p?.options ?? [];
          if (opts.length > 0) {
            const store = storeRef.current;
            store.setSuggestionOptions(opts);
            store.setUiState("suggesting");
            store.setLastStage("suggest");
          }
        }),
      );
    };

    run();

    return () => {
      unsubs.forEach((u) => {
        try {
          u();
        } catch {
          /* noop */
        }
      });
    };
  }, []);
}

export async function respondPermissionInvoke(
  sessionId: string,
  toolUseId: string,
  decision: "allow" | "deny",
): Promise<void> {
  await invoke("respond_permission", {
    sessionId,
    toolUseId,
    decision,
  });
}
