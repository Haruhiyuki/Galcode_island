// 三个 backend 的状态面板：装没装 / 登录情况 / 默认模型 + 验证连接 / 打开登录终端
// / OpenCode 启停。SettingsModal 打开时自动 refresh 一次。
//
// 状态条 chip 颜色：installed 绿、loggedIn 绿、未登录黄、未装红、loading 蓝。
// verify/login/start 之类操作完成后内联显示 toast（成功提示 / 错误信息）。

import { useCallback, useState } from "react";
import { useBackendStatus } from "../../hooks/useBackendStatus";

interface ChipProps {
  ok: boolean | null; // null = unknown / loading
  label: string;
  warn?: boolean;
}

function StatusChip({ ok, label, warn }: ChipProps): JSX.Element {
  const color =
    ok === true
      ? "bg-emerald-500/15 text-emerald-700 dark:bg-emerald-400/20 dark:text-emerald-300"
      : ok === false
        ? warn
          ? "bg-amber-500/15 text-amber-700 dark:bg-amber-400/20 dark:text-amber-300"
          : "bg-rose-500/15 text-rose-700 dark:bg-rose-400/20 dark:text-rose-300"
        : "bg-zinc-200/70 text-zinc-500 dark:bg-slate-700/50 dark:text-zinc-400";
  const symbol = ok === true ? "✓" : ok === false ? (warn ? "○" : "✗") : "…";
  return (
    <span
      className={`inline-flex items-center gap-1 rounded-md px-2 py-0.5 text-xs font-medium ${color}`}
    >
      <span>{symbol}</span>
      <span>{label}</span>
    </span>
  );
}

interface BackendCardProps {
  title: string;
  loading: boolean;
  children: React.ReactNode;
}

function BackendCard({ title, loading, children }: BackendCardProps): JSX.Element {
  return (
    <div className="rounded-xl border border-black/5 bg-white/40 p-4 dark:border-white/5 dark:bg-slate-800/40">
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-sm font-semibold text-zinc-800 dark:text-zinc-100">{title}</h3>
        {loading ? (
          <span className="text-xs text-sky-600 dark:text-sky-400">刷新中…</span>
        ) : null}
      </div>
      {children}
    </div>
  );
}

interface ToastState {
  kind: "success" | "error";
  message: string;
}

function Toast({ toast }: { toast: ToastState | null }): JSX.Element | null {
  if (!toast) return null;
  const color =
    toast.kind === "success"
      ? "text-emerald-700 dark:text-emerald-300"
      : "text-rose-700 dark:text-rose-300";
  return (
    <p className={`mt-2 text-xs leading-relaxed ${color}`}>
      {toast.message.length > 360 ? `${toast.message.slice(0, 360)}…` : toast.message}
    </p>
  );
}

export function AgentBackendsSection({
  isVisible,
}: {
  isVisible: boolean;
}): JSX.Element {
  const status = useBackendStatus(isVisible);
  const [toasts, setToasts] = useState<{
    claude?: ToastState;
    codex?: ToastState;
    opencode?: ToastState;
  }>({});
  const [busy, setBusy] = useState<{ claude?: boolean; codex?: boolean; opencode?: boolean }>({});

  const setBackendBusy = (k: "claude" | "codex" | "opencode", v: boolean) =>
    setBusy((b) => ({ ...b, [k]: v }));
  const setBackendToast = (k: "claude" | "codex" | "opencode", t: ToastState | undefined) =>
    setToasts((s) => ({ ...s, [k]: t }));

  const runOp = useCallback(
    async (
      backend: "claude" | "codex" | "opencode",
      op: () => Promise<string | { ok?: boolean; message?: string } | unknown>
    ) => {
      setBackendBusy(backend, true);
      setBackendToast(backend, undefined);
      try {
        const result = await op();
        const msg =
          typeof result === "string"
            ? result
            : result && typeof result === "object" && "message" in result
              ? String((result as { message: string }).message ?? "操作完成")
              : "操作完成";
        setBackendToast(backend, { kind: "success", message: msg });
      } catch (error) {
        setBackendToast(backend, { kind: "error", message: String(error) });
      } finally {
        setBackendBusy(backend, false);
      }
    },
    []
  );

  const claudeNode = (() => {
    const c = status.claude;
    return (
      <BackendCard title="Claude Code" loading={status.loading.claude}>
        <div className="flex flex-wrap items-center gap-1.5">
          <StatusChip
            ok={c ? c.installed : null}
            label={c?.installed ? `已安装${c.version ? ` ${c.version}` : ""}` : "未检测到"}
          />
          {c?.installed ? (
            <StatusChip
              ok={c.loggedIn}
              warn={!c.loggedIn}
              label={c.loggedIn ? `已登录${c.authMethod ? ` · ${c.authMethod}` : ""}` : "未登录"}
            />
          ) : null}
          {c?.defaultModel ? (
            <span className="text-xs text-zinc-500 dark:text-zinc-400">
              默认模型：{c.defaultModel}
            </span>
          ) : null}
        </div>
        {status.errors.claude ? (
          <p className="mt-2 text-xs text-rose-700 dark:text-rose-300">{status.errors.claude}</p>
        ) : null}
        <div className="mt-3 flex flex-wrap gap-2">
          <button
            type="button"
            disabled={busy.claude || !c?.installed}
            onClick={() =>
              runOp("claude", async () => {
                const r = await status.verifyClaude();
                return r.message ?? "OK";
              })
            }
            className="rounded-md border border-sky-400/50 bg-sky-500/10 px-2.5 py-1 text-xs font-medium text-sky-700 transition-all hover:bg-sky-500/20 disabled:cursor-not-allowed disabled:opacity-50 dark:border-sky-300/40 dark:text-sky-300"
          >
            {busy.claude ? "验证中…" : "验证连接"}
          </button>
          <button
            type="button"
            disabled={busy.claude}
            onClick={() => runOp("claude", () => status.openClaudeLogin())}
            className="rounded-md border border-zinc-300/60 bg-white/40 px-2.5 py-1 text-xs font-medium text-zinc-700 transition-all hover:bg-white/70 disabled:cursor-not-allowed disabled:opacity-50 dark:border-white/10 dark:bg-slate-800/40 dark:text-zinc-200 dark:hover:bg-slate-800/70"
          >
            打开登录终端
          </button>
          <button
            type="button"
            disabled={status.loading.claude}
            onClick={() => status.refreshClaude()}
            className="rounded-md px-2.5 py-1 text-xs text-zinc-500 transition-colors hover:text-zinc-700 dark:text-zinc-400 dark:hover:text-zinc-200"
          >
            刷新状态
          </button>
        </div>
        <Toast toast={toasts.claude ?? null} />
      </BackendCard>
    );
  })();

  const codexNode = (() => {
    const c = status.codex;
    return (
      <BackendCard title="Codex" loading={status.loading.codex}>
        <div className="flex flex-wrap items-center gap-1.5">
          <StatusChip
            ok={c ? c.installed : null}
            label={c?.installed ? `已安装${c.version ? ` ${c.version}` : ""}` : "未检测到"}
          />
          {c?.installed ? (
            <StatusChip
              ok={c.loggedIn}
              warn={!c.loggedIn}
              label={c.loggedIn ? `已登录${c.authMethod ? ` · ${c.authMethod}` : ""}` : "未登录"}
            />
          ) : null}
          {c?.defaultModel ? (
            <span className="text-xs text-zinc-500 dark:text-zinc-400">
              默认模型：{c.defaultModel}
              {c.defaultReasoningEffort ? ` · effort=${c.defaultReasoningEffort}` : ""}
            </span>
          ) : null}
        </div>
        {status.errors.codex ? (
          <p className="mt-2 text-xs text-rose-700 dark:text-rose-300">{status.errors.codex}</p>
        ) : null}
        <div className="mt-3 flex flex-wrap gap-2">
          <button
            type="button"
            disabled={busy.codex || !c?.installed}
            onClick={() =>
              runOp("codex", async () => {
                const r = await status.verifyCodex();
                return r.message ?? "OK";
              })
            }
            className="rounded-md border border-sky-400/50 bg-sky-500/10 px-2.5 py-1 text-xs font-medium text-sky-700 transition-all hover:bg-sky-500/20 disabled:cursor-not-allowed disabled:opacity-50 dark:border-sky-300/40 dark:text-sky-300"
          >
            {busy.codex ? "验证中…" : "验证连接"}
          </button>
          <button
            type="button"
            disabled={busy.codex}
            onClick={() => runOp("codex", () => status.openCodexLogin(false))}
            className="rounded-md border border-zinc-300/60 bg-white/40 px-2.5 py-1 text-xs font-medium text-zinc-700 transition-all hover:bg-white/70 disabled:cursor-not-allowed disabled:opacity-50 dark:border-white/10 dark:bg-slate-800/40 dark:text-zinc-200 dark:hover:bg-slate-800/70"
          >
            打开登录终端
          </button>
          <button
            type="button"
            disabled={busy.codex}
            onClick={() => runOp("codex", () => status.openCodexLogin(true))}
            className="rounded-md border border-zinc-300/60 bg-white/40 px-2.5 py-1 text-xs font-medium text-zinc-700 transition-all hover:bg-white/70 disabled:cursor-not-allowed disabled:opacity-50 dark:border-white/10 dark:bg-slate-800/40 dark:text-zinc-200 dark:hover:bg-slate-800/70"
          >
            设备码登录
          </button>
          <button
            type="button"
            disabled={status.loading.codex}
            onClick={() => status.refreshCodex()}
            className="rounded-md px-2.5 py-1 text-xs text-zinc-500 transition-colors hover:text-zinc-700 dark:text-zinc-400 dark:hover:text-zinc-200"
          >
            刷新状态
          </button>
        </div>
        <Toast toast={toasts.codex ?? null} />
      </BackendCard>
    );
  })();

  const opencodeNode = (() => {
    const o = status.opencode;
    return (
      <BackendCard title="OpenCode" loading={status.loading.opencode}>
        <div className="flex flex-wrap items-center gap-1.5">
          <StatusChip
            ok={o ? o.installed : null}
            label={o?.installed ? `已安装${o.version ? ` ${o.version}` : ""}` : "未检测到"}
          />
          {o?.installed ? (
            <StatusChip
              ok={o.running}
              warn={!o.running}
              label={o.running ? `运行中 · :${o.port}` : `未启动 · 端口 ${o.port}`}
            />
          ) : null}
          {o?.sessionId ? (
            <span className="text-xs text-zinc-500 dark:text-zinc-400">
              session：{o.sessionId.slice(0, 12)}…
            </span>
          ) : null}
        </div>
        {status.errors.opencode ? (
          <p className="mt-2 text-xs text-rose-700 dark:text-rose-300">{status.errors.opencode}</p>
        ) : null}
        <div className="mt-3 flex flex-wrap gap-2">
          {o?.running ? (
            <button
              type="button"
              disabled={busy.opencode}
              onClick={() => runOp("opencode", async () => {
                await status.stopOpencode();
                return "已停止";
              })}
              className="rounded-md border border-rose-400/50 bg-rose-500/10 px-2.5 py-1 text-xs font-medium text-rose-700 transition-all hover:bg-rose-500/20 disabled:cursor-not-allowed disabled:opacity-50 dark:border-rose-300/40 dark:text-rose-300"
            >
              {busy.opencode ? "处理中…" : "停止服务"}
            </button>
          ) : (
            <button
              type="button"
              disabled={busy.opencode || !o?.installed}
              onClick={() => runOp("opencode", async () => {
                const r = await status.startOpencode();
                return `已启动 · :${r.port}`;
              })}
              className="rounded-md border border-emerald-400/50 bg-emerald-500/10 px-2.5 py-1 text-xs font-medium text-emerald-700 transition-all hover:bg-emerald-500/20 disabled:cursor-not-allowed disabled:opacity-50 dark:border-emerald-300/40 dark:text-emerald-300"
            >
              {busy.opencode ? "处理中…" : "启动服务"}
            </button>
          )}
          <button
            type="button"
            disabled={status.loading.opencode}
            onClick={() => status.refreshOpencode()}
            className="rounded-md px-2.5 py-1 text-xs text-zinc-500 transition-colors hover:text-zinc-700 dark:text-zinc-400 dark:hover:text-zinc-200"
          >
            刷新状态
          </button>
        </div>
        <Toast toast={toasts.opencode ?? null} />
      </BackendCard>
    );
  })();

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold text-zinc-700 dark:text-zinc-200">Agent 后端</h3>
        <button
          type="button"
          onClick={() => status.refreshAll()}
          className="rounded-md px-2 py-0.5 text-xs text-zinc-500 transition-colors hover:bg-zinc-100/70 hover:text-zinc-700 dark:text-zinc-400 dark:hover:bg-slate-800/70 dark:hover:text-zinc-200"
        >
          全部刷新
        </button>
      </div>
      {claudeNode}
      {codexNode}
      {opencodeNode}
    </div>
  );
}
