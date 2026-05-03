import { create } from "zustand";
import type {
  AgentStatus,
  AgentTab,
  AgentType,
  LastStage,
  LogEntry,
  TodoItem,
  UiState,
} from "../types/agent";

export type ThemeMode = "light" | "dark";
export type AppView = "welcome" | "main" | "settings";

interface AppState {
  currentView: AppView;
  isStarted: boolean;
  selectedAgent: AgentType;
  agentStatus: AgentStatus;
  activeAgentTab: AgentTab;
  projectPath: string | null;
  todos: TodoItem[];
  logEntries: LogEntry[];
  theme: ThemeMode;

  // Runtime state (from agent IPC)
  task: string;
  uiState: UiState;
  percent: number;
  bubble: string;
  sessionId: string | null;
  resultZh: string;
  mode: string;
  summaryTranslation: string;
  emotionText: string;
  suggestionOptions: string[];
  lastStage: LastStage;
  pendingPermission: {
    sessionId: string;
    toolName: string;
    toolDescription?: string;
    toolUseId: string;
    rawInput?: Record<string, unknown>;
  } | null;

  // Actions
  setCurrentView: (view: AppView) => void;
  setIsStarted: (isStarted: boolean) => void;
  setProjectPath: (path: string | null) => void;
  setSelectedAgent: (agent: AgentType) => void;
  setAgentStatus: (status: AgentStatus) => void;
  setActiveAgentTab: (tab: AgentTab) => void;
  setTheme: (theme: ThemeMode) => void;
  toggleTheme: () => void;
  setTask: (task: string) => void;
  setUiState: (uiState: UiState) => void;
  setPercent: (percent: number) => void;
  setBubble: (bubble: string) => void;
  setSessionId: (sessionId: string | null) => void;
  setResultZh: (resultZh: string) => void;
  setMode: (mode: string) => void;
  setSummaryTranslation: (summaryTranslation: string) => void;
  setEmotionText: (emotionText: string) => void;
  setSuggestionOptions: (options: string[]) => void;
  setLastStage: (lastStage: LastStage) => void;
  setPendingPermission: (
    p: AppState["pendingPermission"],
  ) => void;
  appendTodo: (item: TodoItem) => void;
  clearTodos: () => void;
  addLogEntry: (entry: LogEntry) => void;
  clearLogs: () => void;
  resetSession: () => void;
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
  agentStatus: "idle",
  activeAgentTab: "claude-code",
  projectPath: null,
  todos: [],
  logEntries: [],
  theme: initialTheme,

  task: "",
  uiState: "idle",
  percent: 0,
  bubble: "嗨，我是春日桌宠！输入中文任务，我会调用 Demo Agent。",
  sessionId: null,
  resultZh: "",
  mode: "idle",
  summaryTranslation: "",
  emotionText: "",
  suggestionOptions: [],
  lastStage: "default",
  pendingPermission: null,

  setCurrentView: (view) => set({ currentView: view }),
  setIsStarted: (isStarted) => set({ isStarted }),
  setProjectPath: (projectPath) => set({ projectPath }),
  setSelectedAgent: (selectedAgent) => set({ selectedAgent }),
  setAgentStatus: (agentStatus) => set({ agentStatus }),
  setActiveAgentTab: (activeAgentTab) => set({ activeAgentTab }),
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
  setTask: (task) => set({ task }),
  setUiState: (uiState) => set({ uiState }),
  setPercent: (percent) => set({ percent }),
  setBubble: (bubble) => set({ bubble }),
  setSessionId: (sessionId) => set({ sessionId }),
  setResultZh: (resultZh) => set({ resultZh }),
  setMode: (mode) => set({ mode }),
  setSummaryTranslation: (summaryTranslation) => set({ summaryTranslation }),
  setEmotionText: (emotionText) => set({ emotionText }),
  setSuggestionOptions: (suggestionOptions) => set({ suggestionOptions }),
  setLastStage: (lastStage) => set({ lastStage }),
  setPendingPermission: (pendingPermission) => set({ pendingPermission }),
  appendTodo: (item) =>
    set((state) => ({
      todos: [...state.todos.filter((t) => t.id !== item.id), item].slice(-40),
    })),
  clearTodos: () => set({ todos: [] }),
  addLogEntry: (entry) =>
    set((state) => ({
      logEntries: [...state.logEntries.slice(-79), entry],
    })),
  clearLogs: () => set({ logEntries: [] }),
  resetSession: () =>
    set({
      percent: 0,
      bubble: "",
      resultZh: "",
      mode: "idle",
      summaryTranslation: "",
      emotionText: "",
      suggestionOptions: [],
      lastStage: "default",
      uiState: "idle",
      agentStatus: "idle",
      sessionId: null,
      todos: [],
      pendingPermission: null,
    }),
}));
