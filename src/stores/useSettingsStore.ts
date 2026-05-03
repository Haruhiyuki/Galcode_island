import { create } from "zustand";
import { persist } from "zustand/middleware";

interface SettingsState {
  nickname: string;
  systemPrompt: string;
  apiKey: string;
  apiBaseUrl: string;
  /** OpenAI-compatible chat model id; empty → Rust infers (e.g. DeepSeek host → deepseek-v4-flash). */
  apiModel: string;
  
  // Modal state
  isSettingsModalOpen: boolean;
  
  // Actions
  setNickname: (nickname: string) => void;
  setSystemPrompt: (systemPrompt: string) => void;
  setApiKey: (apiKey: string) => void;
  setApiBaseUrl: (apiBaseUrl: string) => void;
  setApiModel: (apiModel: string) => void;
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
      apiModel: "",
      isSettingsModalOpen: false,

      setNickname: (nickname) => set({ nickname }),
      setSystemPrompt: (systemPrompt) => set({ systemPrompt }),
      setApiKey: (apiKey) => set({ apiKey }),
      setApiBaseUrl: (apiBaseUrl) => set({ apiBaseUrl }),
      setApiModel: (apiModel) => set({ apiModel }),
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
        apiModel: state.apiModel,
      }), // only persist these fields
    }
  )
);
