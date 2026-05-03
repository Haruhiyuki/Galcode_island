import React from "react";

type Props = { children: React.ReactNode };
type State = { error: Error | null };

/** Catches render errors so one bad update does not leave a blank WebView. */
export class AppErrorBoundary extends React.Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  render(): React.ReactNode {
    if (this.state.error) {
      const msg = this.state.error.message;
      const stack = this.state.error.stack ?? "";
      return (
        <div className="flex min-h-screen flex-col items-center justify-center gap-4 bg-slate-100 p-6 text-zinc-800 dark:bg-[#0B1120] dark:text-zinc-100">
          <p className="text-lg font-semibold">界面渲染出错</p>
          <p className="max-w-lg text-center text-sm text-zinc-600 dark:text-zinc-400">{msg}</p>
          <pre className="max-h-[45vh] max-w-full overflow-auto rounded-lg bg-black/5 p-4 text-left text-[11px] dark:bg-white/10">
            {stack}
          </pre>
          <button
            type="button"
            className="rounded-lg bg-sky-600 px-4 py-2 text-sm font-medium text-white hover:bg-sky-700"
            onClick={() => window.location.reload()}
          >
            重新加载
          </button>
        </div>
      );
    }
    return this.props.children;
  }
}
