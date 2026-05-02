import { AgentStatusBadge } from "./AgentStatusBadge";
import { TodoProgress } from "./TodoProgress";
import { LogStream } from "./LogStream";

export function StatusMonitor(): JSX.Element {
  return (
    <section className="flex flex-col gap-3 rounded-xl border border-zinc-300/70 bg-white/65 p-4 backdrop-blur dark:border-zinc-700/70 dark:bg-zinc-900/55">
      <div className="flex items-center justify-between">
        <AgentStatusBadge />
      </div>
      <TodoProgress />
      <LogStream />
    </section>
  );
}
