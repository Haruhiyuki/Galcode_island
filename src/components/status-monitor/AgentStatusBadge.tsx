import { useMemo } from "react";
import { useAppStore } from "../../stores/useAppStore";

export function AgentStatusBadge(): JSX.Element {
  const uiState = useAppStore((s) => s.uiState);
  const sessionId = useAppStore((s) => s.sessionId);

  const color = useMemo(() => {
    switch (uiState) {
      case "running": return "bg-emerald-400 shadow-emerald-400/40";
      case "done": return "bg-sky-400 shadow-sky-400/40";
      case "error": return "bg-rose-400 shadow-rose-400/40";
      case "suggesting": return "bg-violet-400 shadow-violet-400/40";
      default: return "bg-zinc-400 shadow-zinc-400/40";
    }
  }, [uiState]);

  return (
    <div className="flex items-center gap-2">
      <div className="flex items-center gap-2 rounded-full border border-white/60 bg-white/70 px-3 py-1 backdrop-blur-md dark:border-white/10 dark:bg-slate-800/60">
        <span className={`h-2.5 w-2.5 rounded-full shadow-[0_0_8px] ${color}`} />
        <span className="text-xs uppercase tracking-[0.18em] text-zinc-500 dark:text-zinc-400">
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
