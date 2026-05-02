import React from "react";
import { motion, AnimatePresence } from "framer-motion";
import { useSettingsStore } from "../../stores/useSettingsStore";

import { invoke } from "@tauri-apps/api/core";

export function SettingsModal(): JSX.Element {
  const isSettingsModalOpen = useSettingsStore((s) => s.isSettingsModalOpen);
  const closeSettingsModal = useSettingsStore((s) => s.closeSettingsModal);
  
  const nickname = useSettingsStore((s) => s.nickname);
  const systemPrompt = useSettingsStore((s) => s.systemPrompt);
  const apiKey = useSettingsStore((s) => s.apiKey);
  const apiBaseUrl = useSettingsStore((s) => s.apiBaseUrl);

  const setNickname = useSettingsStore((s) => s.setNickname);
  const setSystemPrompt = useSettingsStore((s) => s.setSystemPrompt);
  const setApiKey = useSettingsStore((s) => s.setApiKey);
  const setApiBaseUrl = useSettingsStore((s) => s.setApiBaseUrl);

  const [localNickname, setLocalNickname] = React.useState(nickname);
  const [localSystemPrompt, setLocalSystemPrompt] = React.useState(systemPrompt);
  const [localApiKey, setLocalApiKey] = React.useState(apiKey);
  const [localApiBaseUrl, setLocalApiBaseUrl] = React.useState(apiBaseUrl);

  React.useEffect(() => {
    if (isSettingsModalOpen) {
      setLocalNickname(nickname);
      setLocalSystemPrompt(systemPrompt);
      setLocalApiKey(apiKey);
      setLocalApiBaseUrl(apiBaseUrl);
    }
  }, [isSettingsModalOpen, nickname, systemPrompt, apiKey, apiBaseUrl]);

  const handleSave = async () => {
    setNickname(localNickname);
    setSystemPrompt(localSystemPrompt);
    setApiKey(localApiKey);
    setApiBaseUrl(localApiBaseUrl);
    try {
      await invoke("update_llm_settings", {
        baseUrl: localApiBaseUrl,
        apiKey: localApiKey,
        nickname: localNickname,
        systemPrompt: localSystemPrompt,
      });
    } catch (e) {
      console.error("Failed to update LLM settings in Rust", e);
    }
    closeSettingsModal();
  };

  return (
    <AnimatePresence>
      {isSettingsModalOpen && (
        <React.Fragment>
          {/* Backdrop */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.2 }}
            className="fixed inset-0 z-[200] bg-black/40 backdrop-blur-sm"
            onClick={closeSettingsModal}
          />
          
          <div className="fixed inset-0 z-[210] flex items-center justify-center pointer-events-none">
            <motion.div
              initial={{ opacity: 0, scale: 0.9, y: 10 }}
              animate={{ opacity: 1, scale: 1, y: 0 }}
              exit={{ opacity: 0, scale: 0.95, y: 10 }}
              transition={{ type: "spring", damping: 25, stiffness: 300 }}
              className="pointer-events-auto w-[90%] max-w-md rounded-2xl bg-white p-6 shadow-2xl border border-zinc-200 dark:bg-zinc-900 dark:border-zinc-800"
            >
              <h2 className="mb-6 text-xl font-bold text-zinc-800 dark:text-zinc-100">
                全局设置
              </h2>

            <div className="flex flex-col gap-5">
              {/* Field 1 */}
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium text-zinc-700 dark:text-zinc-300">
                  希望团长怎样称呼你？
                </label>
                <input
                  type="text"
                  value={localNickname}
                  onChange={(e) => setLocalNickname(e.target.value)}
                  placeholder="例如：部员 / 阿虚"
                  className="rounded-lg border border-zinc-300 bg-zinc-50 px-3 py-2 text-sm text-zinc-800 outline-none focus:border-blue-500 focus:bg-white dark:border-zinc-700 dark:bg-zinc-950 dark:text-zinc-100 dark:focus:border-blue-500"
                />
              </div>

              {/* Field 2 */}
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium text-zinc-700 dark:text-zinc-300">
                  你想跟团长说的悄悄话（系统提示词）：
                </label>
                <textarea
                  value={localSystemPrompt}
                  onChange={(e) => setLocalSystemPrompt(e.target.value)}
                  placeholder="例如：请用傲娇的语气回复我..."
                  className="h-28 resize-y rounded-lg border border-zinc-300 bg-zinc-50 px-3 py-2 text-sm text-zinc-800 outline-none focus:border-blue-500 focus:bg-white dark:border-zinc-700 dark:bg-zinc-950 dark:text-zinc-100 dark:focus:border-blue-500"
                />
              </div>

              {/* Field 3 */}
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium text-zinc-700 dark:text-zinc-300">
                  你的 API Key
                </label>
                <input
                  type="password"
                  value={localApiKey}
                  onChange={(e) => setLocalApiKey(e.target.value)}
                  placeholder="sk-..."
                  className="rounded-lg border border-zinc-300 bg-zinc-50 px-3 py-2 text-sm text-zinc-800 outline-none focus:border-blue-500 focus:bg-white dark:border-zinc-700 dark:bg-zinc-950 dark:text-zinc-100 dark:focus:border-blue-500"
                />
              </div>

              {/* Field 4 */}
              <div className="flex flex-col gap-1.5">
                <label className="text-sm font-medium text-zinc-700 dark:text-zinc-300">
                  API Base URL
                </label>
                <input
                  type="text"
                  value={localApiBaseUrl}
                  onChange={(e) => setLocalApiBaseUrl(e.target.value)}
                  placeholder="例如：https://api.openai.com/v1"
                  className="rounded-lg border border-zinc-300 bg-zinc-50 px-3 py-2 text-sm text-zinc-800 outline-none focus:border-blue-500 focus:bg-white dark:border-zinc-700 dark:bg-zinc-950 dark:text-zinc-100 dark:focus:border-blue-500"
                />
              </div>
            </div>

            {/* Actions */}
            <div className="mt-8 flex justify-end gap-3">
              <button
                type="button"
                onClick={closeSettingsModal}
                className="rounded-lg px-4 py-2 text-sm font-medium text-zinc-600 transition-colors hover:bg-zinc-100 dark:text-zinc-400 dark:hover:bg-zinc-800"
              >
                取消
              </button>
              <button
                type="button"
                onClick={handleSave}
                className="rounded-lg bg-blue-500 px-4 py-2 text-sm font-medium text-white shadow-sm transition-colors hover:bg-blue-600"
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
