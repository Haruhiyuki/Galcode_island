import { open } from "@tauri-apps/plugin-dialog";
import { AnimatePresence, motion } from "framer-motion";
import { useCallback, useState } from "react";
import { useAppStore } from "../../stores/useAppStore";
import { useTabsStore } from "../../stores/useTabsStore";
import { PetCharacter } from "../pet-character/PetCharacter";
import type { AgentType } from "../../types/agent";

function deriveTabTitleFromPath(path: string | null): string {
  if (!path) return "新会话";
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] || path;
}

const agentOptions = [
  { value: "claude-code", label: "Claude Code" },
  { value: "opencode", label: "OpenCode" },
  { value: "codex", label: "Codex" },
] as const;

const titleChars = "Galcode Island".split("");

const titleVariants = {
  hidden: {},
  visible: {
    transition: {
      staggerChildren: 0.06,
      delayChildren: 0.3,
    },
  },
};

const charVariants = {
  hidden: { opacity: 0, y: 18, rotate: -8, filter: "blur(4px)" },
  visible: {
    opacity: 1,
    y: 0,
    rotate: 0,
    filter: "blur(0px)",
    transition: { type: "spring" as const, damping: 12, stiffness: 200, mass: 0.6 },
  },
};

export function WelcomeView(): JSX.Element {
  // WelcomeView 用 useState 暂存用户选的项目路径 + 默认 agent；
  // 点"启动"时才创建第一个 tab（把这两个值塞进 TabState），然后切到主界面。
  // 这样 useAppStore 不再需要全局 projectPath 字段。
  const selectedAgent = useAppStore((s) => s.selectedAgent);
  const setSelectedAgent = useAppStore((s) => s.setSelectedAgent);
  const setIsStarted = useAppStore((s) => s.setIsStarted);
  const createTab = useTabsStore((s) => s.createTab);
  const setActiveTab = useTabsStore((s) => s.setActiveTab);

  const [pendingProjectPath, setPendingProjectPath] = useState<string | null>(null);
  const [isSelecting, setIsSelecting] = useState(false);
  const [isAgentMenuOpen, setIsAgentMenuOpen] = useState(false);

  const projectPath = pendingProjectPath;

  const selectedAgentLabel =
    agentOptions.find((o) => o.value === selectedAgent)?.label ?? "Claude Code";

  const pickProjectFolder = useCallback(async (): Promise<void> => {
    setIsSelecting(true);
    try {
      const result = await open({ directory: true });
      if (!result) return;
      const path = Array.isArray(result) ? result[0] : result;
      setPendingProjectPath(path);
    } finally {
      setIsSelecting(false);
    }
  }, []);

  const handlePrimaryAction = useCallback(async (): Promise<void> => {
    if (!projectPath) {
      await pickProjectFolder();
      return;
    }
    const id = createTab({
      title: deriveTabTitleFromPath(projectPath),
      agent: selectedAgent,
      projectPath,
    });
    setActiveTab(id);
    setIsStarted(true);
  }, [projectPath, pickProjectFolder, createTab, setActiveTab, selectedAgent, setIsStarted]);

  const actionLabel = projectPath ? "启动" : "选择项目";
  const isReady = Boolean(projectPath);

  return (
    <section className="relative mx-auto flex h-full w-full max-w-6xl flex-col overflow-hidden rounded-[28px] border border-white/60 bg-white/70 px-10 pb-0 shadow-[0_20px_60px_rgba(0,0,0,0.06)] backdrop-blur-2xl dark:border-white/10 dark:bg-slate-800/60 dark:shadow-none">
      <div data-tauri-drag-region className="h-[30px] w-full shrink-0" />

      <motion.div
        initial={{ opacity: 0, y: 14 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.55, ease: "easeOut" }}
        className="flex flex-1 flex-col items-center justify-center gap-8"
      >
        {/* Apple-style handwriting reveal title */}
        <motion.h1
          variants={titleVariants}
          initial="hidden"
          animate="visible"
          className="flex flex-wrap justify-center gap-x-[0.05em] text-center text-5xl font-bold italic tracking-tight text-zinc-800 dark:text-zinc-100 font-serif"
          style={{ fontVariationSettings: "'wght' 700" }}
        >
          {titleChars.map((char, i) => (
            <motion.span
              key={`${char}-${i}`}
              variants={charVariants}
              className="inline-block bg-gradient-to-br from-amber-400 via-orange-500 to-amber-500 bg-clip-text text-transparent dark:from-amber-300 dark:via-orange-400 dark:to-amber-400"
              style={{ minWidth: char === " " ? "0.3em" : undefined }}
            >
              {char === " " ? " " : char}
            </motion.span>
          ))}
        </motion.h1>

        <div className="flex items-center gap-3">
          <motion.button
            whileHover={{ y: -2, scale: 1.01 }}
            whileTap={{ scale: 0.985 }}
            type="button"
            onClick={handlePrimaryAction}
            disabled={isSelecting}
            className={`min-w-36 border px-5 py-2.5 text-sm font-medium backdrop-blur-md transition-all duration-300 hover:-translate-y-0.5 hover:shadow-lg disabled:cursor-not-allowed disabled:opacity-70 ${
              isReady
                ? "rounded-2xl border-sky-400/50 bg-sky-400/25 text-zinc-900 shadow-sky-400/15 dark:border-sky-400/40 dark:bg-sky-500/30 dark:text-zinc-100"
                : "rounded-xl border-white/30 bg-white/20 text-zinc-800 dark:border-white/15 dark:bg-white/10 dark:text-zinc-100"
            }`}
          >
            {isSelecting ? "选择中..." : actionLabel}
          </motion.button>

          <div className="relative">
            <motion.button
              type="button"
              whileHover={{ y: -2, scale: 1.01 }}
              whileTap={{ scale: 0.985 }}
              onClick={() => setIsAgentMenuOpen((prev) => !prev)}
              className="min-w-40 rounded-2xl border border-white/30 bg-white/20 px-4 py-2.5 text-sm text-zinc-800 backdrop-blur-md transition-all duration-300 hover:-translate-y-0.5 hover:bg-white/30 hover:shadow-lg dark:border-white/15 dark:bg-white/10 dark:text-zinc-100"
              aria-haspopup="listbox"
              aria-expanded={isAgentMenuOpen}
            >
              Agent · {selectedAgentLabel}
            </motion.button>

            <AnimatePresence>
              {isAgentMenuOpen ? (
                <motion.ul
                  initial={{ opacity: 0, y: -6 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, y: -6 }}
                  transition={{ duration: 0.2 }}
                  className="absolute left-0 top-[calc(100%+8px)] z-20 w-full rounded-2xl border border-white/60 bg-white/70 p-1.5 shadow-lg backdrop-blur-xl dark:border-white/10 dark:bg-slate-800/60"
                  role="listbox"
                >
                  {agentOptions.map((option) => (
                    <li key={option.value}>
                      <button
                        type="button"
                        className={`w-full rounded-xl px-3 py-2 text-left text-sm text-zinc-800 transition-all duration-300 hover:-translate-y-0.5 hover:bg-white/40 hover:shadow-md dark:text-zinc-100 dark:hover:bg-white/10 ${
                          selectedAgent === option.value ? "bg-white/40 shadow-md dark:bg-white/10" : ""
                        }`}
                        onClick={() => {
                          setSelectedAgent(option.value as AgentType);
                          setIsAgentMenuOpen(false);
                        }}
                      >
                        {option.label}
                      </button>
                    </li>
                  ))}
                </motion.ul>
              ) : null}
            </AnimatePresence>
          </div>
        </div>

        <AnimatePresence>
          {projectPath ? (
            <motion.p
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              className="max-w-2xl rounded-xl border border-white/60 bg-white/70 px-3 py-2 text-center text-xs text-zinc-700 backdrop-blur-md dark:border-white/10 dark:bg-slate-800/60 dark:text-zinc-300"
            >
              当前项目：{projectPath}
            </motion.p>
          ) : null}
        </AnimatePresence>
      </motion.div>

      {/* Interactive pet character at bottom */}
      <div className="flex h-56 w-full items-end justify-center pb-2">
        <PetCharacter />
      </div>
    </section>
  );
}
