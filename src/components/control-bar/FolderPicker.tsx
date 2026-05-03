import { useCallback } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "../../stores/useAppStore";

export function FolderPicker(): JSX.Element {
  const setProjectPath = useAppStore((s) => s.setProjectPath);
  const addLogEntry = useAppStore((s) => s.addLogEntry);

  const pickFolder = useCallback(async () => {
    try {
      const result = await open({ directory: true });
      if (!result) return;
      const path = Array.isArray(result) ? result[0] : result;
      setProjectPath(path);
    } catch (err) {
      addLogEntry({
        timestamp: Date.now(),
        level: "error",
        message: `select_project_folder: ${String(err)}`,
      });
    }
  }, [setProjectPath, addLogEntry]);

  return (
    <button
      type="button"
      onClick={pickFolder}
      className="rounded-xl border border-white/20 bg-white/10 px-3 py-1.5 text-xs font-medium text-zinc-900 backdrop-blur-md transition-all duration-300 hover:-translate-y-0.5 hover:bg-white/20 hover:shadow-lg dark:text-zinc-100"
    >
      选择文件夹
    </button>
  );
}
