import { useAppStore } from "../../stores/useAppStore";
import { motion } from "framer-motion";

export function RunningBubble(): JSX.Element {
  const bubble = useAppStore((s) => s.bubble);

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      exit={{ opacity: 0, scale: 0.95 }}
      className="relative rounded-2xl rounded-bl-sm border border-indigo-200/50 bg-indigo-50/80 p-4 shadow-sm backdrop-blur-md dark:border-indigo-900/30 dark:bg-indigo-950/20"
    >
      <div className="flex items-center gap-2 mb-2">
        <div className="h-2 w-2 rounded-full bg-indigo-500 animate-pulse" />
        <span className="text-xs font-bold uppercase tracking-wider text-indigo-600 dark:text-indigo-400">
          Agent正在全力执行...
        </span>
      </div>
      <p className="text-sm font-medium text-zinc-700 dark:text-zinc-300">
        {bubble || "脑电波同步中..."}
      </p>
    </motion.div>
  );
}

