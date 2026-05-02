import { AnimatePresence, motion } from "framer-motion";
import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../stores/useAppStore";
import type { AgentType } from "../types/agent";

type PetVisualState = "thinking" | "completed" | "error" | "waiting";

const PET_ASSET_BASE = "/pet";
const DEFAULT_GIF = `${PET_ASSET_BASE}/thinking/thinking_1.gif`;
const PET_GIF_MAP: Record<PetVisualState, string[]> = {
  thinking: [
    `${PET_ASSET_BASE}/thinking/thinking_1.gif`,
    `${PET_ASSET_BASE}/thinking/thinking_2.gif`,
  ],
  completed: [
    `${PET_ASSET_BASE}/complete/complete_1.gif`,
    `${PET_ASSET_BASE}/complete/complete_2.gif`,
  ],
  waiting: [
    `${PET_ASSET_BASE}/waiting/waiting_1.gif`,
    `${PET_ASSET_BASE}/waiting/waiting_2.gif`,
  ],
  error: [`${PET_ASSET_BASE}/error/error_1.gif`, `${PET_ASSET_BASE}/error/error_2.gif`],
};

function getVisualState(agentStatus: string): PetVisualState {
  if (agentStatus === "error") return "error";
  if (agentStatus === "waiting") return "waiting";
  if (agentStatus === "running") return "thinking";
  return "thinking";
}

function pickRandomGif(state: PetVisualState): string {
  const candidates = PET_GIF_MAP[state];
  if (!candidates.length) return DEFAULT_GIF;
  return candidates[Math.floor(Math.random() * candidates.length)];
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
    <div className="relative">
      <motion.button
        type="button"
        whileHover={{ y: -2, scale: 1.01 }}
        whileTap={{ scale: 0.985 }}
        onClick={() => setIsOpen((prev) => !prev)}
        className="rounded-2xl border border-white/20 bg-white/10 px-3 py-1.5 text-xs text-zinc-900 backdrop-blur-md transition-all duration-300 hover:-translate-y-0.5 hover:bg-white/20 hover:shadow-lg dark:text-zinc-100"
      >
        Agent · {selectedLabel}
      </motion.button>
      <AnimatePresence>
        {isOpen ? (
          <motion.div
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={{ duration: 0.2 }}
            className="absolute left-0 top-[calc(100%+8px)] z-30 min-w-36 rounded-2xl border border-white/20 bg-white/10 p-1.5 backdrop-blur-xl"
          >
            {options.map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() => { setSelectedAgent(option.value); setIsOpen(false); }}
                className={`block w-full rounded-xl px-3 py-2 text-left text-xs text-zinc-900 transition-all duration-300 hover:-translate-y-0.5 hover:bg-white/20 hover:shadow-md dark:text-zinc-100 ${
                  selectedAgent === option.value ? "bg-white/20 shadow-lg" : ""
                }`}
              >
                {option.label}
              </button>
            ))}
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
  );
}

function StatusLight(): JSX.Element {
  const uiState = useAppStore((s) => s.uiState);
  const color = useMemo(() => {
    if (uiState === "running") return "bg-emerald-400 shadow-emerald-400/70";
    if (uiState === "done") return "bg-sky-400 shadow-sky-400/70";
    if (uiState === "error") return "bg-rose-500 shadow-rose-500/75";
    if (uiState === "suggesting") return "bg-amber-400 shadow-amber-400/70";
    return "bg-zinc-400 shadow-zinc-400/60";
  }, [uiState]);

  return (
    <div className="flex items-center gap-2 rounded-full border border-zinc-300/70 bg-white/65 px-3 py-1 dark:border-zinc-700/70 dark:bg-zinc-900/55">
      <span className={`h-2.5 w-2.5 rounded-full shadow-[0_0_12px] ${color}`} />
      <span className="text-xs uppercase tracking-[0.18em] text-zinc-600 dark:text-zinc-300">
        {uiState}
      </span>
    </div>
  );
}

function TaskProgressSection(): JSX.Element {
  const percent = useAppStore((s) => s.percent);
  const bubble = useAppStore((s) => s.bubble);
  const logEntries = useAppStore((s) => s.logEntries);

  return (
    <section className="flex flex-col gap-3 rounded-2xl border border-zinc-300/50 bg-white/45 p-4 backdrop-blur-xl dark:border-zinc-700/70 dark:bg-zinc-900/35">
      <div className="space-y-3">
        <div className="flex items-center justify-between text-xs tracking-[0.18em] text-zinc-500 dark:text-zinc-400">
          <span>SESSION PROGRESS</span>
          <span>{Math.round(percent)}%</span>
        </div>
        <div className="h-2 overflow-hidden rounded-full bg-zinc-200/70 dark:bg-zinc-800/70">
          <motion.div
            initial={{ width: 0 }}
            animate={{ width: `${percent}%` }}
            transition={{ duration: 0.45, ease: "easeOut" }}
            className="h-full rounded-full bg-gradient-to-r from-zinc-500 via-zinc-700 to-zinc-900 dark:from-zinc-400 dark:via-zinc-300 dark:to-zinc-100"
          />
        </div>
      </div>

      {bubble ? (
        <div className="rounded-xl border border-zinc-300/55 bg-white/55 p-3 dark:border-zinc-700/70 dark:bg-zinc-900/40">
          <p className="mb-1 text-xs uppercase tracking-[0.2em] text-zinc-500 dark:text-zinc-400">
            STATUS
          </p>
          <p className="text-sm text-zinc-700 dark:text-zinc-200">{bubble}</p>
        </div>
      ) : null}

      {logEntries.length > 0 ? (
        <div className="log-panel">
          {logEntries.slice(-15).map((entry, i) => (
            <div key={`${i}-${entry.timestamp}`} className="log-line">
              {entry.message}
            </div>
          ))}
        </div>
      ) : null}
    </section>
  );
}

function PetPreviewPanel(): JSX.Element {
  const agentStatus = useAppStore((s) => s.agentStatus);
  const [currentGif, setCurrentGif] = useState<string>(DEFAULT_GIF);
  const visualState = getVisualState(agentStatus);

  useEffect(() => {
    setCurrentGif(pickRandomGif(visualState));
  }, [visualState]);

  return (
    <div className="relative flex h-[220px] w-[300px] items-end justify-start overflow-hidden">
      <motion.img
        key={currentGif}
        src={currentGif}
        alt={`宠物状态：${visualState}`}
        onError={() => { if (currentGif !== DEFAULT_GIF) setCurrentGif(DEFAULT_GIF); }}
        initial={{ opacity: 0, y: 10, scale: 0.98 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        transition={{ duration: 0.28, ease: "easeOut" }}
        className="h-full object-contain"
      />
    </div>
  );
}

export function MainView(): JSX.Element {
  const task = useAppStore((s) => s.task);
  const setTask = useAppStore((s) => s.setTask);
  const projectPath = useAppStore((s) => s.projectPath);
  const selectedAgent = useAppStore((s) => s.selectedAgent);
  const uiState = useAppStore((s) => s.uiState);
  const agentStatus = useAppStore((s) => s.agentStatus);

  const setSessionId = useAppStore((s) => s.setSessionId);
  const setUiState = useAppStore((s) => s.setUiState);
  const setLastStage = useAppStore((s) => s.setLastStage);
  const setBubble = useAppStore((s) => s.setBubble);
  const setPercent = useAppStore((s) => s.setPercent);
  const setResultZh = useAppStore((s) => s.setResultZh);
  const setSummaryText = useAppStore((s) => s.setSummaryText);
  const setEmotionText = useAppStore((s) => s.setEmotionText);
  const setSuggestion = useAppStore((s) => s.setSuggestion);
  const setAgentStatus = useAppStore((s) => s.setAgentStatus);
  const clearLogs = useAppStore((s) => s.clearLogs);
  const addLogEntry = useAppStore((s) => s.addLogEntry);

  const hasActiveTask = agentStatus !== "idle";

  const launch = useCallback(async () => {
    clearLogs();
    setResultZh("");
    setSummaryText("");
    setEmotionText("");
    setSuggestion("");
    setPercent(0);
    setUiState("running");
    setAgentStatus("running");
    setLastStage("init");
    setBubble("启动 Agent…");
    setSessionId(null);

    try {
      const res = await invoke<{ sessionId?: string }>("start_agent", {
        userInputZh: task,
        cwd: projectPath || ".",
        selectedAgent,
      });
      const sid = res?.sessionId ?? null;
      setSessionId(sid);
    } catch (err) {
      setUiState("error");
      setAgentStatus("error");
      setBubble(String(err));
      addLogEntry({ timestamp: Date.now(), level: "error", message: String(err) });
    }
  }, [task, projectPath, selectedAgent]);

  const stop = useCallback(async () => {
    try {
      await invoke("stop_agent", {});
      setBubble("已停止。");
      setUiState("idle");
      setAgentStatus("idle");
    } catch (err) {
      addLogEntry({ timestamp: Date.now(), level: "error", message: `stop: ${String(err)}` });
    }
  }, []);

  return (
    <motion.section
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.42, ease: "easeOut" }}
      className="mx-auto flex h-full w-full max-w-7xl flex-col gap-3 p-3"
    >
      {/* Top row: info + status */}
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <p className="mb-2 truncate text-xs text-zinc-700/85 dark:text-zinc-300/85">
            当前工作目录：{projectPath ?? "未选择目录"}
          </p>
          <AgentSelector />
        </div>
        <StatusLight />
      </div>

      {/* Task input */}
      <textarea
        className="task-input"
        value={task}
        onChange={(e) => setTask(e.target.value)}
        placeholder="用中文描述你想让 Agent 做的事…"
      />

      {/* Launch / Stop buttons */}
      <div className="flex gap-2">
        <motion.button
          whileHover={{ y: -2, scale: 1.01 }}
          whileTap={{ scale: 0.985 }}
          type="button"
          onClick={launch}
          disabled={uiState === "running"}
          className="rounded-xl border border-emerald-500/55 bg-emerald-500/28 px-4 py-2 text-xs font-medium text-zinc-900 backdrop-blur-md transition-all duration-300 hover:-translate-y-0.5 hover:bg-emerald-500/35 hover:shadow-lg disabled:cursor-not-allowed disabled:opacity-45 dark:border-emerald-300/50 dark:bg-emerald-500/35 dark:text-zinc-100"
        >
          {uiState === "running" ? "运行中…" : "启动 Agent"}
        </motion.button>
        {uiState === "running" ? (
          <motion.button
            whileHover={{ y: -2, scale: 1.01 }}
            whileTap={{ scale: 0.985 }}
            type="button"
            onClick={stop}
            className="rounded-xl border border-rose-500/55 bg-rose-500/20 px-4 py-2 text-xs font-medium text-rose-600 backdrop-blur-md transition-all duration-300 hover:-translate-y-0.5 hover:bg-rose-500/35 dark:text-rose-100"
          >
            停止 Agent
          </motion.button>
        ) : null}
      </div>

      {/* Progress + status + logs */}
      {hasActiveTask || uiState !== "idle" ? <TaskProgressSection /> : null}

      {/* Pet GIF */}
      <div className="relative min-h-[180px] flex-1">
        <div className="absolute left-0 bottom-0">
          <PetPreviewPanel />
        </div>
      </div>
    </motion.section>
  );
}
