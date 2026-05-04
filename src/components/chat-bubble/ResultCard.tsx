import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../../stores/useAppStore";
import { motion, AnimatePresence } from "framer-motion";

export function ResultCard(): JSX.Element {
  const mode = useAppStore((s) => s.mode);
  const uiState = useAppStore((s) => s.uiState);
  const emotionText = useAppStore((s) => s.emotionText);
  const summaryTranslation = useAppStore((s) => s.summaryTranslation);
  const suggestionOptions = useAppStore((s) => s.suggestionOptions);
  const setTask = useAppStore((s) => s.setTask);
  const setUiState = useAppStore((s) => s.setUiState);
  const setMode = useAppStore((s) => s.setMode);
  const setAgentStatus = useAppStore((s) => s.setAgentStatus);
  const setSessionId = useAppStore((s) => s.setSessionId);
  const setPercent = useAppStore((s) => s.setPercent);
  const setResultZh = useAppStore((s) => s.setResultZh);
  const setSummaryTranslation = useAppStore((s) => s.setSummaryTranslation);
  const setEmotionText = useAppStore((s) => s.setEmotionText);
  const setSuggestionOptions = useAppStore((s) => s.setSuggestionOptions);
  const addLogEntry = useAppStore((s) => s.addLogEntry);

  const isVisible = uiState === "done" || uiState === "error" || mode === "complete" || mode === "suggestion" || mode === "error";

  /// 点选项 = 立即启动新一轮（不回 idle，避免 InputBubble 还没显示就把上一轮的
  /// 状态条都清掉造成"白屏"）
  const handleOptionClick = async (opt: string) => {
    setTask(opt);
    setSessionId(null);
    setPercent(0);
    setResultZh("");
    setSummaryTranslation("");
    setEmotionText("");
    setSuggestionOptions([]);
    setUiState("running");
    setMode("working");
    setAgentStatus("running");

    const { selectedAgent, projectPath } = useAppStore.getState();
    try {
      const res = await invoke<{ sessionId?: string }>("start_agent", {
        userInputZh: opt,
        cwd: projectPath || ".",
        agent: selectedAgent,
      });
      if (res?.sessionId) setSessionId(res.sessionId);
    } catch (err) {
      addLogEntry({
        timestamp: Date.now(),
        level: "error",
        message: `launch err: ${String(err)}`,
      });
      setUiState("error");
      setMode("error");
      setAgentStatus("error");
    }
  };

  const isError = mode === "error" || uiState === "error";
  const headerColor = isError ? "text-rose-500 dark:text-rose-400" : "text-emerald-600 dark:text-emerald-400";

  return (
    <AnimatePresence>
      {isVisible && (
        <motion.div
          key="result-card"
          initial={{ opacity: 0, y: 10, scale: 0.98 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: -10, scale: 0.98 }}
          transition={{ type: "spring", damping: 22, stiffness: 280 }}
          className="relative w-full overflow-hidden rounded-2xl p-[2px] shadow-lg shadow-amber-500/10 dark:shadow-none"
        >
          {/* Faint amber base layer */}
          <div className="absolute inset-0 bg-amber-100/50 dark:bg-amber-400/10" />

          {/* Spinning conic gradient glow — longer visible arc in light mode */}
          <div className="absolute top-[-50%] left-[-50%] h-[200%] w-[200%] animate-[spin_4s_linear_infinite] bg-[conic-gradient(from_0deg,transparent_60%,#fb923c_100%)] dark:bg-[conic-gradient(from_0deg,transparent_75%,#fb923c_100%)]" />

          {/* Inner glass content container */}
          <div className="relative flex h-full w-full flex-col gap-3 rounded-[14px] border border-white/60 bg-white/70 p-4 backdrop-blur-2xl dark:border-white/10 dark:bg-slate-800/60">
            {/* Header */}
            <div className="flex items-center gap-2">
              <div className={`h-2.5 w-2.5 rounded-full ${isError ? "bg-rose-400 shadow-[0_0_6px_rgba(251,113,133,0.5)]" : "bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.5)]"} animate-pulse`} />
              <span className={`text-sm font-extrabold uppercase tracking-widest ${headerColor}`}>
                {mode || "COMPLETE"}
              </span>
            </div>

            {/* Emotion Speech */}
            {emotionText && (
              <div className="relative rounded-t-2xl rounded-br-2xl rounded-bl-sm border border-white/50 bg-white/70 p-4 shadow-sm backdrop-blur-md dark:border-white/10 dark:bg-slate-700/60">
                <p className="text-[15px] font-bold leading-snug text-zinc-800 dark:text-zinc-100">
                  {emotionText}
                </p>
              </div>
            )}

            {/* Summary */}
            {summaryTranslation && (
              <div className="px-1 text-sm leading-relaxed text-zinc-600 dark:text-zinc-300">
                {summaryTranslation}
              </div>
            )}

            {/* Suggestion Options */}
            {suggestionOptions?.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-2 pt-1 border-t border-zinc-200/50 dark:border-zinc-700/50">
                {suggestionOptions.map((opt, i) => (
                  <button
                    key={i}
                    onClick={() => handleOptionClick(opt)}
                    className="flex items-center rounded-full border border-white/40 bg-white/50 px-3.5 py-1.5 text-xs font-semibold text-zinc-700 shadow-sm backdrop-blur-sm transition-all hover:-translate-y-0.5 hover:bg-white hover:text-zinc-900 hover:shadow-md active:translate-y-0 active:scale-95 dark:border-white/10 dark:bg-slate-700/50 dark:text-zinc-300 dark:hover:bg-slate-600 dark:hover:text-white"
                  >
                    {opt}
                  </button>
                ))}
              </div>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
