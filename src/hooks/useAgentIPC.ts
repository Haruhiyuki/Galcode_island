import { useEffect } from "react";

export function useAgentIPC(): void {
  useEffect(() => {
    // Phase 1 仅保留占位，Phase 2 开始接入 listen/invoke。
    return () => {};
  }, []);
}
