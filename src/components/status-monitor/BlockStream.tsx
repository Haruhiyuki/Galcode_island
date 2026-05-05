// 流式渲染三个 backend 通过 `galcode://cli-output` 推过来的 block。
// 不同类型分别渲染：
//   - text     普通文本气泡（Agent 中间消息）
//   - thought  灰色折叠 / 等宽字体（思考过程）
//   - command  黑底等宽（终端样式 + 命令 + 输出）
//   - todo     列表 + 状态图标
//   - confirm  黄色卡片（auto-approve 模式下也会一闪而过）
//   - tool     一行小标签（OpenCode 工具调用）
//   - file     文件路径 + 工具
//   - status   单行小标签
//   - error    红色提示
//
// 用 AnimatePresence 让新增 / 移除带过渡，但避免每次 update 都触发动画
// （update 是同 id，AnimatePresence 不会重放 enter）。

import { motion, AnimatePresence } from "framer-motion";
import { useAppStore } from "../../stores/useAppStore";
import type { CliBlock } from "../../types/blocks";

function statusBadge(status?: string): { label: string; cls: string } {
  switch (status) {
    case "success":
    case "completed":
      return { label: "✓", cls: "text-emerald-600 dark:text-emerald-400" };
    case "error":
    case "failed":
      return { label: "✗", cls: "text-rose-600 dark:text-rose-400" };
    case "running":
      return { label: "⏵", cls: "text-sky-600 dark:text-sky-400" };
    case "waiting":
      return { label: "?", cls: "text-amber-600 dark:text-amber-400" };
    case "pending":
      return { label: "·", cls: "text-zinc-500 dark:text-zinc-400" };
    default:
      return { label: "·", cls: "text-zinc-500 dark:text-zinc-400" };
  }
}

function TextBlock({ block }: { block: CliBlock }): JSX.Element | null {
  const content = block.content?.trim();
  if (!content) return null;
  const accent =
    block.tone === "file" ? "text-sky-700 dark:text-sky-300" : "text-zinc-800 dark:text-zinc-100";
  return (
    <div className={`whitespace-pre-wrap text-xs leading-relaxed ${accent}`}>{content}</div>
  );
}

function ThoughtBlock({ block }: { block: CliBlock }): JSX.Element | null {
  const content = block.content?.trim();
  if (!content) return null;
  return (
    <div className="rounded-md border-l-2 border-zinc-300 bg-zinc-100/40 px-2 py-1 font-mono text-[11px] leading-relaxed text-zinc-500 dark:border-zinc-700 dark:bg-zinc-800/30 dark:text-zinc-400">
      {content}
    </div>
  );
}

function CommandBlock({ block }: { block: CliBlock }): JSX.Element {
  const badge = statusBadge(block.status);
  const cmd = block.command?.trim() || "(command)";
  const output = block.output?.trim();
  return (
    <div className="overflow-hidden rounded-md border border-zinc-700/30 bg-zinc-900/95 font-mono text-[11px] leading-relaxed text-zinc-200 dark:border-zinc-600/30">
      <div className="flex items-center gap-2 border-b border-zinc-700/30 px-2 py-1 dark:border-zinc-600/30">
        <span className={badge.cls}>{badge.label}</span>
        <span className="truncate">$ {cmd}</span>
      </div>
      {output ? (
        <pre className="max-h-32 overflow-y-auto whitespace-pre-wrap break-all px-2 py-1 text-zinc-400">
          {output}
        </pre>
      ) : null}
    </div>
  );
}

function TodoBlock({ block }: { block: CliBlock }): JSX.Element | null {
  const items = block.items ?? [];
  if (items.length === 0) return null;
  return (
    <div className="rounded-md border border-amber-300/40 bg-amber-50/60 p-2 text-xs dark:border-amber-400/30 dark:bg-amber-400/5">
      <div className="mb-1 text-[11px] font-semibold text-amber-700 dark:text-amber-300">
        {block.title || "Todo"}
      </div>
      <ul className="flex flex-col gap-0.5">
        {items.map((item) => {
          const badge = statusBadge(item.status);
          return (
            <li key={item.id} className="flex items-start gap-1.5">
              <span className={`mt-0.5 ${badge.cls}`}>{badge.label}</span>
              <span className="text-zinc-700 dark:text-zinc-200">{item.label}</span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function ConfirmBlock({ block }: { block: CliBlock }): JSX.Element {
  return (
    <div className="rounded-md border border-amber-400/50 bg-amber-50/70 px-2 py-1.5 text-xs dark:border-amber-300/40 dark:bg-amber-400/10">
      <div className="font-semibold text-amber-700 dark:text-amber-300">{block.title || "需要确认"}</div>
      {block.content ? (
        <div className="mt-0.5 whitespace-pre-wrap text-zinc-700 dark:text-zinc-200">
          {block.content}
        </div>
      ) : null}
      {block.note ? (
        <div className="mt-0.5 text-[10px] text-amber-600 dark:text-amber-400">{block.note}</div>
      ) : null}
    </div>
  );
}

function ToolBlock({ block }: { block: CliBlock }): JSX.Element {
  const badge = statusBadge(block.status);
  return (
    <div className="flex items-center gap-2 rounded-md bg-zinc-100/50 px-2 py-1 text-[11px] dark:bg-zinc-800/40">
      <span className={badge.cls}>{badge.label}</span>
      <span className="font-medium text-zinc-700 dark:text-zinc-200">{block.tool || "tool"}</span>
      {block.detail ? (
        <span className="truncate text-zinc-500 dark:text-zinc-400">{block.detail}</span>
      ) : null}
      {block.message ? (
        <span className="truncate text-rose-600 dark:text-rose-400">{block.message}</span>
      ) : null}
    </div>
  );
}

function DiffBlock({ block }: { block: CliBlock }): JSX.Element {
  const path = block.path?.trim() || "(unknown file)";
  const tool = block.tool?.trim() || "Edit";
  const lines = (block.diff ?? "").split("\n");
  const summary = (() => {
    let added = 0;
    let removed = 0;
    for (const l of lines) {
      if (l.startsWith("+")) added += 1;
      else if (l.startsWith("-")) removed += 1;
    }
    return `+${added} −${removed}`;
  })();
  return (
    <div className="overflow-hidden rounded-md border border-zinc-700/30 bg-zinc-900/95 font-mono text-[11px] leading-relaxed">
      <div className="flex items-center justify-between border-b border-zinc-700/30 px-2 py-1 text-zinc-300">
        <span className="flex items-center gap-2 truncate">
          <span className="text-amber-300">{tool}</span>
          <span className="truncate text-sky-300">{path}</span>
        </span>
        <span className="shrink-0 text-[10px] text-zinc-400">{summary}</span>
      </div>
      <pre className="max-h-44 overflow-y-auto whitespace-pre-wrap break-all">
        {lines.map((line, i) => {
          let cls = "text-zinc-400";
          if (line.startsWith("+")) cls = "bg-emerald-500/10 text-emerald-300";
          else if (line.startsWith("-")) cls = "bg-rose-500/10 text-rose-300";
          else if (line.startsWith("@@")) cls = "text-amber-300";
          return (
            <div key={i} className={`px-2 ${cls}`}>
              {line || " "}
            </div>
          );
        })}
      </pre>
    </div>
  );
}

function StderrBlock({ block }: { block: CliBlock }): JSX.Element | null {
  const msg = block.message?.trim();
  if (!msg) return null;
  // 启发式：含 error/failed/panic 时染红，否则灰（多数是 warning / debug）
  const looksLikeError = /(error|failed|panic|exception|fatal)/i.test(msg);
  const cls = looksLikeError
    ? "border-rose-400/40 bg-rose-500/10 text-rose-700 dark:text-rose-300"
    : "border-zinc-300/40 bg-zinc-100/40 text-zinc-500 dark:border-zinc-600/40 dark:bg-zinc-800/30 dark:text-zinc-400";
  return (
    <div
      className={`flex items-start gap-1.5 rounded-md border-l-2 px-2 py-1 font-mono text-[11px] leading-relaxed ${cls}`}
    >
      <span className="shrink-0 opacity-60">stderr</span>
      <span className="break-all">{msg}</span>
    </div>
  );
}

function FileBlock({ block }: { block: CliBlock }): JSX.Element {
  const badge = statusBadge(block.status);
  return (
    <div className="flex items-center gap-2 rounded-md border-l-2 border-sky-400/50 bg-sky-50/40 px-2 py-1 text-[11px] dark:border-sky-300/30 dark:bg-sky-400/5">
      <span className={badge.cls}>{badge.label}</span>
      <span className="font-medium text-zinc-700 dark:text-zinc-200">{block.tool || "file"}</span>
      {block.path ? (
        <span className="truncate font-mono text-sky-700 dark:text-sky-300">{block.path}</span>
      ) : null}
    </div>
  );
}

function StatusLine({ block }: { block: CliBlock }): JSX.Element | null {
  const msg = block.message?.trim();
  if (!msg) return null;
  return (
    <div className="text-[11px] italic text-zinc-500 dark:text-zinc-400">{msg}</div>
  );
}

function ErrorLine({ block }: { block: CliBlock }): JSX.Element | null {
  const msg = block.message?.trim();
  if (!msg) return null;
  return (
    <div className="rounded-md border border-rose-400/40 bg-rose-50/60 px-2 py-1 text-[11px] text-rose-700 dark:border-rose-300/30 dark:bg-rose-400/10 dark:text-rose-300">
      {msg}
    </div>
  );
}

function BlockRenderer({ block }: { block: CliBlock }): JSX.Element | null {
  switch (block.type) {
    case "text":
      return <TextBlock block={block} />;
    case "thought":
      return <ThoughtBlock block={block} />;
    case "command":
      return <CommandBlock block={block} />;
    case "todo":
      return <TodoBlock block={block} />;
    case "confirm":
      return <ConfirmBlock block={block} />;
    case "tool":
      return <ToolBlock block={block} />;
    case "file":
      return <FileBlock block={block} />;
    case "diff":
      return <DiffBlock block={block} />;
    case "status":
      return <StatusLine block={block} />;
    case "error":
      return <ErrorLine block={block} />;
    case "stderr":
      return <StderrBlock block={block} />;
    default:
      return null;
  }
}

export function BlockStream(): JSX.Element {
  const blocks = useAppStore((s) => s.cliBlocks);
  console.log("[BlockStream] render, blocks=", blocks.length);

  return (
    <div className="flex min-h-[120px] flex-col gap-1.5 rounded-lg border border-sky-300/40 bg-sky-50/30 p-2 dark:border-sky-400/20 dark:bg-sky-400/5">
      <div className="sticky top-0 flex items-center justify-between border-b border-sky-300/30 pb-1 text-[11px] font-semibold text-sky-700 dark:border-sky-400/20 dark:text-sky-300">
        <span>🔄 Agent 流式输出</span>
        <span className="rounded-full bg-sky-200/60 px-2 py-0.5 text-[10px] dark:bg-sky-400/20">
          {blocks.length}
        </span>
      </div>
      {blocks.length === 0 ? (
        <div className="flex flex-1 items-center justify-center text-[11px] text-zinc-400 dark:text-zinc-500">
          Agent 还没产出中间消息（block 计数 = 0）
        </div>
      ) : (
        <div className="flex max-h-[400px] flex-col gap-1.5 overflow-y-auto">
          <AnimatePresence initial={false}>
            {blocks.map((block) => (
              <motion.div
                key={block.id}
                initial={{ opacity: 0, y: 4 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -4 }}
                transition={{ duration: 0.18, ease: "easeOut" }}
              >
                <BlockRenderer block={block} />
              </motion.div>
            ))}
          </AnimatePresence>
        </div>
      )}
    </div>
  );
}
