import { AnimatePresence, motion } from "framer-motion";
import { useEffect, useMemo, useState } from "react";
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

const mockTodos = [
  "读取项目目录",
  "初始化 IPC 监听",
  "等待 Agent 首次反馈",
];

function getVisualState(agentStatus: string): PetVisualState {
  if (agentStatus === "error") {
    return "error";
  }
  if (agentStatus === "waiting") {
    return "waiting";
  }
  if (agentStatus === "running") {
    return "thinking";
  }
  return "thinking";
}

function pickRandomGif(state: PetVisualState): string {
  const candidates = PET_GIF_MAP[state];
  if (!candidates.length) {
    return DEFAULT_GIF;
  }
  const randomIndex = Math.floor(Math.random() * candidates.length);
  return candidates[randomIndex];
}

function AgentSelector(): JSX.Element {
  const selectedAgent = useAppStore((state) => state.selectedAgent);
  const setSelectedAgent = useAppStore((state) => state.setSelectedAgent);
  const [isOpen, setIsOpen] = useState(false);

  const options: { value: AgentType; label: string }[] = [
    { key: "claude-code", label: "Claude Code" },
    { key: "opencode", label: "OpenCode" },
  ].map((item) => ({ value: item.key as AgentType, label: item.label }));

  const selectedLabel =
    options.find((option) => option.value === selectedAgent)?.label ?? "Claude Code";

  return (
    <div className="relative">
      <motion.button
        type="button"
        whileHover={{ y: -2, scale: 1.01 }}
        whileTap={{ scale: 0.985 }}
        onClick={() => setIsOpen((prev) => !prev)}
        className="rounded-2xl border border-white/20 bg-white/10 px-4 py-2 text-sm text-zinc-900 backdrop-blur-md transition-all duration-300 hover:-translate-y-0.5 hover:bg-white/20 hover:shadow-lg dark:text-zinc-100"
      >
        {`> ${selectedLabel}`}
      </motion.button>
      <AnimatePresence>
        {isOpen ? (
          <motion.div
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={{ duration: 0.2 }}
            className="absolute left-0 top-[calc(100%+8px)] z-30 min-w-44 rounded-2xl border border-white/20 bg-white/10 p-1.5 backdrop-blur-xl"
          >
            {options.map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() => {
                  setSelectedAgent(option.value);
                  setIsOpen(false);
                }}
                className={`block w-full rounded-xl px-3 py-2 text-left text-sm text-zinc-900 transition-all duration-300 hover:-translate-y-0.5 hover:bg-white/20 hover:shadow-md dark:text-zinc-100 ${
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
  const agentStatus = useAppStore((state) => state.agentStatus);
  const color = useMemo(() => {
    if (agentStatus === "running") {
      return "bg-emerald-400 shadow-emerald-400/70";
    }
    if (agentStatus === "waiting") {
      return "bg-amber-400 shadow-amber-400/70";
    }
    if (agentStatus === "error") {
      return "bg-rose-500 shadow-rose-500/75";
    }
    return "bg-zinc-400 shadow-zinc-400/60";
  }, [agentStatus]);

  return (
    <div className="flex items-center gap-2 rounded-full border border-zinc-300/70 bg-white/65 px-3 py-1 dark:border-zinc-700/70 dark:bg-zinc-900/55">
      <span className={`h-2.5 w-2.5 rounded-full shadow-[0_0_12px] ${color}`} />
      <span className="text-xs uppercase tracking-[0.18em] text-zinc-600 dark:text-zinc-300">
        {agentStatus}
      </span>
    </div>
  );
}

function TaskProgressSection({ progressValue }: { progressValue: number }): JSX.Element {
  return (
    <section className="grid grid-rows-[auto_1fr] gap-4 rounded-2xl border border-zinc-300/50 bg-white/45 p-4 backdrop-blur-xl dark:border-zinc-700/70 dark:bg-zinc-900/35">
      <div className="space-y-3">
        <div className="flex items-center justify-between text-xs tracking-[0.18em] text-zinc-500 dark:text-zinc-400">
          <span>SESSION PROGRESS</span>
          <span>{progressValue}%</span>
        </div>
        <div className="h-2 overflow-hidden rounded-full bg-zinc-200/70 dark:bg-zinc-800/70">
          <motion.div
            initial={{ width: 0 }}
            animate={{ width: `${progressValue}%` }}
            transition={{ duration: 0.45, ease: "easeOut" }}
            className="h-full rounded-full bg-gradient-to-r from-zinc-500 via-zinc-700 to-zinc-900 dark:from-zinc-400 dark:via-zinc-300 dark:to-zinc-100"
          />
        </div>
      </div>

      <div className="mt-2 rounded-xl border border-zinc-300/55 bg-white/55 p-4 dark:border-zinc-700/70 dark:bg-zinc-900/40">
        <p className="mb-3 text-xs uppercase tracking-[0.2em] text-zinc-500 dark:text-zinc-400">
          Todo Stream (Mock)
        </p>
        <ul className="space-y-2 text-sm text-zinc-700 dark:text-zinc-200">
          {mockTodos.map((todo, index) => (
            <li key={todo} className="flex items-center gap-2">
              <span className="h-1.5 w-1.5 rounded-full bg-zinc-500 dark:bg-zinc-300" />
              <span>
                {index + 1}. {todo}
              </span>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function IdleBlankSection(): JSX.Element {
  return <div className="flex-1" />;
}

function PetPreviewPanel(): JSX.Element {
  const agentStatus = useAppStore((state) => state.agentStatus);
  const [currentGif, setCurrentGif] = useState<string>(DEFAULT_GIF);
  const visualState = getVisualState(agentStatus);

  useEffect(() => {
    setCurrentGif(pickRandomGif(visualState));
  }, [visualState]);

  return (
    <div className="relative flex h-[240px] w-[320px] items-end justify-start overflow-hidden">
      <motion.img
        key={currentGif}
        src={currentGif}
        alt={`宠物状态：${visualState}`}
        onError={() => {
          if (currentGif !== DEFAULT_GIF) {
            setCurrentGif(DEFAULT_GIF);
          }
        }}
        initial={{ opacity: 0, y: 10, scale: 0.98 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        transition={{ duration: 0.28, ease: "easeOut" }}
        className="h-full object-contain"
      />
    </div>
  );
}

export function MainView(): JSX.Element {
  const agentStatus = useAppStore((state) => state.agentStatus);
  const projectPath = useAppStore((state) => state.projectPath);
  const hasActiveTask = agentStatus !== "idle";
  const progressValue = useMemo(() => {
    if (agentStatus === "running") {
      return 68;
    }
    if (agentStatus === "waiting") {
      return 82;
    }
    if (agentStatus === "error") {
      return 28;
    }
    return 15;
  }, [agentStatus]);

  return (
    <motion.section
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.42, ease: "easeOut" }}
      className="mx-auto grid h-full w-full max-w-7xl gap-4 p-4"
    >
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0">
          <p className="mb-2 truncate text-xs text-zinc-700/85 dark:text-zinc-300/85">
            当前工作目录：{projectPath ?? "未选择目录"}
          </p>
          <AgentSelector />
        </div>
        <StatusLight />
      </div>

      {hasActiveTask ? (
        <TaskProgressSection progressValue={progressValue} />
      ) : (
        <IdleBlankSection />
      )}

      <div className="relative min-h-[240px]">
        <div className="absolute left-0 bottom-0">
          <PetPreviewPanel />
        </div>
      </div>
    </motion.section>
  );
}
