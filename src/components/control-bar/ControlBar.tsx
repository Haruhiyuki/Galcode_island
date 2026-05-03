import { useAppStore } from "../../stores/useAppStore";
import { AgentSelector } from "./AgentSelector";
import { FolderPicker } from "./FolderPicker";
import { LaunchButton } from "./LaunchButton";

export function ControlBar(): JSX.Element {
  const projectPath = useAppStore((s) => s.projectPath);

  return (
    <header className="flex items-center justify-between gap-3 rounded-xl border border-zinc-300/70 bg-white/70 p-3 text-sm backdrop-blur dark:border-zinc-700/70 dark:bg-zinc-900/60">
      <div className="flex items-center gap-2 min-w-0">
        <FolderPicker />
        <AgentSelector />
      </div>
      <div className="flex items-center gap-2">
        {projectPath ? (
          <span className="text-xs text-zinc-500 dark:text-zinc-400 truncate max-w-[160px]">
            {projectPath}
          </span>
        ) : null}
        <LaunchButton />
      </div>
    </header>
  );
}
