import { motion, AnimatePresence } from "framer-motion";
import { useAppStore } from "../../stores/useAppStore";
import { AgentStatusBadge } from "./AgentStatusBadge";
import { BlockStream } from "./BlockStream";
import { TodoProgress } from "./TodoProgress";
import { LogStream } from "./LogStream";

export function StatusMonitor(): JSX.Element {
  const uiState = useAppStore((s) => s.uiState);
  const mode = useAppStore((s) => s.mode);
  const isVisible = uiState === "running" || mode === "working" || mode === "thinking";

  return (
    <AnimatePresence>
      {isVisible && (
        <motion.section
          key="status-monitor"
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          exit={{ opacity: 0, y: -10 }}
          transition={{ duration: 0.3 }}
          className="flex h-full min-h-0 flex-col gap-3 overflow-hidden rounded-xl border border-white/60 bg-white/70 p-4 shadow-[0_8px_30px_rgba(0,0,0,0.04)] backdrop-blur-2xl dark:border-white/10 dark:bg-slate-800/60 dark:shadow-none"
        >
          <div className="flex shrink-0 items-center justify-between">
            <AgentStatusBadge />
          </div>
          <div className="shrink-0">
            <TodoProgress />
          </div>
          <div className="min-h-0 flex-1">
            <BlockStream />
          </div>
          <div className="shrink-0">
            <LogStream />
          </div>
        </motion.section>
      )}
    </AnimatePresence>
  );
}
