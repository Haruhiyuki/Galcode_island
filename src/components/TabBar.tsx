// 多 tab 标签栏。
//
// 嵌入 GlobalTopBar 的中间区域（标题区）。
// - 按 tabsStore.order 顺序渲染
// - 每个 tab 显示：title + 运行中状态点（agentStatus !== "idle"）+ 未读小红点
//   （hasUnread === true）+ 关闭按钮（仅多 tab 时显示）
// - 末尾一个 `+` 按钮新建 tab
// - 单击切换；中键关闭；双击 active tab 进入重命名
// - 鼠标滚轮可水平滚动 tab 列表（tabs 多时）

import { invoke } from "@tauri-apps/api/core";
import { motion } from "framer-motion";
import { useEffect, useRef, useState } from "react";
import { useAppStore } from "../stores/useAppStore";
import { useTabsStore, type TabState } from "../stores/useTabsStore";
import type { WheelEvent as ReactWheelEvent } from "react";

function isTabRunning(tab: TabState): boolean {
  return (
    tab.agentStatus === "running" ||
    tab.agentStatus === "thinking" ||
    tab.agentStatus === "processing" ||
    tab.uiState === "running"
  );
}

interface TabItemProps {
  tab: TabState;
  isActive: boolean;
  canClose: boolean;
  onSelect: () => void;
  onClose: () => void;
  onRename: (next: string) => void;
}

function TabItem({ tab, isActive, canClose, onSelect, onClose, onRename }: TabItemProps): JSX.Element {
  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState(tab.title);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (isEditing) {
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [isEditing]);

  // tab.title 外部更新时同步 draft（避免重命名后还在显示旧值）
  useEffect(() => {
    if (!isEditing) setDraft(tab.title);
  }, [tab.title, isEditing]);

  const commitRename = (): void => {
    const next = draft.trim();
    if (next && next !== tab.title) {
      onRename(next);
    } else {
      setDraft(tab.title);
    }
    setIsEditing(false);
  };

  const running = isTabRunning(tab);

  return (
    <div
      onClick={() => {
        if (!isActive) onSelect();
      }}
      onAuxClick={(e) => {
        // 鼠标中键关闭
        if (e.button === 1 && canClose) {
          e.preventDefault();
          onClose();
        }
      }}
      onDoubleClick={() => {
        if (isActive) setIsEditing(true);
      }}
      className={`group flex h-6 max-w-[180px] shrink-0 items-center gap-1.5 rounded-md border px-2 text-[11px] font-medium transition-all ${
        isActive
          ? "border-sky-400/40 bg-sky-400/15 text-zinc-800 shadow-sm dark:border-sky-300/40 dark:bg-sky-400/20 dark:text-zinc-100"
          : "cursor-pointer border-white/40 bg-white/40 text-zinc-600 hover:bg-white/65 dark:border-white/10 dark:bg-zinc-800/40 dark:text-zinc-300 dark:hover:bg-zinc-800/65"
      }`}
      role="tab"
      aria-selected={isActive}
    >
      {/* 运行中状态点 / 未读小红点 */}
      {running ? (
        <span className="h-1.5 w-1.5 shrink-0 animate-pulse rounded-full bg-sky-400 shadow-[0_0_4px_rgba(56,189,248,0.6)]" />
      ) : tab.hasUnread ? (
        <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-rose-400 shadow-[0_0_4px_rgba(251,113,133,0.6)]" />
      ) : null}

      {isEditing ? (
        <input
          ref={inputRef}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commitRename}
          onKeyDown={(e) => {
            if (e.key === "Enter") commitRename();
            else if (e.key === "Escape") {
              setDraft(tab.title);
              setIsEditing(false);
            }
          }}
          onClick={(e) => e.stopPropagation()}
          className="min-w-[60px] max-w-[140px] truncate bg-transparent text-[11px] outline-none"
        />
      ) : (
        <span className="truncate" title={tab.title}>
          {tab.title}
        </span>
      )}

      {canClose && !isEditing && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation();
            onClose();
          }}
          aria-label="关闭 tab"
          className="ml-0.5 flex h-3.5 w-3.5 shrink-0 items-center justify-center rounded text-zinc-400 opacity-0 transition-opacity hover:bg-rose-400/20 hover:text-rose-500 group-hover:opacity-100 dark:text-zinc-500 dark:hover:text-rose-400"
        >
          <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" className="h-2.5 w-2.5">
            <path d="M2.5 2.5l7 7M9.5 2.5l-7 7" strokeLinecap="round" />
          </svg>
        </button>
      )}
    </div>
  );
}

export function TabBar(): JSX.Element | null {
  const order = useTabsStore((s) => s.order);
  const tabs = useTabsStore((s) => s.tabs);
  const activeTabId = useTabsStore((s) => s.activeTabId);
  const setActiveTab = useTabsStore((s) => s.setActiveTab);
  const removeTab = useTabsStore((s) => s.removeTab);
  const updateTab = useTabsStore((s) => s.updateTab);
  const createTab = useTabsStore((s) => s.createTab);
  const selectedAgent = useAppStore((s) => s.selectedAgent);
  const setIsStarted = useAppStore((s) => s.setIsStarted);
  const addLogEntry = useAppStore((s) => s.addLogEntry);

  // 鼠标滚轮在 tab 列表上时把竖向滚动转成横向滚动（tab 多时方便切换）
  const listRef = useRef<HTMLDivElement | null>(null);
  const handleWheel = (e: ReactWheelEvent<HTMLDivElement>): void => {
    if (!listRef.current) return;
    if (e.deltaY === 0) return;
    listRef.current.scrollLeft += e.deltaY;
  };

  // 没有 tab（理论上不该发生 — WelcomeView 启动时会创建第一个）；退化为不渲染
  if (order.length === 0) return null;

  const handleClose = async (id: string): Promise<void> => {
    const tab = tabs[id];
    if (!tab) return;

    // 1) 如果还在跑或留有 session，调后端 stop（互斥安全：后端找不到的会报错，吞掉）
    if (tab.sessionId || isTabRunning(tab)) {
      try {
        await invoke("stop_agent", { runId: id, sessionId: tab.sessionId });
      } catch (err) {
        addLogEntry({
          timestamp: Date.now(),
          level: "warn",
          message: `关闭 tab 时 stop_agent 失败: ${String(err)}`,
        });
      }
    }

    // 2) 从 store 移除（自动切到相邻 tab；为最后一个时 activeTabId 变 null）
    removeTab(id);

    // 3) 最后一个 tab 关闭后回到 WelcomeView
    const remaining = useTabsStore.getState().order;
    if (remaining.length === 0) {
      setIsStarted(false);
    }
  };

  const handleCreate = (): void => {
    const id = createTab({
      title: "新会话",
      agent: selectedAgent,
      projectPath: null,
    });
    setActiveTab(id);
  };

  return (
    <div className="flex min-w-0 flex-1 items-center gap-1">
      <div
        ref={listRef}
        onWheel={handleWheel}
        className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto scrollbar-none"
        role="tablist"
      >
        {order.map((id) => {
          const tab = tabs[id];
          if (!tab) return null;
          return (
            <TabItem
              key={id}
              tab={tab}
              isActive={activeTabId === id}
              canClose={order.length > 1}
              onSelect={() => setActiveTab(id)}
              onClose={() => void handleClose(id)}
              onRename={(next) => updateTab(id, { title: next })}
            />
          );
        })}
      </div>

      <motion.button
        type="button"
        whileTap={{ scale: 0.92 }}
        onClick={handleCreate}
        aria-label="新建 tab"
        title="新建会话"
        className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md border border-white/40 bg-white/40 text-zinc-500 transition-all hover:bg-white/65 hover:text-zinc-800 dark:border-white/10 dark:bg-zinc-800/40 dark:text-zinc-400 dark:hover:bg-zinc-800/65 dark:hover:text-zinc-100"
      >
        <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.6" className="h-3 w-3">
          <path d="M7 2v10M2 7h10" strokeLinecap="round" />
        </svg>
      </motion.button>
    </div>
  );
}
