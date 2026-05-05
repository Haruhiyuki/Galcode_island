import { AnimatePresence, motion } from "framer-motion";
import { useState } from "react";
import { useAppStore } from "../stores/useAppStore";
import { PetCharacter } from "./pet-character/PetCharacter";
import type { AgentType } from "../types/agent";

import { InputBubble } from "./chat-bubble/InputBubble";
import { ResultCard } from "./chat-bubble/ResultCard";
import { RunningBubble } from "./chat-bubble/RunningBubble";
import { StatusMonitor } from "./status-monitor/StatusMonitor";

function AgentSelector(): JSX.Element {
  const selectedAgent = useAppStore((s) => s.selectedAgent);
  const setSelectedAgent = useAppStore((s) => s.setSelectedAgent);
  const [isOpen, setIsOpen] = useState(false);

  const options: { value: AgentType; label: string }[] = [
    { value: "claude-code", label: "Claude Code" },
    { value: "opencode", label: "OpenCode" },
    { value: "codex", label: "Codex" },
  ];
  const selectedLabel = options.find((o) => o.value === selectedAgent)?.label ?? "Claude Code";

  return (
    <div className="relative inline-block text-left">
      <button
        onClick={() => setIsOpen((p) => !p)}
        className="inline-flex w-full items-center justify-between gap-x-2 rounded-xl border border-white/40 bg-white/50 px-3 py-1.5 text-sm font-semibold text-zinc-700 shadow-sm backdrop-blur-md transition-all hover:bg-white/70 hover:shadow-md dark:border-white/10 dark:bg-zinc-800/50 dark:text-zinc-200 dark:hover:bg-zinc-800/70"
      >
        {selectedLabel}
        <svg className="-mr-1 h-5 w-5 text-zinc-400" viewBox="0 0 20 20" fill="currentColor">
          <path fillRule="evenodd" d="M5.23 7.21a.75.75 0 011.06.02L10 11.168l3.71-3.938a.75.75 0 111.08 1.04l-4.25 4.5a.75.75 0 01-1.08 0l-4.25-4.5a.75.75 0 01.02-1.06z" clipRule="evenodd" />
        </svg>
      </button>
      <AnimatePresence>
        {isOpen && (
          <motion.div
            initial={{ opacity: 0, y: -5, scale: 0.95 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -5, scale: 0.95 }}
            transition={{ type: "spring", damping: 25, stiffness: 300 }}
            className="absolute left-0 z-10 mt-2 w-40 origin-top-left overflow-hidden rounded-xl border border-white/40 bg-white/70 p-1 shadow-[0_8px_30px_rgba(0,0,0,0.06)] backdrop-blur-xl dark:border-white/10 dark:bg-zinc-800/80 dark:shadow-[0_8px_30px_rgba(0,0,0,0.2)]"
          >
            <div className="flex flex-col gap-0.5">
              {options.map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => {
                    setSelectedAgent(opt.value);
                    setIsOpen(false);
                  }}
                  className="block w-full rounded-lg px-3 py-2 text-left text-sm font-medium text-zinc-700 transition-colors hover:bg-zinc-100/80 dark:text-zinc-200 dark:hover:bg-zinc-700/50"
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function StatusLight(): JSX.Element {
  const agentStatus = useAppStore((s) => s.agentStatus);
  const isRunning = agentStatus === "running" || agentStatus === "thinking" || agentStatus === "processing";
  const isError = agentStatus === "error";
  const bg = isRunning ? "bg-sky-400" : isError ? "bg-rose-400" : "bg-emerald-400";
  const shadow = isRunning ? "shadow-[0_0_8px_rgba(56,189,248,0.5)]" : isError ? "shadow-[0_0_6px_rgba(251,113,133,0.4)]" : "shadow-[0_0_6px_rgba(52,211,153,0.4)]";

  return (
    <div className="flex flex-col items-center justify-center gap-1">
      <div className={`h-3 w-3 rounded-full ${bg} ${shadow} ${isRunning ? "animate-pulse" : ""}`} />
      <span className="text-[10px] uppercase tracking-wider text-zinc-400 dark:text-zinc-500 font-medium">
        {agentStatus}
      </span>
    </div>
  );
}

export function MainView(): JSX.Element {
  const projectPath = useAppStore((s) => s.projectPath);
  const uiState = useAppStore((s) => s.uiState);
  const mode = useAppStore((s) => s.mode);
  const cliBlockCount = useAppStore((s) => s.cliBlocks.length);
  // 完成后保留 StatusMonitor 让 BlockStream 历史可见，跟 ResultCard 共存。
  // 下一轮提交时 InputBubble.handleLaunch 会 setSessionId(null) → useCliStream
  // 自动 clearCliBlocks，StatusMonitor 显示空占位再开始新一轮累积。
  const showStatus =
    uiState === "running" ||
    mode === "working" ||
    mode === "thinking" ||
    cliBlockCount > 0;

  return (
    <motion.section
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -10 }}
      transition={{ duration: 0.42, ease: "easeOut" }}
      className="mx-auto flex h-full w-full max-w-7xl flex-col gap-4 p-4"
    >
      {/* Top Header */}
      <div className="flex items-start justify-between gap-4 border-b border-black/5 pb-3 dark:border-white/5">
        <div className="min-w-0">
          <p className="mb-2 truncate text-xs font-medium text-zinc-500/85 dark:text-zinc-400/85">
            当前工程：{projectPath ?? "未选择"}
          </p>
          <AgentSelector />
        </div>
        <StatusLight />
      </div>

      {/* Status Monitor Section —— flex-1 + min-h-0 让它在 turn 期间占满中部空间，
          BlockStream 在内部能拿到足够高度显示流式块 */}
      <AnimatePresence mode="popLayout">
        {showStatus && (
          <motion.div
            key="status-monitor"
            initial={{ opacity: 0, height: 0, scale: 0.98 }}
            animate={{ opacity: 1, height: "auto", scale: 1 }}
            exit={{ opacity: 0, height: 0, scale: 0.98 }}
            transition={{ duration: 0.3 }}
            className="flex flex-1 min-h-0 flex-col overflow-hidden"
          >
            <StatusMonitor />
          </motion.div>
        )}
      </AnimatePresence>

      {/* 没在跑时给个弹簧保证 PetCharacter 在底部；跑起来时 StatusMonitor 已经
          flex-1 占空间，这个 spacer 会被压缩掉 */}
      {!showStatus && <div className="flex-1" />}

      {/* Pet & Bubble Interaction Area */}
      <div className="flex w-full items-end gap-3 relative min-h-[220px]">
        <div className="shrink-0">
          <PetCharacter />
        </div>

        <div className="flex-1 w-full translate-y-3">
          <AnimatePresence mode="wait">
            {uiState === "idle" && (mode === "idle" || !mode) && (
              <InputBubble key="input-bubble" />
            )}

            {showStatus && uiState !== "done" && uiState !== "error" && (
              <RunningBubble key="running-bubble" />
            )}

            {(uiState === "done" || uiState === "error" || mode === "complete" || mode === "suggestion" || mode === "error") && (
              <ResultCard key="result-card" />
            )}
          </AnimatePresence>
        </div>
      </div>
    </motion.section>
  );
}
