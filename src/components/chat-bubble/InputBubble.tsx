import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../../stores/useAppStore";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { useActiveTab, useActiveTabActions } from "../../hooks/useActiveTab";

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
  const addLogEntry = useAppStore((s) => s.addLogEntry);

  const tab = useActiveTab();
  const { activeTabId, update } = useActiveTabActions();

  const projectPath = tab.projectPath;
  const agentStatus = tab.agentStatus;
  const task = tab.task;

  const [greeting, setGreeting] = useState("");
  const [displayedGreeting, setDisplayedGreeting] = useState("");

  useEffect(() => {
    if (agentStatus === "idle") {
      const g = GREETINGS[Math.floor(Math.random() * GREETINGS.length)];
      setGreeting(g.replace(/\[称呼\]/g, displayNickname));
      setDisplayedGreeting("");
    }
  }, [agentStatus, displayNickname]);

  useEffect(() => {
    if (!greeting || agentStatus !== "idle") return;

    let currentIndex = 0;
    const intervalId = setInterval(() => {
      setDisplayedGreeting(greeting.substring(0, currentIndex + 1));
      currentIndex++;
      if (currentIndex >= greeting.length) {
        clearInterval(intervalId);
      }
    }, 40);

    return () => clearInterval(intervalId);
  }, [greeting, agentStatus]);

  const handleLaunch = async (): Promise<void> => {
    if (!task.trim() || !activeTabId || !projectPath) return;
    try {
      // 重置该 tab 的会话级字段（保留 task / agent / projectPath / title）；
      // 然后置 running 状态、清掉 sessionId（让 IPC 早期事件按 fallback 路由进来）
      update({
        sessionId: null,
        percent: 0,
        uiState: "running",
        mode: "working",
        agentStatus: "running",
        cliBlocks: [],
        resultZh: "",
        summaryTranslation: "",
        emotionText: "",
        suggestionOptions: [],
      });

      const res = await invoke<{ sessionId?: string }>("start_agent", {
        userInputZh: task,
        cwd: projectPath || ".",
        agent: tab.agent,
        runId: activeTabId,
      });
      if (res?.sessionId) {
        update({ sessionId: res.sessionId });
      }
    } catch (err) {
      addLogEntry({
        timestamp: Date.now(),
        level: "error",
        message: `launch err: ${String(err)}`,
      });
      update({
        uiState: "error",
        mode: "error",
        agentStatus: "error",
      });
    }
  };

  return (
    <AnimatePresence>
      {agentStatus === "idle" && (
        <motion.div
          key="input-bubble"
          initial={{ opacity: 0, y: 10, scale: 0.95 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: -10, scale: 0.95 }}
          transition={{ type: "spring", damping: 22, stiffness: 280 }}
          className="relative w-full overflow-hidden rounded-[22px] rounded-bl-[6px] p-[2px] shadow-lg shadow-amber-500/10 dark:shadow-none"
        >
          {/* Faint amber base layer */}
          <div className="absolute inset-0 bg-amber-100/50 dark:bg-amber-400/10" />

          {/* Spinning conic gradient glow — longer visible arc in light mode */}
          <div className="absolute top-[-50%] left-[-50%] h-[200%] w-[200%] animate-[spin_4s_linear_infinite] bg-[conic-gradient(from_0deg,transparent_60%,#fb923c_100%)] dark:bg-[conic-gradient(from_0deg,transparent_75%,#fb923c_100%)]" />

          {/* Inner glass content container */}
          <div className="relative flex h-full w-full flex-col rounded-[20px] rounded-bl-[4px] border border-white/60 bg-white/70 p-5 backdrop-blur-2xl dark:border-white/10 dark:bg-slate-800/60">
            <div className="mb-4 min-h-[3rem] text-[15px] font-medium leading-relaxed tracking-wide text-zinc-600 dark:text-zinc-300">
              {displayedGreeting}
              {displayedGreeting.length < greeting.length && (
                <motion.span
                  animate={{ opacity: [1, 0] }}
                  transition={{ repeat: Infinity, duration: 0.8 }}
                  className="ml-1 inline-block h-[15px] w-2 bg-sky-400/70 align-middle"
                />
              )}
            </div>

            <textarea
              value={task}
              onChange={(e) => update({ task: e.target.value })}
              placeholder="和团长对话……"
              className="min-h-[100px] w-full resize-none rounded-xl border border-black/5 bg-white/50 p-3.5 text-sm text-zinc-800 outline-none transition-all placeholder:text-zinc-400 focus:border-sky-400/50 focus:bg-white/80 focus:ring-2 focus:ring-sky-400/15 dark:border-white/5 dark:bg-slate-900/40 dark:text-zinc-100 dark:placeholder:text-zinc-500 dark:focus:border-sky-400/40 dark:focus:bg-slate-900/60 dark:focus:ring-sky-400/10"
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  void handleLaunch();
                }
              }}
            />

            <div className="mt-4 flex items-center justify-end gap-3">
              {!projectPath && (
                <span className="text-[11px] text-amber-600/90 dark:text-amber-300/90">
                  请先在顶部选择项目目录
                </span>
              )}
              <motion.button
                whileHover={{ scale: 1.02, y: -1 }}
                whileTap={{ scale: 0.98 }}
                onClick={() => void handleLaunch()}
                disabled={!task.trim() || !projectPath}
                className="rounded-xl bg-sky-500 px-6 py-2.5 text-sm font-semibold tracking-wide text-white shadow-md shadow-sky-400/25 transition-all hover:bg-sky-600 hover:shadow-sky-400/40 active:bg-sky-700 disabled:cursor-not-allowed disabled:opacity-50"
              >
                启动
              </motion.button>
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
