import { AnimatePresence, motion } from "framer-motion";
import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../stores/useAppStore";
import type { AgentType } from "../types/agent";

import { InputBubble } from "./chat-bubble/InputBubble";
import { ResultCard } from "./chat-bubble/ResultCard";
import { RunningBubble } from "./chat-bubble/RunningBubble";
import { StatusMonitor } from "./status-monitor/StatusMonitor";

type PetVisualState = "thinking" | "complete" | "error" | "waiting";

const PET_ASSET_BASE = "pet";

function getVisualState(uiState: string, mode: string): PetVisualState {
  if (uiState === "error" || mode === "error") return "error";
  if (uiState === "done" || mode === "complete") return "complete";
  if (uiState === "running" || mode === "thinking" || mode === "working") return "thinking";
  if (uiState === "idle" && (mode === "idle" || !mode)) return "thinking";
  return "waiting";
}

function pickRandomGif(state: PetVisualState): string {
  const maxMap: Record<PetVisualState, number> = {
    thinking: 2,
    complete: 3,
    waiting: 2,
    error: 2,
  };
  const max = maxMap[state] || 1;
  const n = Math.floor(Math.random() * max) + 1;
  return `/${PET_ASSET_BASE}/${state}/${state}_${n}.gif`;
}

function AgentSelector(): JSX.Element {
  const selectedAgent = useAppStore((s) => s.selectedAgent);
  const setSelectedAgent = useAppStore((s) => s.setSelectedAgent);
  const [isOpen, setIsOpen] = useState(false);

  const options: { value: AgentType; label: string }[] = [
    { value: "claude-code", label: "Claude Code" },
    { value: "opencode", label: "OpenCode" },
  ];
  const selectedLabel = options.find((o) => o.value === selectedAgent)?.label ?? "Claude Code";

  return (
    <div className="relative inline-block text-left">
      <button
        onClick={() => setIsOpen((p) => !p)}
        className="inline-flex w-full items-center justify-between gap-x-2 rounded-xl border border-zinc-200/70 bg-white/50 px-3 py-1.5 text-sm font-semibold text-zinc-700 shadow-sm backdrop-blur-md transition-all hover:bg-white/80 hover:shadow-md dark:border-zinc-700/50 dark:bg-zinc-800/50 dark:text-zinc-200 dark:hover:bg-zinc-800/80"
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
            className="absolute left-0 z-10 mt-2 w-40 origin-top-left overflow-hidden rounded-xl border border-zinc-200/60 bg-white/80 p-1 shadow-[0_8px_30px_rgb(0,0,0,0.08)] backdrop-blur-xl focus:outline-none dark:border-zinc-700/50 dark:bg-zinc-800/90 dark:shadow-[0_8px_30px_rgb(0,0,0,0.2)]"
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
  const bg = isRunning ? "bg-amber-400" : agentStatus === "error" ? "bg-rose-500" : "bg-emerald-400";
  const shadow = isRunning ? "shadow-[0_0_8px_rgba(251,191,36,0.8)]" : agentStatus === "error" ? "shadow-[0_0_8px_rgba(244,63,94,0.8)]" : "shadow-[0_0_8px_rgba(52,211,153,0.8)]";

  return (
    <div className="flex flex-col items-center justify-center gap-1">
      <div className={`h-3 w-3 rounded-full ${bg} ${shadow} ${isRunning ? "animate-pulse" : ""}`} />
      <span className="text-[10px] uppercase tracking-wider text-zinc-500 dark:text-zinc-400 font-medium">
        {agentStatus}
      </span>
    </div>
  );
}

function PetPreviewPanel(): JSX.Element {
  const uiState = useAppStore((s) => s.uiState);
  const mode = useAppStore((s) => s.mode);
  
  const [currentGif, setCurrentGif] = useState<string>(() => pickRandomGif(getVisualState(uiState, mode)));

  useEffect(() => {
    setCurrentGif(pickRandomGif(getVisualState(uiState, mode)));
  }, [uiState, mode]);

  return (
    <div className="relative flex h-[260px] w-[320px] shrink-0 items-end justify-start overflow-hidden">
      <AnimatePresence mode="popLayout">
        <motion.img
          key={currentGif}
          src={currentGif}
          alt={`宠物状态`}
          initial={{ opacity: 0, x: -20 }}
          animate={{ opacity: 1, x: 0 }}
          exit={{ opacity: 0, x: 20 }}
          transition={{ duration: 0.4, ease: "easeOut" }}
          className="h-full object-contain pointer-events-none drop-shadow-xl"
        />
      </AnimatePresence>
    </div>
  );
}

export function MainView(): JSX.Element {
  const projectPath = useAppStore((s) => s.projectPath);
  const uiState = useAppStore((s) => s.uiState);
  const mode = useAppStore((s) => s.mode);

  return (
    <motion.section
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -10 }}
      transition={{ duration: 0.42, ease: "easeOut" }}
      className="mx-auto flex h-full w-full max-w-7xl flex-col gap-4 p-4"
    >
      {/* Top Header */}
      <div className="flex items-start justify-between gap-4 border-b border-zinc-200 pb-3 dark:border-zinc-800">
        <div className="min-w-0">
          <p className="mb-2 truncate text-xs font-medium text-zinc-700/85 dark:text-zinc-300/85">
            当前工程：{projectPath ?? "未选择"}
          </p>
          <AgentSelector />
        </div>
        <StatusLight />
      </div>

      {/* Main content dynamically switches based on uiState and mode */}
      {/* Status Monitor Section (visible when running/thinking/working) */}
      <AnimatePresence mode="popLayout">
        {(uiState === "running" || mode === "working" || mode === "thinking") && (
          <motion.div
            key="status-monitor"
            initial={{ opacity: 0, height: 0, scale: 0.98 }}
            animate={{ opacity: 1, height: "auto", scale: 1 }}
            exit={{ opacity: 0, height: 0, scale: 0.98 }}
            transition={{ duration: 0.3 }}
            className="flex-1 min-h-0 overflow-y-auto overflow-x-hidden"
          >
            <StatusMonitor />
          </motion.div>
        )}
      </AnimatePresence>

      <div className="flex-1" />

      {/* Pet & Bubble Interaction Area */}
      <div className="flex w-full items-end gap-1 relative min-h-[220px]">
        <PetPreviewPanel />

        <div className="flex-1 w-full translate-y-3 -ml-4">
          <AnimatePresence mode="wait">
            {(uiState === "idle" && (mode === "idle" || !mode)) && (
              <InputBubble key="input-bubble" />
            )}

            {(uiState === "running" || mode === "thinking" || mode === "working") && (
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

