import { create } from "zustand";
import { persist } from "zustand/middleware";

interface SettingsState {
  nickname: string;
  systemPrompt: string;
  apiKey: string;
  apiBaseUrl: string;
  
  // Modal state
  isSettingsModalOpen: boolean;
  
  // Actions
  setNickname: (nickname: string) => void;
  setSystemPrompt: (systemPrompt: string) => void;
  setApiKey: (apiKey: string) => void;
  setApiBaseUrl: (apiBaseUrl: string) => void;
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
      isSettingsModalOpen: false,

      setNickname: (nickname) => set({ nickname }),
      setSystemPrompt: (systemPrompt) => set({ systemPrompt }),
      setApiKey: (apiKey) => set({ apiKey }),
      setApiBaseUrl: (apiBaseUrl) => set({ apiBaseUrl }),
      openSettingsModal: () => set({ isSettingsModalOpen: true }),
      closeSettingsModal: () => set({ isSettingsModalOpen: false }),
    }),
    {
      name: "agent-settings-storage",
      partialize: (state) => ({ 
        nickname: state.nickname, 
        systemPrompt: state.systemPrompt, 
        apiKey: state.apiKey, 
        apiBaseUrl: state.apiBaseUrl 
      }), // only persist these fields
    }
  )
);
