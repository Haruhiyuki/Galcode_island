// 订阅后端 agent://* 事件，按 runId 路由到对应 tab slice。
//
// 路由策略（按优先级）：
//   1. 事件 payload 带 runId 且 store 里有这个 tab → 直接写
//   2. 事件 payload 没有 runId 但带 sessionId →
//      a. 通过 sessionId 反查 tab（findTabBySessionId）
//      b. 反查不到说明这是该 tab 第一条事件（后端先 emit status-changed，
//         然后 launch_*_agent 才返回 sessionId 给前端写回）—— 此时
//         如果只有一个 tab，写到那个；多个 tab 则丢弃 + warn
//   3. 都没有 → 丢弃 + warn
//
// 为什么 fallback 到"只有一个 tab 时直接写"：避免阶段 B 单 tab 模式下
// 后端兜底用 DEFAULT_RUN_ID 而前端 tab id 是 UUID 时事件全部丢失。这只在
// 没有 sessionId / runId 的极早期事件触发。

import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useAppStore } from "../stores/useAppStore";
import { useTabsStore, type TabState } from "../stores/useTabsStore";
import type {
  ErrorPayload,
  SessionCompletePayload,
  StatusChangedPayload,
} from "../types/ipc";

function mapAgentStatusToStage(
  st: string,
): TabState["lastStage"] {
  const s = st.toLowerCase();
  if (s === "thinking") return "thinking";
  if (s === "processing") return "working";
  if (s === "completed") return "done";
  if (s === "starting" || s === "running") return "init";
  if (s === "waitingapproval") return "thinking";
  if (s === "error") return "error";
  return "default";
}

/// 路由事件到对应 tab。返回 tab id（命中），或 null（未命中）。
function resolveTabId(
  runId: string | undefined,
  sessionId: string | undefined,
): string | null {
  const store = useTabsStore.getState();
  if (runId && store.tabs[runId]) return runId;
  if (sessionId) {
    const byId = store.findTabBySessionId(sessionId);
    if (byId) return byId;
  }
  // 单 tab 兜底：只有一个 tab + 该 tab 还没有 sessionId 时，假定事件就是它的
  const ids = store.order;
  if (ids.length === 1) {
    const onlyId = ids[0];
    const onlyTab = store.tabs[onlyId];
    if (onlyTab && onlyTab.sessionId === null) return onlyId;
  }
  return null;
}

/// 把后端 sessionId 写回 tab.sessionId（首次匹配上时），方便后续事件路由。
function ensureSessionLinked(tabId: string, sessionId: string | undefined): void {
  if (!sessionId) return;
  const tab = useTabsStore.getState().tabs[tabId];
  if (!tab || tab.sessionId === sessionId) return;
  useTabsStore.getState().updateTab(tabId, { sessionId });
}

export function useAgentIPC(): void {
  useEffect(() => {
    const unsubs: UnlistenFn[] = [];

    const run = async () => {
      unsubs.push(
        await listen<StatusChangedPayload>("agent://status-changed", (e) => {
          const p = e.payload;
          const tabId = resolveTabId(p?.runId, p?.sessionId);
          if (!tabId) {
            console.warn("[ipc] status-changed dropped, no tab match", p);
            return;
          }
          ensureSessionLinked(tabId, p?.sessionId);

          const update = useTabsStore.getState().updateTab;
          const patch: Partial<TabState> = {
            uiState: "running",
            agentStatus: "running",
            lastStage: mapAgentStatusToStage(p.status),
          };
          if (typeof p.percent === "number") {
            patch.percent = Math.max(0, Math.min(100, p.percent));
          }
          const hint = p.toolDescription ?? p.toolName;
          if (hint) patch.bubble = String(hint);
          update(tabId, patch);
        }),
      );

      unsubs.push(
        await listen<SessionCompletePayload>("agent://session-complete", (e) => {
          const p = e.payload;
          const tabId = resolveTabId(p?.runId, p?.sessionId);
          if (!tabId) {
            console.warn("[ipc] session-complete dropped, no tab match", p);
            return;
          }
          ensureSessionLinked(tabId, p?.sessionId);

          const update = useTabsStore.getState().updateTab;
          update(tabId, {
            uiState: "done",
            percent: 100,
            lastStage: "done",
            mode: p.mode ?? "complete",
            resultZh: p.resultZh ?? "",
            summaryTranslation: p.summaryTranslation ?? "",
            emotionText: p.emotion ?? "",
            suggestionOptions: p.suggestionOptions ?? [],
            bubble: p.emotion || "任务完成！",
            agentStatus: "idle",
          });

          // 非活动 tab 完成时打未读小红点（D 阶段 TabBar 会显示）
          const activeId = useTabsStore.getState().activeTabId;
          if (activeId !== tabId) {
            update(tabId, { hasUnread: true });
          }
        }),
      );

      unsubs.push(
        await listen<ErrorPayload>("agent://error", (e) => {
          const p = e.payload;
          const tabId = resolveTabId(p?.runId, p?.sessionId);
          const msg = p?.message ?? String(e.payload ?? "未知错误");
          useAppStore.getState().addLogEntry({
            timestamp: Date.now(),
            level: "error",
            message: `[agent://error]${tabId ? ` tab=${tabId}` : ""} ${msg}`,
          });
          if (!tabId) return;
          useTabsStore.getState().updateTab(tabId, {
            uiState: "error",
            agentStatus: "error",
            lastStage: "error",
            bubble: msg,
          });
        }),
      );
    };

    void run();

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
