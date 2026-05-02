import { useAppStore } from "../../stores/useAppStore";

export function LogStream(): JSX.Element {
  const logEntries = useAppStore((s) => s.logEntries);

  return (
    <div className="flex-1 min-h-[80px] max-h-[160px] overflow-y-auto rounded-lg bg-zinc-100/70 p-2 font-mono text-[11px] leading-relaxed dark:bg-zinc-800/70">
      {logEntries.length === 0 ? (
        <div className="text-zinc-400 dark:text-zinc-500">等待 Agent 事件…</div>
      ) : (
        logEntries.map((entry, i) => (
          <div
            key={`${i}-${entry.timestamp}`}
            className={`whitespace-pre-wrap break-all ${
              entry.level === "error"
                ? "text-rose-600 dark:text-rose-400"
                : "text-zinc-600 dark:text-zinc-300"
            }`}
          >
            {entry.message}
          </div>
        ))
      )}
    </div>
  );
}
