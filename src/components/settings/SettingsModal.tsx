import React from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  PROVIDER_PRESETS,
  getProviderPreset,
  useSettingsStore,
  type LlmProvider,
} from "../../stores/useSettingsStore";
import { AgentBackendsSection } from "./AgentBackendsSection";

import { invoke } from "@tauri-apps/api/core";

const inputCls =
  "rounded-lg border border-black/5 bg-white/50 px-3 py-2 text-sm text-zinc-800 outline-none transition-all focus:border-sky-400/50 focus:bg-white/80 focus:ring-2 focus:ring-sky-400/15 dark:border-white/5 dark:bg-slate-800/50 dark:text-zinc-100 dark:focus:border-sky-400/40 dark:focus:bg-slate-800/70 dark:focus:ring-sky-400/10";

export function SettingsModal(): JSX.Element {
  const isSettingsModalOpen = useSettingsStore((s) => s.isSettingsModalOpen);
  const closeSettingsModal = useSettingsStore((s) => s.closeSettingsModal);

  const nickname = useSettingsStore((s) => s.nickname);
  const systemPrompt = useSettingsStore((s) => s.systemPrompt);
  const apiKey = useSettingsStore((s) => s.apiKey);
  const apiBaseUrl = useSettingsStore((s) => s.apiBaseUrl);
  const provider = useSettingsStore((s) => s.provider);
  const model = useSettingsStore((s) => s.model);
  const thinking = useSettingsStore((s) => s.thinking);
  const availableModels = useSettingsStore((s) => s.availableModels);

  const setNickname = useSettingsStore((s) => s.setNickname);
  const setSystemPrompt = useSettingsStore((s) => s.setSystemPrompt);
  const setApiKey = useSettingsStore((s) => s.setApiKey);
  const setApiBaseUrl = useSettingsStore((s) => s.setApiBaseUrl);
  const setProvider = useSettingsStore((s) => s.setProvider);
  const setModel = useSettingsStore((s) => s.setModel);
  const setThinking = useSettingsStore((s) => s.setThinking);
  const setAvailableModels = useSettingsStore((s) => s.setAvailableModels);

  const [localNickname, setLocalNickname] = React.useState(nickname);
  const [localSystemPrompt, setLocalSystemPrompt] = React.useState(systemPrompt);
  const [localApiKey, setLocalApiKey] = React.useState(apiKey);
  const [localApiBaseUrl, setLocalApiBaseUrl] = React.useState(apiBaseUrl);
  const [localProvider, setLocalProvider] = React.useState<LlmProvider>(provider);
  const [localModel, setLocalModel] = React.useState(model);
  const [localThinking, setLocalThinking] = React.useState(thinking);
  const [localModels, setLocalModels] = React.useState<string[]>(availableModels);

  const [fetchState, setFetchState] = React.useState<
    { kind: "idle" } | { kind: "loading" } | { kind: "ok"; count: number } | { kind: "err"; msg: string }
  >({ kind: "idle" });

  React.useEffect(() => {
    if (isSettingsModalOpen) {
      setLocalNickname(nickname);
      setLocalSystemPrompt(systemPrompt);
      setLocalApiKey(apiKey);
      setLocalApiBaseUrl(apiBaseUrl);
      setLocalProvider(provider);
      setLocalModel(model);
      setLocalThinking(thinking);
      setLocalModels(availableModels);
      setFetchState({ kind: "idle" });
    }
  }, [
    isSettingsModalOpen,
    nickname,
    systemPrompt,
    apiKey,
    apiBaseUrl,
    provider,
    model,
    thinking,
    availableModels,
  ]);

  /// 切换服务商时：base_url 跟随预设；如果当前 model 不在新预设的预期里，
  /// 用 thinking 决定换 thinkingModel 还是 defaultModel。已经手输过的 model
  /// 留给用户自己改——不强制覆盖，避免吞掉手输的特殊 id。
  const handleProviderChange = (next: LlmProvider) => {
    const preset = getProviderPreset(next);
    setLocalProvider(next);
    if (preset.baseUrl) {
      setLocalApiBaseUrl(preset.baseUrl);
    }
    // 切到新服务商时清空手输模型，强制重选（避免把 deepseek-chat 带到 OpenAI 报错）
    const target = localThinking
      ? preset.thinkingModel ?? preset.defaultModel
      : preset.defaultModel;
    setLocalModel(target);
    setLocalModels([]);
    setFetchState({ kind: "idle" });
  };

  /// thinking 切换时如果当前服务商有 thinkingModel 提示，自动换 model（用户可再改）
  const handleThinkingToggle = (next: boolean) => {
    setLocalThinking(next);
    const preset = getProviderPreset(localProvider);
    if (preset.thinkingModel) {
      setLocalModel(next ? preset.thinkingModel : preset.defaultModel);
    }
  };

  const fetchModels = async () => {
    if (!localApiBaseUrl || !localApiKey) {
      setFetchState({ kind: "err", msg: "请先填 base_url 和 API Key" });
      return;
    }
    setFetchState({ kind: "loading" });
    try {
      const list = await invoke<string[]>("list_llm_models", {
        baseUrl: localApiBaseUrl,
        apiKey: localApiKey,
      });
      setLocalModels(list);
      setFetchState({ kind: "ok", count: list.length });
    } catch (error) {
      setFetchState({ kind: "err", msg: String(error) });
    }
  };

  const handleSave = async () => {
    setNickname(localNickname);
    setSystemPrompt(localSystemPrompt);
    setApiKey(localApiKey);
    setApiBaseUrl(localApiBaseUrl);
    setProvider(localProvider);
    setModel(localModel);
    setThinking(localThinking);
    setAvailableModels(localModels);
    try {
      await invoke("update_llm_settings", {
        baseUrl: localApiBaseUrl,
        apiKey: localApiKey,
        nickname: localNickname,
        systemPrompt: localSystemPrompt,
        provider: localProvider,
        model: localModel,
        thinking: localThinking,
      });
    } catch (e) {
      console.error("Failed to update LLM settings in Rust", e);
    }
    closeSettingsModal();
  };

  const presetHint = getProviderPreset(localProvider).thinkingHint;

  return (
    <AnimatePresence>
      {isSettingsModalOpen && (
        <React.Fragment>
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 z-[200] bg-black/30 backdrop-blur-sm"
            onClick={closeSettingsModal}
          />

          <div className="fixed inset-0 z-[210] flex items-center justify-center pointer-events-none">
            <motion.div
              initial={{ opacity: 0, scale: 0.9, y: 10 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.95, y: 10 }}
              transition={{ type: "spring", damping: 25, stiffness: 300 }}
              className="pointer-events-auto flex max-h-[85vh] w-[92%] max-w-2xl flex-col rounded-2xl border border-white/60 bg-white/70 shadow-[0_20px_60px_rgba(0,0,0,0.12)] backdrop-blur-2xl dark:border-white/10 dark:bg-slate-800/60 dark:shadow-none"
            >
              <div className="flex items-center justify-between border-b border-black/5 px-6 py-4 dark:border-white/5">
                <h2 className="text-xl font-bold text-zinc-800 dark:text-zinc-100">全局设置</h2>
              </div>

              <div className="flex flex-col gap-6 overflow-y-auto px-6 py-5">
                <section className="flex flex-col gap-5">
                  <div className="flex flex-col gap-1.5">
                    <label className="text-sm font-medium text-zinc-600 dark:text-zinc-400">
                      希望团长怎样称呼你？
                    </label>
                    <input
                      type="text"
                      value={localNickname}
                      onChange={(e) => setLocalNickname(e.target.value)}
                      placeholder="例如：部员 / 阿虚"
                      className={inputCls}
                    />
                  </div>

                  <div className="flex flex-col gap-1.5">
                    <label className="text-sm font-medium text-zinc-600 dark:text-zinc-400">
                      你想跟团长说的悄悄话（系统提示词）：
                    </label>
                    <textarea
                      value={localSystemPrompt}
                      onChange={(e) => setLocalSystemPrompt(e.target.value)}
                      placeholder="例如：请用傲娇的语气回复我..."
                      className={`${inputCls} h-24 resize-y`}
                    />
                  </div>
                </section>

                <hr className="border-black/5 dark:border-white/5" />

                <section className="flex flex-col gap-4">
                  <h3 className="text-sm font-semibold text-zinc-700 dark:text-zinc-200">
                    LLM API 配置
                  </h3>

                  <div className="grid grid-cols-2 gap-3">
                    <div className="flex flex-col gap-1.5">
                      <label className="text-xs font-medium text-zinc-600 dark:text-zinc-400">
                        服务商
                      </label>
                      <select
                        value={localProvider}
                        onChange={(e) => handleProviderChange(e.target.value as LlmProvider)}
                        className={inputCls}
                      >
                        {PROVIDER_PRESETS.map((p) => (
                          <option key={p.id} value={p.id}>
                            {p.label}
                          </option>
                        ))}
                      </select>
                    </div>

                    <div className="flex flex-col gap-1.5">
                      <label className="text-xs font-medium text-zinc-600 dark:text-zinc-400">
                        Base URL
                      </label>
                      <input
                        type="text"
                        value={localApiBaseUrl}
                        onChange={(e) => setLocalApiBaseUrl(e.target.value)}
                        placeholder="https://api.deepseek.com/v1"
                        className={inputCls}
                      />
                    </div>
                  </div>

                  <div className="flex flex-col gap-1.5">
                    <label className="text-xs font-medium text-zinc-600 dark:text-zinc-400">
                      API Key
                    </label>
                    <input
                      type="password"
                      value={localApiKey}
                      onChange={(e) => setLocalApiKey(e.target.value)}
                      placeholder="sk-..."
                      className={inputCls}
                      autoComplete="off"
                    />
                  </div>

                  <div className="flex flex-col gap-1.5">
                    <div className="flex items-center justify-between">
                      <label className="text-xs font-medium text-zinc-600 dark:text-zinc-400">
                        模型 ID
                      </label>
                      <button
                        type="button"
                        onClick={fetchModels}
                        disabled={fetchState.kind === "loading"}
                        className="rounded-md border border-sky-400/50 bg-sky-500/10 px-2 py-0.5 text-[11px] font-medium text-sky-700 transition-all hover:bg-sky-500/20 disabled:cursor-not-allowed disabled:opacity-50 dark:border-sky-300/40 dark:text-sky-300"
                      >
                        {fetchState.kind === "loading" ? "拉取中…" : "拉取最新列表"}
                      </button>
                    </div>
                    <input
                      type="text"
                      list="model-list"
                      value={localModel}
                      onChange={(e) => setLocalModel(e.target.value)}
                      placeholder="deepseek-v4-flash / gpt-5.5 / kimi-k2.6 / 自定义..."
                      className={inputCls}
                    />
                    <datalist id="model-list">
                      {localModels.map((m) => (
                        <option key={m} value={m} />
                      ))}
                    </datalist>
                    {fetchState.kind === "ok" ? (
                      <p className="text-[11px] text-emerald-600 dark:text-emerald-400">
                        ✓ 拿到 {fetchState.count} 个模型，可在输入框下拉选择
                      </p>
                    ) : null}
                    {fetchState.kind === "err" ? (
                      <p className="break-all text-[11px] text-rose-600 dark:text-rose-400">
                        ✗ {fetchState.msg.slice(0, 240)}
                      </p>
                    ) : null}
                  </div>

                  <div className="flex items-start gap-3 rounded-lg border border-black/5 bg-white/30 p-3 dark:border-white/5 dark:bg-slate-800/30">
                    <button
                      type="button"
                      role="switch"
                      aria-checked={localThinking}
                      onClick={() => handleThinkingToggle(!localThinking)}
                      className={`relative mt-0.5 inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-colors ${
                        localThinking ? "bg-sky-500" : "bg-zinc-300 dark:bg-zinc-600"
                      }`}
                    >
                      <span
                        className={`inline-block h-3.5 w-3.5 transform rounded-full bg-white shadow transition-transform ${
                          localThinking ? "translate-x-[18px]" : "translate-x-1"
                        }`}
                      />
                    </button>
                    <div className="flex flex-col gap-0.5">
                      <span className="text-sm font-medium text-zinc-800 dark:text-zinc-100">
                        思考模式 (Reasoning)
                      </span>
                      <span className="text-[11px] text-zinc-500 dark:text-zinc-400">
                        启用后请求体加 enable_thinking=true。
                        {presetHint ? ` ${presetHint}。` : ""}
                        默认关闭。
                      </span>
                    </div>
                  </div>
                </section>

                <hr className="border-black/5 dark:border-white/5" />

                <AgentBackendsSection isVisible={isSettingsModalOpen} />
              </div>

              <div className="flex justify-end gap-3 border-t border-black/5 px-6 py-4 dark:border-white/5">
                <button
                  type="button"
                  onClick={closeSettingsModal}
                  className="rounded-lg px-4 py-2 text-sm font-medium text-zinc-500 transition-colors hover:bg-zinc-100/70 dark:text-zinc-400 dark:hover:bg-slate-800/70"
                >
                  取消
                </button>
                <button
                  type="button"
                  onClick={handleSave}
                  className="rounded-lg bg-sky-500 px-4 py-2 text-sm font-medium text-white shadow-md shadow-sky-400/25 transition-all hover:bg-sky-600 hover:shadow-sky-400/40 active:bg-sky-700"
                >
                  保存
                </button>
              </div>
            </motion.div>
          </div>
        </React.Fragment>
      )}
    </AnimatePresence>
  );
}
