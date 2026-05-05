// 应用全局 store —— 只放真正全局的字段。
//
// 多 tab 重构后，所有"会话级"字段（task / sessionId / cliBlocks / mode /
// uiState / agentStatus / bubble / percent / resultZh / summaryTranslation /
// emotionText / suggestionOptions / lastStage / projectPath）都搬到了
// `useTabsStore` 的 TabState 里，每个 tab 一份。
//
// 留在这里的全局字段：
//   - currentView / isStarted：导航
//   - theme：dark/light
//   - selectedAgent：**新建 tab 时的默认 agent**（不是当前 tab 的 agent —— 那个在 TabState）
//   - logEntries：跨 tab 共享的诊断日志栏

import { create } from "zustand";
import type { AgentType, LogEntry } from "../types/agent";

export type ThemeMode = "light" | "dark";
export type AppView = "welcome" | "main" | "settings";

interface AppState {
  currentView: AppView;
  isStarted: boolean;
  /// 新建 tab 时的默认 backend；当前 tab 在用哪个 agent 看 TabState.agent。
  /// WelcomeView 用户选 agent → 创建第一个 tab 时拿这个值；
  /// MainView 顶部的 AgentSelector 改的是 active tab 的 agent，会一并写回这里
  /// 作为下次新 tab 的默认。
  selectedAgent: AgentType;
  theme: ThemeMode;
  /// 跨 tab 共享的诊断日志（错误 / 系统提示）；不影响业务。
  logEntries: LogEntry[];

  setCurrentView: (view: AppView) => void;
  setIsStarted: (isStarted: boolean) => void;
  setSelectedAgent: (agent: AgentType) => void;
  setTheme: (theme: ThemeMode) => void;
  toggleTheme: () => void;
  addLogEntry: (entry: LogEntry) => void;
  clearLogs: () => void;
}

const getSystemTheme = (): ThemeMode => {
  if (typeof window === "undefined") return "light";
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
};

const initialTheme = getSystemTheme();

const applyTheme = (theme: ThemeMode): void => {
  if (typeof document === "undefined") return;
  document.documentElement.classList.toggle("dark", theme === "dark");
  document.documentElement.dataset.theme = theme;
};

applyTheme(initialTheme);

export const useAppStore = create<AppState>((set) => ({
  currentView: "welcome",
  isStarted: false,
  selectedAgent: "claude-code",
  theme: initialTheme,
  logEntries: [],

  setCurrentView: (view) => set({ currentView: view }),
  setIsStarted: (isStarted) => set({ isStarted }),
  setSelectedAgent: (selectedAgent) => set({ selectedAgent }),
  setTheme: (theme) => {
    applyTheme(theme);
    set({ theme });
  },
  toggleTheme: () =>
    set((state) => {
      const nextTheme = state.theme === "light" ? "dark" : "light";
      applyTheme(nextTheme);
      return { theme: nextTheme };
    }),
  addLogEntry: (entry) =>
    set((state) => ({
      logEntries: [...state.logEntries.slice(-79), entry],
    })),
  clearLogs: () => set({ logEntries: [] }),
}));
