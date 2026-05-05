// 启动时把前端持久化的 tab 列表跟后端 list_sessions 对账。
//
// Tauri 应用前后端在同一进程：Tauri app 退出 = 后端 RuntimeState drop +
// `shutdown_runtime_clients` 杀掉所有 child。重启后 list_sessions 通常返回
// 空列表 —— 没有真的"reattach 到正在跑的进程"这件事。
//
// 这个 hook 实际做的事：
//   1. 拉一次 list_sessions 看后端有没有活跃会话（极端场景：开发模式 vite
//      hot-reload 时前端重挂载但后端 process 没重启）
//   2. 把所有持久化 tab 的"运行时字段"按对账结果置位：
//      - 后端有该 sessionId 且 status=running → uiState=running, agentStatus=running
//      - 后端没有 → uiState=idle, agentStatus=idle, percent=0, bubble=""
//      （sessionId / projectPath / agent / 上次结果 都保留）
//   3. 后端有但前端没对应 tab → 自动建一个 tab 让用户能看到
//
// 重启后 ResultCard 仍然会显示上次的 emotionText / summaryTranslation /
// suggestionOptions（因为这些字段持久化了），用户能从那里继续。

import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";
import { useAppStore } from "../stores/useAppStore";
import { useTabsStore } from "../stores/useTabsStore";
import type { AgentType } from "../types/agent";
import type { SessionSummary } from "../types/ipc";

const RUNNING_STATUSES = new Set(["starting", "running", "thinking", "processing"]);

function backendStatusToTabPatch(status: string): {
  uiState: "running" | "idle" | "done" | "error";
  agentStatus: "running" | "idle" | "thinking" | "processing" | "error" | "completed";
} {
  const s = status.toLowerCase();
  if (RUNNING_STATUSES.has(s)) {
    return { uiState: "running", agentStatus: "running" };
  }
  if (s === "error") return { uiState: "error", agentStatus: "error" };
  if (s === "completed") return { uiState: "done", agentStatus: "idle" };
  return { uiState: "idle", agentStatus: "idle" };
}

function isAgentType(value: string): value is AgentType {
  return ["claude-code", "opencode", "codex", "gemini", "cursor"].includes(value);
}

export function useTabsReattach(): void {
  useEffect(() => {
    let cancelled = false;
    const run = async (): Promise<void> => {
      let sessions: SessionSummary[] = [];
      try {
        sessions = await invoke<SessionSummary[]>("list_sessions");
      } catch (err) {
        useAppStore.getState().addLogEntry({
          timestamp: Date.now(),
          level: "warn",
          message: `list_sessions 失败（启动 reattach 跳过）: ${String(err)}`,
        });
        // 后端 / 命令未就绪 — 仍然把所有 tab 的运行时字段重置为 idle
        if (!cancelled) resetAllTabsToIdle();
        return;
      }
      if (cancelled) return;

      reconcileWithBackendSessions(sessions);
    };
    void run();
    return () => {
      cancelled = true;
    };
  }, []);
}

function resetAllTabsToIdle(): void {
  const { tabs, updateTab } = useTabsStore.getState();
  for (const id of Object.keys(tabs)) {
    updateTab(id, {
      uiState: "idle",
      agentStatus: "idle",
      percent: 0,
      bubble: "",
    });
  }
}

function reconcileWithBackendSessions(sessions: SessionSummary[]): void {
  const store = useTabsStore.getState();
  const { tabs, updateTab, createTab, setActiveTab, activeTabId, order } = store;

  // 持久化恢复出来后，如果 tabs 非空但应用还在 WelcomeView 状态，
  // 直接切到主界面 — 用户上次开着的 tab 应该一进来就看到。
  if (Object.keys(tabs).length > 0 && !useAppStore.getState().isStarted) {
    useAppStore.getState().setIsStarted(true);
  }
  // activeTabId 引用已失效（持久化的 active 在 tabs 里没了）→ 切到第一个
  if ((activeTabId === null || !tabs[activeTabId]) && order.length > 0) {
    setActiveTab(order[0]);
  }

  // 1. 把后端会话按 runId 索引（注意后端可能多个 session 共享一个 runId —
  //    比如 Codex thread resume 后留了旧 thread；按最新 createdAtMs 取一份）
  const liveByRunId = new Map<string, SessionSummary>();
  for (const sess of sessions) {
    const existing = liveByRunId.get(sess.runId);
    if (!existing || sess.createdAtMs < existing.createdAtMs) {
      liveByRunId.set(sess.runId, sess);
    }
  }

  // 2. 遍历前端 tab：有对账上 → 同步状态；没对账上 → reset 到 idle
  for (const id of Object.keys(tabs)) {
    const tab = tabs[id];
    const live = liveByRunId.get(id);
    if (live) {
      const patch = backendStatusToTabPatch(live.status);
      updateTab(id, {
        ...patch,
        sessionId: live.sessionId,
        projectPath: tab.projectPath ?? live.cwd ?? null,
      });
      liveByRunId.delete(id);
    } else {
      updateTab(id, {
        uiState: "idle",
        agentStatus: "idle",
        percent: 0,
        bubble: "",
      });
    }
  }

  // 3. 后端有但前端没对应 tab 的 → 用后端 runId 作为前端 tab.id 自动建
  //    （极端情况：前端 localStorage 被清掉但后端进程仍存在）
  let firstNewId: string | null = null;
  for (const live of liveByRunId.values()) {
    const agent: AgentType = isAgentType(live.agentType) ? live.agentType : "claude-code";
    const newId = createTab({
      id: live.runId,
      title: live.lastUserPrompt?.slice(0, 24) || "已恢复会话",
      agent,
      projectPath: live.cwd ?? null,
      sessionId: live.sessionId,
      ...backendStatusToTabPatch(live.status),
    });
    if (!firstNewId) firstNewId = newId;
  }
  if (firstNewId && !activeTabId) {
    setActiveTab(firstNewId);
  }
}
