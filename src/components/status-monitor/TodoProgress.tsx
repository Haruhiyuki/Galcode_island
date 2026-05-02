import { motion } from "framer-motion";
import { useAppStore } from "../../stores/useAppStore";

export function TodoProgress(): JSX.Element {
  const percent = useAppStore((s) => s.percent);

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between text-xs tracking-[0.18em] text-zinc-500 dark:text-zinc-400">
        <span>SESSION PROGRESS</span>
        <span>{Math.round(percent)}%</span>
      </div>
      <div className="h-2 overflow-hidden rounded-full bg-zinc-200/70 dark:bg-zinc-800/70">
        <motion.div
          initial={{ width: 0 }}
          animate={{ width: `${percent}%` }}
          transition={{ duration: 0.35, ease: "easeOut" }}
          className="h-full rounded-full bg-gradient-to-r from-zinc-500 via-zinc-700 to-zinc-900 dark:from-zinc-400 dark:via-zinc-300 dark:to-zinc-100"
        />
      </div>
    </div>
  );
}
