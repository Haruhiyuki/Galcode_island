import { useMemo } from "react";
import { useAppStore } from "../../stores/useAppStore";

export function AgentStatusBadge(): JSX.Element {
  const uiState = useAppStore((s) => s.uiState);
  const sessionId = useAppStore((s) => s.sessionId);

  const color = useMemo(() => {
    switch (uiState) {
      case "running": return "bg-emerald-400 shadow-emerald-400/70";
      case "done": return "bg-sky-400 shadow-sky-400/70";
      case "error": return "bg-rose-500 shadow-rose-500/75";
      case "suggesting": return "bg-violet-400 shadow-violet-400/70";
      default: return "bg-zinc-400 shadow-zinc-400/60";
    }
  }, [uiState]);

  return (
    <div className="flex items-center gap-2">
      <div className="flex items-center gap-2 rounded-full border border-zinc-300/70 bg-white/65 px-3 py-1 dark:border-zinc-700/70 dark:bg-zinc-900/55">
        <span className={`h-2.5 w-2.5 rounded-full shadow-[0_0_12px] ${color}`} />
        <span className="text-xs uppercase tracking-[0.18em] text-zinc-600 dark:text-zinc-300">
          {uiState}
        </span>
      </div>
      {sessionId ? (
        <span className="text-xs text-zinc-400 dark:text-zinc-500">
          session: {sessionId.slice(0, 8)}…
        </span>
      ) : null}
    </div>
  );
}
