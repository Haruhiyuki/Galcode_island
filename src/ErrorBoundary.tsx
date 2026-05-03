import { Component, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}

interface State {
  err: Error | null;
}

/** Catches render errors (e.g. Live2D / PIXI) so the window doesn’t stay blank. */
export class ErrorBoundary extends Component<Props, State> {
  override state: State = { err: null };

  static getDerivedStateFromError(err: Error): State {
    return { err };
  }

  override componentDidCatch(err: Error, info: ErrorInfo): void {
    console.error("[ErrorBoundary]", err, info.componentStack);
  }

  override render(): ReactNode {
    if (this.state.err) {
      return (
        <div
          style={{
            padding: 24,
            fontFamily: "system-ui, sans-serif",
            background: "#f5efdf",
            color: "#1a1a1a",
            minHeight: "100vh",
            boxSizing: "border-box",
          }}
        >
          <h1 style={{ fontSize: 18, marginBottom: 12 }}>界面加载出错</h1>
          <pre
            style={{
              whiteSpace: "pre-wrap",
              fontSize: 12,
              background: "rgba(0,0,0,0.06)",
              padding: 12,
              borderRadius: 8,
            }}
          >
            {this.state.err.message}
            {"\n\n"}
            {this.state.err.stack}
          </pre>
          <p style={{ fontSize: 12, marginTop: 16, opacity: 0.8 }}>
            若为桌宠 Live2D / WebGL 导致，可暂时从 MainView 移除 PetCharacter 或更新显卡驱动后再试。
          </p>
        </div>
      );
    }
    return this.props.children;
  }
}
