import { motion } from "framer-motion";
import { respondPermissionInvoke } from "../hooks/useAgentIPC";
import { useAppStore } from "../stores/useAppStore";

export function PermissionPrompt(): JSX.Element | null {
  const pending = useAppStore((s) => s.pendingPermission);
  const setPending = useAppStore((s) => s.setPendingPermission);

  if (!pending) return null;

  const onDecision = async (decision: "allow" | "deny"): Promise<void> => {
    try {
      await respondPermissionInvoke(pending.sessionId, pending.toolUseId, decision);
    } catch (e) {
      console.error(e);
    } finally {
      setPending(null);
    }
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="fixed inset-x-4 bottom-24 z-[200] mx-auto max-w-md rounded-2xl border border-amber-400/40 bg-amber-50/95 p-4 shadow-xl backdrop-blur-md dark:border-amber-500/35 dark:bg-zinc-900/95"
    >
      <p className="mb-1 text-xs font-bold uppercase tracking-wider text-amber-800 dark:text-amber-200">
        OpenCode 权限请求
      </p>
      <p className="mb-2 text-sm font-semibold text-zinc-900 dark:text-zinc-100">{pending.toolName}</p>
      {pending.toolDescription ? (
        <pre className="mb-3 max-h-28 overflow-auto rounded-lg bg-white/80 p-2 text-xs text-zinc-700 dark:bg-zinc-800/80 dark:text-zinc-300">
          {pending.toolDescription}
        </pre>
      ) : pending.rawInput ? (
        <pre className="mb-3 max-h-28 overflow-auto rounded-lg bg-white/80 p-2 text-xs text-zinc-700 dark:bg-zinc-800/80 dark:text-zinc-300">
          {JSON.stringify(pending.rawInput, null, 2)}
        </pre>
      ) : null}
      <div className="flex justify-end gap-2">
        <button
          type="button"
          onClick={() => void onDecision("deny")}
          className="rounded-lg border border-zinc-300 px-3 py-1.5 text-xs font-medium text-zinc-700 dark:border-zinc-600 dark:text-zinc-200"
        >
          拒绝
        </button>
        <button
          type="button"
          onClick={() => void onDecision("allow")}
          className="rounded-lg bg-emerald-600 px-3 py-1.5 text-xs font-semibold text-white shadow-md hover:bg-emerald-500"
        >
          允许
        </button>
      </div>
    </motion.div>
  );
}
