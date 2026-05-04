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

/// 服务商预设 —— 选定后自动填 base_url 和默认 model id。
/// "custom" 让用户自己填 base_url。
export type LlmProvider =
  | "deepseek"
  | "openai"
  | "moonshot"
  | "qwen"
  | "zhipu"
  | "openrouter"
  | "custom";

export interface ProviderPreset {
  id: LlmProvider;
  label: string;
  baseUrl: string;
  /// 该服务商上"思考模式 = ON" 时建议的 model id（可与默认 model 不同）
  thinkingModel?: string;
  /// 默认 model（思考关闭时用）
  defaultModel: string;
  /// 在思考模式开关上额外说明（如 "DeepSeek 用 deepseek-reasoner"）
  thinkingHint?: string;
}

export const PROVIDER_PRESETS: ProviderPreset[] = [
  {
    id: "deepseek",
    label: "DeepSeek V4",
    baseUrl: "https://api.deepseek.com/v1",
    defaultModel: "deepseek-v4-flash",
    thinkingHint: "V4 Flash/Pro 双模：思考开关通过 enable_thinking 参数切（同 model id）",
  },
  {
    id: "openai",
    label: "OpenAI",
    baseUrl: "https://api.openai.com/v1",
    defaultModel: "gpt-5.5",
    thinkingModel: "o4-mini",
    thinkingHint: "开启后切到 o4-mini 推理模型",
  },
  {
    id: "moonshot",
    label: "Moonshot Kimi",
    baseUrl: "https://api.moonshot.cn/v1",
    defaultModel: "kimi-k2.6",
    thinkingHint: "K2 系列支持 enable_thinking 参数切",
  },
  {
    id: "qwen",
    label: "通义千问",
    baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
    defaultModel: "qwen3.6-flash",
    thinkingModel: "qwen3.6-plus",
    thinkingHint: "qwen3.6-plus 思考 + Web 搜索 + Code 工具",
  },
  {
    id: "zhipu",
    label: "智谱 GLM",
    baseUrl: "https://open.bigmodel.cn/api/paas/v4",
    defaultModel: "glm-5",
    thinkingModel: "glm-5.1",
    thinkingHint: "GLM-5.1 智能体优化版",
  },
  {
    id: "openrouter",
    label: "OpenRouter",
    baseUrl: "https://openrouter.ai/api/v1",
    defaultModel: "deepseek/deepseek-v4-flash",
  },
  {
    id: "custom",
    label: "自定义",
    baseUrl: "",
    defaultModel: "",
  },
];

export function getProviderPreset(id: LlmProvider): ProviderPreset {
  return PROVIDER_PRESETS.find((p) => p.id === id) ?? PROVIDER_PRESETS[PROVIDER_PRESETS.length - 1];
}

interface SettingsState {
  nickname: string;
  systemPrompt: string;
  apiKey: string;
  apiBaseUrl: string;
  provider: LlmProvider;
  model: string;
  thinking: boolean;
  /// 缓存上次拉到的模型列表，避免每次 SettingsModal 打开都拉
  availableModels: string[];

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
  setProvider: (provider: LlmProvider) => void;
  setModel: (model: string) => void;
  setThinking: (thinking: boolean) => void;
  setAvailableModels: (models: string[]) => void;
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
      apiBaseUrl: "https://api.deepseek.com/v1",
      provider: "deepseek",
      model: "deepseek-v4-flash",
      thinking: false,
      availableModels: [],
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
      setProvider: (provider) => set({ provider }),
      setModel: (model) => set({ model }),
      setThinking: (thinking) => set({ thinking }),
      setAvailableModels: (availableModels) => set({ availableModels }),
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
        provider: state.provider,
        model: state.model,
        thinking: state.thinking,
        availableModels: state.availableModels,
        backends: state.backends,
      }),
    }
  )
);
