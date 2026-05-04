import { create } from "zustand";
import { persist } from "zustand/middleware";

export type BackendKey = "claude-code" | "codex" | "opencode";

export interface BackendPrefs {
  model: string;
  effort: string;
  proxy: string;
  binary: string;
}

const emptyBackendPrefs = (): BackendPrefs => ({
  model: "",
  effort: "",
  proxy: "",
  binary: "",
});

interface SettingsState {
  nickname: string;
  systemPrompt: string;
  apiKey: string;
  apiBaseUrl: string;

  /// 三个 backend 各自的 model / effort / proxy / binary。空字符串表示用默认。
  /// 启动时由 App.tsx 同步到 Rust 端 update_backend_preferences；保存时也同步。
  backends: Record<BackendKey, BackendPrefs>;

  // Modal state
  isSettingsModalOpen: boolean;

  // Actions
  setNickname: (nickname: string) => void;
  setSystemPrompt: (systemPrompt: string) => void;
  setApiKey: (apiKey: string) => void;
  setApiBaseUrl: (apiBaseUrl: string) => void;
  setBackendPref: (backend: BackendKey, field: keyof BackendPrefs, value: string) => void;
  setBackendPrefs: (backend: BackendKey, prefs: Partial<BackendPrefs>) => void;
  openSettingsModal: () => void;
  closeSettingsModal: () => void;
}

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      nickname: "",
      systemPrompt: "",
      apiKey: "",
      apiBaseUrl: "",
      backends: {
        "claude-code": emptyBackendPrefs(),
        codex: emptyBackendPrefs(),
        opencode: emptyBackendPrefs(),
      },
      isSettingsModalOpen: false,

      setNickname: (nickname) => set({ nickname }),
      setSystemPrompt: (systemPrompt) => set({ systemPrompt }),
      setApiKey: (apiKey) => set({ apiKey }),
      setApiBaseUrl: (apiBaseUrl) => set({ apiBaseUrl }),
      setBackendPref: (backend, field, value) =>
        set((state) => ({
          backends: {
            ...state.backends,
            [backend]: { ...state.backends[backend], [field]: value },
          },
        })),
      setBackendPrefs: (backend, prefs) =>
        set((state) => ({
          backends: {
            ...state.backends,
            [backend]: { ...state.backends[backend], ...prefs },
          },
        })),
      openSettingsModal: () => set({ isSettingsModalOpen: true }),
      closeSettingsModal: () => set({ isSettingsModalOpen: false }),
    }),
    {
      name: "agent-settings-storage",
      partialize: (state) => ({
        nickname: state.nickname,
        systemPrompt: state.systemPrompt,
        apiKey: state.apiKey,
        apiBaseUrl: state.apiBaseUrl,
        backends: state.backends,
      }),
    }
  )
);
