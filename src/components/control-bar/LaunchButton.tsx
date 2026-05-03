import { useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../../stores/useAppStore";

export function LaunchButton(): JSX.Element {
  const uiState = useAppStore((s) => s.uiState);
  const task = useAppStore((s) => s.task);
  const projectPath = useAppStore((s) => s.projectPath);

  const setSessionId = useAppStore((s) => s.setSessionId);
  const setUiState = useAppStore((s) => s.setUiState);
  const setLastStage = useAppStore((s) => s.setLastStage);
  const setBubble = useAppStore((s) => s.setBubble);
  const setPercent = useAppStore((s) => s.setPercent);
  const setResultZh = useAppStore((s) => s.setResultZh);
  const setSummaryTranslation = useAppStore((s) => s.setSummaryTranslation);
  const setEmotionText = useAppStore((s) => s.setEmotionText);
  const setSuggestionOptions = useAppStore((s) => s.setSuggestionOptions);
  const setAgentStatus = useAppStore((s) => s.setAgentStatus);
  const clearLogs = useAppStore((s) => s.clearLogs);
  const clearTodos = useAppStore((s) => s.clearTodos);
  const addLogEntry = useAppStore((s) => s.addLogEntry);

  const launch = useCallback(async () => {
    clearLogs();
    clearTodos();
    setResultZh("");
    setSummaryTranslation("");
    setEmotionText("");
    setSuggestionOptions([]);
    setPercent(0);
    setUiState("running");
    setAgentStatus("running");
    setLastStage("init");
    setBubble("启动 Agent…");
    setSessionId(null);

    try {
      const res = await invoke<{ sessionId: string; status: string }>("start_agent", {
        userInputZh: task,
        cwd: projectPath || ".",
      });
      setSessionId(res.sessionId);
    } catch (err) {
      setUiState("error");
      setAgentStatus("error");
      setBubble(String(err));
      addLogEntry({
        timestamp: Date.now(),
        level: "error",
        message: `[error] ${String(err)}`,
      });
    }
  }, [
    task,
    projectPath,
    clearLogs,
    clearTodos,
    setResultZh,
    setSummaryTranslation,
    setEmotionText,
    setSuggestionOptions,
    setPercent,
    setUiState,
    setAgentStatus,
    setLastStage,
    setBubble,
    setSessionId,
    addLogEntry,
  ]);

  const isRunning = uiState === "running";

  return (
    <button
      type="button"
      onClick={launch}
      disabled={isRunning}
      className="rounded-xl border border-emerald-500/55 bg-emerald-500/28 px-4 py-1.5 text-xs font-medium text-zinc-900 backdrop-blur-md transition-all duration-300 hover:-translate-y-0.5 hover:bg-emerald-500/35 hover:shadow-lg disabled:cursor-not-allowed disabled:opacity-45 dark:border-emerald-300/50 dark:bg-emerald-500/35 dark:text-zinc-100"
    >
      {isRunning ? "运行中…" : "启动 Agent"}
    </button>
  );
}
