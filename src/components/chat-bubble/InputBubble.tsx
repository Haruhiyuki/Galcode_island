import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../../stores/useAppStore";
import { useSettingsStore } from "../../stores/useSettingsStore";

const GREETINGS = [
  "喂，[称呼]，发什么呆呢？今天的部团活动要开始咯，有什么有趣的企划快交上来看看。",
  "真是的，让我等这么久。说吧，今天又有什么好玩的事情要做？",
  "就算是[称呼]，也得好好工作才行哦。有什么想做的，我们一起搞定吧！",
  "既然来了，就一起来找点乐子吧。有什么代码或者麻烦的任务需要我出马吗？",
  "[称呼]，今天有没有带来能让我眼前一亮的需求？普通的任务我可是会打哈欠的哦。",
];

export function InputBubble(): JSX.Element {
  const nickname = useSettingsStore((s) => s.nickname);
  const displayNickname = nickname.trim() ? nickname : "部员";
  
  const projectPath = useAppStore((s) => s.projectPath);
  const uiState = useAppStore((s) => s.uiState);
  const agentStatus = useAppStore((s) => s.agentStatus);
  const mode = useAppStore((s) => s.mode);
  const setUiState = useAppStore((s) => s.setUiState);
  const setMode = useAppStore((s) => s.setMode);
  const setAgentStatus = useAppStore((s) => s.setAgentStatus);
  const setSessionId = useAppStore((s) => s.setSessionId);
  const clearTodos = useAppStore((s) => s.clearTodos);
  const addLogEntry = useAppStore((s) => s.addLogEntry);

  const [greeting, setGreeting] = useState("");
  const [displayedGreeting, setDisplayedGreeting] = useState("");
  const task = useAppStore((s) => s.task);
  const setTask = useAppStore((s) => s.setTask);

  // Random greeting Selection
  useEffect(() => {
    if (agentStatus === "idle" || agentStatus === "completed") {
      const g = GREETINGS[Math.floor(Math.random() * GREETINGS.length)];
      setGreeting(g.replace(/\[称呼\]/g, displayNickname));
      setDisplayedGreeting("");
    }
  }, [agentStatus, displayNickname]);

  // Typewriter effect
  useEffect(() => {
    if (!greeting || (agentStatus !== "idle" && agentStatus !== "completed")) return;
    
    let currentIndex = 0;
    const intervalId = setInterval(() => {
      setDisplayedGreeting(greeting.substring(0, currentIndex + 1));
      currentIndex++;
      if (currentIndex >= greeting.length) {
        clearInterval(intervalId);
      }
    }, 40); // 40ms per character

    return () => clearInterval(intervalId);
  }, [greeting, agentStatus]);

  const showInput = uiState === "idle" && (mode === "idle" || !mode);

  const handleLaunch = async () => {
    if (!task.trim()) return;
    try {
      clearTodos();
      setUiState("running");
      setMode("working");
      setAgentStatus("running");
      const result = await invoke<{ sessionId: string; status: string }>("start_agent", {
        userInputZh: task,
        cwd: projectPath || ".",
      });
      setSessionId(result.sessionId);
    } catch (err) {
      addLogEntry({
        timestamp: Date.now(),
        level: "error",
        message: `launch err: ${String(err)}`,
      });
      setUiState("error");
      setMode("error");
      setAgentStatus("error");
    }
  };

  return (
    <AnimatePresence>
      {showInput && (
        <motion.div
          initial={{ opacity: 0, scale: 0.9, x: -10, y: 10 }}
          animate={{ opacity: 1, scale: 1, x: 0, y: 0 }}
          exit={{ opacity: 0, scale: 0.95, y: 10 }}
          transition={{ type: "spring", damping: 22, stiffness: 280 }}
          className="flex w-full flex-col overflow-hidden rounded-3xl rounded-bl-sm border border-zinc-200/60 bg-white/80 p-5 shadow-[0_8px_30px_rgb(0,0,0,0.06)] backdrop-blur-xl dark:border-zinc-700/50 dark:bg-zinc-800/80 dark:shadow-[0_8px_30px_rgb(0,0,0,0.2)]"
        >
          <div className="mb-4 min-h-[3rem] text-[15px] font-medium leading-relaxed tracking-wide text-zinc-700 dark:text-zinc-200">
            {displayedGreeting}
            {displayedGreeting.length < greeting.length && (
              <motion.span
                animate={{ opacity: [1, 0] }}
                transition={{ repeat: Infinity, duration: 0.8 }}
                className="ml-1 inline-block h-[15px] w-2 bg-blue-500/70 align-middle"
              />
            )}
          </div>

          <textarea
            value={task}
            onChange={(e) => setTask(e.target.value)}
            placeholder="和团长对话……"
            className="min-h-[100px] w-full resize-none rounded-xl border border-zinc-200/70 bg-zinc-50/50 p-3.5 text-sm text-zinc-800 outline-none transition-colors placeholder:text-zinc-400 focus:border-blue-500/50 focus:bg-white/80 dark:border-zinc-700/60 dark:bg-zinc-900/40 dark:text-zinc-100 dark:placeholder:text-zinc-500 dark:focus:border-blue-500/40 dark:focus:bg-zinc-900/70"
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                handleLaunch();
              }
            }}
          />

          <div className="mt-4 flex justify-end">
            <motion.button
              whileHover={{ scale: 1.02, y: -1 }}
              whileTap={{ scale: 0.98 }}
              onClick={handleLaunch}
              disabled={!task.trim()}
              className="rounded-xl bg-blue-500 px-6 py-2.5 text-sm font-semibold tracking-wide text-white shadow-md shadow-blue-500/25 transition-all hover:bg-blue-600 hover:shadow-blue-500/40 disabled:cursor-not-allowed disabled:opacity-50"
            >
              启动
            </motion.button>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}