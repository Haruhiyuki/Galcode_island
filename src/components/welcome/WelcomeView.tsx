import { open } from "@tauri-apps/plugin-dialog";
import { AnimatePresence, motion } from "framer-motion";
import { useMemo, useState } from "react";
import { useAppStore } from "../../stores/useAppStore";
import type { AgentType } from "../../types/agent";

const agentOptions = [
  { value: "claude-code", label: "Claude Code" },
  { value: "opencode", label: "OpenCode" },
] as const;

export function WelcomeView(): JSX.Element {
  const projectPath = useAppStore((state) => state.projectPath);
  const selectedAgent = useAppStore((state) => state.selectedAgent);
  const setProjectPath = useAppStore((state) => state.setProjectPath);
  const setSelectedAgent = useAppStore((state) => state.setSelectedAgent);
  const setIsStarted = useAppStore((state) => state.setIsStarted);

  const [isSelecting, setIsSelecting] = useState(false);
  const [isAgentMenuOpen, setIsAgentMenuOpen] = useState(false);

  const actionButtonLabel = useMemo(
    () => (projectPath ? "启动" : "选择项目"),
    [projectPath],
  );
  const selectedAgentLabel = useMemo(
    () =>
      agentOptions.find((option) => option.value === selectedAgent)?.label ??
      "Claude Code",
    [selectedAgent],
  );

  const pickProjectFolder = async (): Promise<void> => {
    setIsSelecting(true);
    const result = await open({ directory: true });
    setIsSelecting(false);

    if (!result) {
      return;
    }

    const path = Array.isArray(result) ? result[0] : result;
    setProjectPath(path);
  };

  const handlePrimaryAction = async (): Promise<void> => {
    if (!projectPath) {
      await pickProjectFolder();
      return;
    }
    setIsStarted(true);
  };

  return (
    <section className="relative mx-auto flex h-full w-full max-w-6xl flex-col overflow-hidden rounded-[28px] border border-zinc-300/45 bg-gradient-to-br from-white/72 via-white/38 to-zinc-200/25 px-10 pb-0 shadow-[0_30px_90px_rgba(0,0,0,0.18)] backdrop-blur-2xl dark:border-zinc-700/55 dark:from-zinc-900/70 dark:via-zinc-900/45 dark:to-zinc-950/28">
      <div data-tauri-drag-region className="h-[30px] w-full shrink-0" />

      <motion.div
        initial={{ opacity: 0, y: 14 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.55, ease: "easeOut" }}
        className="flex flex-1 flex-col items-center justify-center gap-8"
      >
        <motion.h1
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 1.5, ease: "easeOut" }}
          className="text-center text-5xl font-semibold tracking-wide text-zinc-900 dark:text-zinc-100"
        >
          Galcode Island
        </motion.h1>

        <div className="flex items-center gap-3">
          <motion.button
            whileHover={{ y: -2, scale: 1.01 }}
            whileTap={{ scale: 0.985 }}
            type="button"
            onClick={handlePrimaryAction}
            disabled={isSelecting}
            className={`min-w-36 border px-5 py-2.5 text-sm font-medium backdrop-blur-md transition-all duration-300 hover:-translate-y-0.5 hover:bg-white/20 hover:shadow-lg disabled:cursor-not-allowed disabled:opacity-70 ${
              projectPath
                ? "rounded-2xl border-emerald-500/55 bg-emerald-500/28 text-zinc-900 shadow-emerald-500/20 dark:border-emerald-300/50 dark:bg-emerald-500/35 dark:text-zinc-100"
                : "rounded-xl border-white/20 bg-white/10 text-zinc-900 dark:text-zinc-100"
            }`}
          >
            {isSelecting ? "选择中..." : actionButtonLabel}
          </motion.button>

          <div className="relative">
            <motion.button
              type="button"
              whileHover={{ y: -2, scale: 1.01 }}
              whileTap={{ scale: 0.985 }}
              onClick={() => setIsAgentMenuOpen((prev) => !prev)}
              className="min-w-40 rounded-2xl border border-white/20 bg-white/10 px-4 py-2.5 text-sm text-zinc-900 backdrop-blur-md transition-all duration-300 hover:-translate-y-0.5 hover:bg-white/20 hover:shadow-lg dark:text-zinc-100"
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
                  className="absolute left-0 top-[calc(100%+8px)] z-20 w-full rounded-2xl border border-white/20 bg-white/10 p-1.5 backdrop-blur-md"
                  role="listbox"
                >
                  {agentOptions.map((option) => (
                    <li key={option.value}>
                      <button
                        type="button"
                        className={`w-full rounded-xl px-3 py-2 text-left text-sm text-zinc-900 transition-all duration-300 hover:-translate-y-0.5 hover:bg-white/20 hover:shadow-md dark:text-zinc-100 ${
                          selectedAgent === option.value ? "bg-white/20 shadow-lg" : ""
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
            <span className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-xs text-zinc-700 dark:text-zinc-300">
              ▾
            </span>
          </div>
        </div>

        <AnimatePresence>
          {projectPath ? (
            <motion.p
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              className="max-w-2xl rounded-xl border border-white/20 bg-white/10 px-3 py-2 text-center text-xs text-zinc-900 backdrop-blur-md dark:text-zinc-100"
            >
              当前项目：{projectPath}
            </motion.p>
          ) : null}
        </AnimatePresence>
      </motion.div>

      <div className="flex h-52 w-full items-end justify-center overflow-hidden">
        <motion.img
          src="/pet/welcome/welcome.gif"
          alt="欢迎动图"
          initial={{ opacity: 0.55, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5, ease: "easeOut" }}
          className="h-56 object-contain"
        />
      </div>
    </section>
  );
}
