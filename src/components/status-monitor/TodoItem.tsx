import type { TodoItem as TodoItemType } from "../../types/agent";

const dot: Record<TodoItemType["status"], string> = {
  pending: "bg-zinc-400",
  in_progress: "bg-amber-400",
  completed: "bg-emerald-500",
  error: "bg-rose-500",
};

export function TodoItem({ item }: { item: TodoItemType }): JSX.Element {
  return (
    <li className="flex items-start gap-2 rounded-md bg-white/40 px-2 py-1 dark:bg-zinc-800/40">
      <span className={`mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full ${dot[item.status]}`} />
      <span className="leading-snug">{item.content}</span>
    </li>
  );
}
