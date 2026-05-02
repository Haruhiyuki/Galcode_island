import { create } from "zustand";
import type {
  AgentStatus,
  AgentTab,
  AgentType,
  LogEntry,
  TodoItem,
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
  setCurrentView: (view: AppView) => void;
  setIsStarted: (isStarted: boolean) => void;
  setProjectPath: (path: string | null) => void;
  setSelectedAgent: (agent: AgentType) => void;
  setAgentStatus: (status: AgentStatus) => void;
  setActiveAgentTab: (tab: AgentTab) => void;
  setTheme: (theme: ThemeMode) => void;
  toggleTheme: () => void;
}

const getSystemTheme = (): ThemeMode => {
  if (typeof window === "undefined") {
    return "light";
  }

  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
};

const initialTheme = getSystemTheme();

const applyTheme = (theme: ThemeMode): void => {
  if (typeof document === "undefined") {
    return;
  }

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
}));
