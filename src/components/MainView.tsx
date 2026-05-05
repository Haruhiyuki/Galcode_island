import { AnimatePresence, motion } from "framer-motion";
import { useState } from "react";
import { useAppStore } from "../stores/useAppStore";
import { useActiveTab, useActiveTabActions } from "../hooks/useActiveTab";
import { PetCharacter } from "./pet-character/PetCharacter";
import type { AgentType } from "../types/agent";

import { InputBubble } from "./chat-bubble/InputBubble";
import { ResultCard } from "./chat-bubble/ResultCard";
import { RunningBubble } from "./chat-bubble/RunningBubble";
import { StatusMonitor } from "./status-monitor/StatusMonitor";

/// 切换当前 tab 用的 backend。
/// 同时同步到 useAppStore.selectedAgent 作为下次新建 tab 的默认值。
function AgentSelector(): JSX.Element {
  const tab = useActiveTab();
  const { update } = useActiveTabActions();
  const setSelectedAgent = useAppStore((s) => s.setSelectedAgent);
  const [isOpen, setIsOpen] = useState(false);

  const options: { value: AgentType; label: string }[] = [
    { value: "claude-code", label: "Claude Code" },
    { value: "opencode", label: "OpenCode" },
    { value: "codex", label: "Codex" },
  ];
  const selectedLabel = options.find((o) => o.value === tab.agent)?.label ?? "Claude Code";

  return (
    <div className="relative inline-block text-left">
      <button
        onClick={() => setIsOpen((p) => !p)}
        className="inline-flex items-center gap-1 rounded-md border border-white/40 bg-white/50 px-2 py-0.5 text-[11px] font-medium text-zinc-700 shadow-sm backdrop-blur-md transition-all hover:bg-white/70 dark:border-white/10 dark:bg-zinc-800/50 dark:text-zinc-200 dark:hover:bg-zinc-800/70"
      >
        {selectedLabel}
        <svg className="h-3 w-3 text-zinc-400" viewBox="0 0 20 20" fill="currentColor">
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
            className="absolute left-0 z-10 mt-1 w-32 origin-top-left overflow-hidden rounded-md border border-white/40 bg-white/70 p-0.5 shadow-[0_8px_30px_rgba(0,0,0,0.06)] backdrop-blur-xl dark:border-white/10 dark:bg-zinc-800/80 dark:shadow-[0_8px_30px_rgba(0,0,0,0.2)]"
          >
            <div className="flex flex-col gap-0.5">
              {options.map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => {
                    update({ agent: opt.value });
                    setSelectedAgent(opt.value);
                    setIsOpen(false);
                  }}
                  className="block w-full rounded px-2 py-1 text-left text-[11px] font-medium text-zinc-700 transition-colors hover:bg-zinc-100/80 dark:text-zinc-200 dark:hover:bg-zinc-700/50"
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
  const agentStatus = useActiveTab().agentStatus;
  const isRunning = agentStatus === "running" || agentStatus === "thinking" || agentStatus === "processing";
  const isError = agentStatus === "error";
  const bg = isRunning ? "bg-sky-400" : isError ? "bg-rose-400" : "bg-emerald-400";
  const shadow = isRunning ? "shadow-[0_0_6px_rgba(56,189,248,0.5)]" : isError ? "shadow-[0_0_5px_rgba(251,113,133,0.4)]" : "shadow-[0_0_5px_rgba(52,211,153,0.4)]";

  return (
    <div className="flex items-center gap-1.5">
      <div className={`h-2 w-2 rounded-full ${bg} ${shadow} ${isRunning ? "animate-pulse" : ""}`} />
      <span className="text-[10px] uppercase tracking-wider text-zinc-400 dark:text-zinc-500 font-medium">
        {agentStatus}
      </span>
    </div>
  );
}

export function MainView(): JSX.Element {
  const tab = useActiveTab();
  const projectPath = tab.projectPath;
  const uiState = tab.uiState;
  const mode = tab.mode;
  const cliBlockCount = tab.cliBlocks.length;
  // 完成后保留 StatusMonitor 让 BlockStream 历史可见，跟 ResultCard 共存。
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
      className="mx-auto flex h-full w-full max-w-7xl flex-col gap-3 px-4 py-3"
    >
      {/* Top Header — 单行紧凑：Agent 选择 + 工程路径 + 状态灯 */}
      <div className="flex items-center justify-between gap-3 border-b border-black/5 pb-1.5 dark:border-white/5">
        <div className="flex min-w-0 items-center gap-2">
          <AgentSelector />
          <span className="truncate text-[11px] text-zinc-500/85 dark:text-zinc-400/85">
            {projectPath ?? "未选择工程"}
          </span>
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
