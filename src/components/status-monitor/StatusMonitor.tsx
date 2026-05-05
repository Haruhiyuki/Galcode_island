// 由 MainView 的 showStatus 控制是否挂载——这里**不再**自带 isVisible 守卫，
// 否则会出现"MainView 让显示但 StatusMonitor 自己隐藏"的死锁。
//
// turn 进行中：BlockStream 实时累积；turn 完成后保留 blocks 历史让用户回顾，
// 直到 InputBubble.handleLaunch 触发 setSessionId(null) → useCliStream
// clearCliBlocks 清空。

import { AgentStatusBadge } from "./AgentStatusBadge";
import { BlockStream } from "./BlockStream";
import { LogStream } from "./LogStream";
import { TodoProgress } from "./TodoProgress";

export function StatusMonitor(): JSX.Element {
  return (
    <section className="flex h-full min-h-0 flex-col gap-3 overflow-hidden rounded-xl border border-white/60 bg-white/70 p-4 shadow-[0_8px_30px_rgba(0,0,0,0.04)] backdrop-blur-2xl dark:border-white/10 dark:bg-slate-800/60 dark:shadow-none">
      <div className="flex shrink-0 items-center justify-between">
        <AgentStatusBadge />
      </div>
      <div className="shrink-0">
        <TodoProgress />
      </div>
      <div className="min-h-0 flex-1">
        <BlockStream />
      </div>
      <div className="shrink-0">
        <LogStream />
      </div>
    </section>
  );
}
