import { motion } from "framer-motion";
import { useActiveTabField } from "../../hooks/useActiveTab";

export function TodoProgress(): JSX.Element {
  const percent = useActiveTabField("percent");

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -8 }}
      transition={{ duration: 0.3 }}
      className="space-y-2"
    >
      <div className="flex items-center justify-between text-xs tracking-[0.18em] text-zinc-400 dark:text-zinc-500">
        <span>SESSION PROGRESS</span>
        <span>{Math.round(percent)}%</span>
      </div>
      <div className="h-2 overflow-hidden rounded-full bg-zinc-200/50 dark:bg-zinc-800/50">
        <motion.div
          initial={{ width: 0 }}
          animate={{ width: `${percent}%` }}
          transition={{ duration: 0.35, ease: "easeOut" }}
          className="h-full rounded-full bg-gradient-to-r from-sky-400 via-sky-500 to-sky-600 shadow-[0_0_8px_rgba(14,165,233,0.25)] dark:from-sky-500 dark:via-sky-400 dark:to-sky-300"
        />
      </div>
    </motion.div>
  );
}
