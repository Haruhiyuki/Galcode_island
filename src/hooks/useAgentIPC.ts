import { useEffect, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useAppStore } from "../stores/useAppStore";
import type {
  ErrorPayload,
  SessionCompletePayload,
  StatusChangedPayload,
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

export function useAgentIPC(): void {
  const storeRef = useRef(useAppStore.getState());
  const sessionRef = useRef<string | null>(null);

  useEffect(() => {
    const unsub = useAppStore.subscribe((state) => {
      storeRef.current = state;
      sessionRef.current = state.sessionId;
    });
    return unsub;
  }, []);

  useEffect(() => {
    const unsubs: UnlistenFn[] = [];

    const forSession = (sid: string | undefined, fn: () => void) => {
      const current = sessionRef.current;
      if (!current || sid !== current) return;
      fn();
    };

    const run = async () => {
      unsubs.push(
        await listen<StatusChangedPayload>("agent://status-changed", (e) => {
          const p = e.payload;
          forSession(p?.sessionId, () => {
            const store = storeRef.current;
            store.setUiState("running");
            store.setLastStage(mapAgentStatusToStage(p.status));
            if (typeof p.percent === "number") {
              store.setPercent(Math.max(0, Math.min(100, p.percent)));
            }
            const hint = p.toolDescription ?? p.toolName;
            if (hint) store.setBubble(String(hint));
          });
        }),
      );

      unsubs.push(
        await listen<SessionCompletePayload>("agent://session-complete", (e) => {
          const p = e.payload;
          forSession(p?.sessionId, () => {
            const store = storeRef.current;
            store.setUiState("done");
            store.setPercent(100);
            store.setLastStage("done");
            store.setMode(p.mode ?? "complete");
            store.setResultZh(p.resultZh ?? "");
            store.setSummaryTranslation(p.summaryTranslation ?? "");
            store.setEmotionText(p.emotion ?? "");
            store.setSuggestionOptions(p.suggestionOptions ?? []);
            store.setBubble(p.emotion || "任务完成！");
            store.addLogEntry({
              timestamp: Date.now(),
              level: "info",
              message: `[session-complete] ${(p.summaryTranslation ?? "").slice(0, 320)}`,
            });
          });
        }),
      );

      unsubs.push(
        await listen<ErrorPayload>("agent://error", (e) => {
          const p = e.payload;
          forSession(p?.sessionId, () => {
            const store = storeRef.current;
            const msg = p?.message ?? String(e.payload ?? "未知错误");
            store.setUiState("error");
            store.setLastStage("error");
            store.setBubble(msg);
            store.addLogEntry({
              timestamp: Date.now(),
              level: "error",
              message: `[agent://error] ${msg}`,
            });
          });
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
