import { useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { useAppStore } from "../../stores/useAppStore";
import type { AgentType } from "../../types/agent";

const agentOptions: { value: AgentType; label: string }[] = [
  { value: "claude-code", label: "Claude Code" },
  { value: "opencode", label: "OpenCode" },
  { value: "codex", label: "Codex" },
];

export function AgentSelector(): JSX.Element {
  const selectedAgent = useAppStore((s) => s.selectedAgent);
  const setSelectedAgent = useAppStore((s) => s.setSelectedAgent);
  const [isOpen, setIsOpen] = useState(false);

  const selectedLabel =
    agentOptions.find((o) => o.value === selectedAgent)?.label ?? "Claude Code";

  return (
    <div className="relative">
      <motion.button
        type="button"
        whileHover={{ y: -2, scale: 1.01 }}
        whileTap={{ scale: 0.985 }}
        onClick={() => setIsOpen((prev) => !prev)}
        className="rounded-2xl border border-white/20 bg-white/10 px-3 py-1.5 text-xs font-medium text-zinc-900 backdrop-blur-md transition-all duration-300 hover:-translate-y-0.5 hover:bg-white/20 hover:shadow-lg dark:text-zinc-100"
        aria-haspopup="listbox"
        aria-expanded={isOpen}
      >
        Agent · {selectedLabel}
      </motion.button>

      <AnimatePresence>
        {isOpen ? (
          <motion.ul
            initial={{ opacity: 0, y: -6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -6 }}
            transition={{ duration: 0.2 }}
            className="absolute left-0 top-[calc(100%+8px)] z-20 min-w-40 rounded-2xl border border-white/20 bg-white/10 p-1.5 backdrop-blur-xl"
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
                    setSelectedAgent(option.value);
                    setIsOpen(false);
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
  );
}
