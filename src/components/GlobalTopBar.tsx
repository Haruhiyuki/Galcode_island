import { getCurrentWindow } from "@tauri-apps/api/window";
import type { MouseEvent } from "react";
import { useAppStore } from "../stores/useAppStore";

export function GlobalTopBar(): JSX.Element {
  const theme = useAppStore((state) => state.theme);
  const toggleTheme = useAppStore((state) => state.toggleTheme);
  const appWindow = getCurrentWindow();
  const handleDragMouseDown = async (event: MouseEvent<HTMLDivElement>): Promise<void> => {
    if (event.button !== 0) {
      return;
    }
    try {
      await appWindow.startDragging();
    } catch (error) {
      console.error("Failed to start dragging window", error);
    }
  };

  return (
    <header className="absolute top-0 left-0 z-[100] flex h-10 w-full items-center justify-between px-3 pt-1">
      <div
        data-tauri-drag-region
        onMouseDown={(event) => {
          void handleDragMouseDown(event);
        }}
        className="h-full flex-1"
      />

      <div className="flex items-center gap-3 pr-1">
        <button
          type="button"
          onClick={toggleTheme}
          className="flex h-8 w-8 items-center justify-center rounded-lg bg-black/15 text-zinc-700 transition-all duration-200 hover:-translate-y-0.5 hover:bg-black/25 dark:bg-white/15 dark:text-zinc-100 dark:hover:bg-white/25"
          aria-label="切换黑白模式"
          title="切换黑白模式 (Ctrl/Cmd + Shift + L)"
        >
          {theme === "dark" ? (
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.9"
              className="h-4 w-4"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M21 12.8A9 9 0 1111.2 3a7 7 0 009.8 9.8z"
              />
            </svg>
          ) : (
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.8"
              className="h-4 w-4"
            >
              <circle cx="12" cy="12" r="4" />
              <path strokeLinecap="round" d="M12 2.5V5M12 19v2.5M21.5 12H19M5 12H2.5M18.4 5.6l-1.8 1.8M7.4 16.6l-1.8 1.8M18.4 18.4l-1.8-1.8M7.4 7.4L5.6 5.6" />
            </svg>
          )}
        </button>
        <button
          type="button"
          onClick={async () => {
            try {
              await appWindow.minimize();
            } catch (error) {
              console.error("Failed to minimize window", error);
            }
          }}
          className="flex h-8 w-8 items-center justify-center rounded-lg bg-black/15 text-sm text-zinc-700 transition-all duration-200 hover:-translate-y-0.5 hover:bg-black/25 dark:text-zinc-100 dark:hover:bg-white/20"
          aria-label="最小化窗口"
        >
          -
        </button>
        <button
          type="button"
          onClick={async () => {
            try {
              await appWindow.toggleMaximize();
            } catch (error) {
              console.error("Failed to toggle maximize window", error);
            }
          }}
          className="flex h-8 w-8 items-center justify-center rounded-lg bg-black/15 text-xs text-zinc-700 transition-all duration-200 hover:-translate-y-0.5 hover:bg-black/25 dark:text-zinc-100 dark:hover:bg-white/20"
          aria-label="最大化窗口"
        >
          □
        </button>
        <button
          type="button"
          onClick={async () => {
            try {
              await appWindow.close();
            } catch (error) {
              console.error("Failed to close window", error);
            }
          }}
          className="flex h-8 w-8 items-center justify-center rounded-lg bg-rose-500/20 text-sm text-rose-600 transition-all duration-200 hover:-translate-y-0.5 hover:bg-rose-500/35 dark:text-rose-100"
          aria-label="关闭窗口"
        >
          ×
        </button>
      </div>
    </header>
  );
}
