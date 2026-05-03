import { useAppStore } from "../../stores/useAppStore";
import { motion } from "framer-motion";

export function ResultCard(): JSX.Element {
  const mode = useAppStore((s) => s.mode);
  const emotionText = useAppStore((s) => s.emotionText);
  const summaryTranslation = useAppStore((s) => s.summaryTranslation);
  const suggestionOptions = useAppStore((s) => s.suggestionOptions);
  const setTask = useAppStore((s) => s.setTask);
  const setUiState = useAppStore((s) => s.setUiState);
  const setMode = useAppStore((s) => s.setMode);

  const handleOptionClick = (opt: string) => {
    setTask(opt);
    setUiState("idle");
    setMode("idle");
  };

  const isError = mode === "error";
  const headerColor = isError ? "text-rose-600 dark:text-rose-400" : "text-emerald-600 dark:text-emerald-400";
  const borderColor = isError ? "border-rose-500/30" : "border-emerald-500/30";
  const bgLight = isError ? "bg-rose-100" : "bg-emerald-50/80";
  const bgDark = isError ? "dark:bg-rose-950/30" : "dark:bg-emerald-950/20";

  return (
    <motion.section
      initial={{ opacity: 0, y: 10, scale: 0.98 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, y: -10, scale: 0.98 }}
      transition={{ duration: 0.3 }}
      className={`flex w-full flex-col gap-3 rounded-2xl border ${borderColor} ${bgLight} ${bgDark} p-4 backdrop-blur-md shadow-sm`}
    >
      {/* Header */}
      <div className="flex items-center gap-2">
        <div className={`h-2.5 w-2.5 rounded-full ${isError ? "bg-rose-500 shadow-[0_0_8px_rgba(244,63,94,0.6)]" : "bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.6)]"} animate-pulse`} />
        <span className={`text-sm font-extrabold uppercase tracking-widest ${headerColor}`}>
          {mode || "COMPLETE"}
        </span>
      </div>

      {/* Emotion Speech (Haruhi) */}
      {emotionText && (
        <div className="relative rounded-t-2xl rounded-br-2xl rounded-bl-sm border border-white/40 bg-white/70 dark:border-white/10 dark:bg-zinc-800/80 p-4 shadow-sm">
          <p className="text-[15px] font-bold leading-snug text-zinc-800 dark:text-zinc-100">
            {emotionText}
          </p>
        </div>
      )}

      {/* Summary */}
      {summaryTranslation && (
        <div className="px-1 text-sm leading-relaxed text-zinc-700 dark:text-zinc-300">
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
              className="flex items-center rounded-full border border-zinc-300/80 bg-white/40 px-3.5 py-1.5 text-xs font-semibold text-zinc-700 backdrop-blur-sm transition-all hover:-translate-y-0.5 hover:bg-white hover:text-zinc-900 hover:shadow-sm active:translate-y-0 active:scale-95 dark:border-zinc-700 dark:bg-zinc-900/50 dark:text-zinc-300 dark:hover:bg-zinc-800 dark:hover:text-white"
            >
              {opt}
            </button>
          ))}
        </div>
      )}
    </motion.section>
  );
}

