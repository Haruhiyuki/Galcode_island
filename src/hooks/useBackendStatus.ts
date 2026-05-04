// 拉三个 backend 状态的 hook。
// SettingsModal 打开时调一次 refresh，verify/login_open/start/stop 之后也手动 refresh。
// 不做轮询——状态变化由用户操作触发，定时检查徒增 CLI 启动开销（version 缓存 5min，
// status 命令本身要 spawn 一次 CLI 拿 version）。

import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type {
  ClaudeStatus,
  CodexStatus,
  OpencodeStatus,
  VerifyResult,
} from "../types/backend";

interface BackendStatusState {
  claude: ClaudeStatus | null;
  codex: CodexStatus | null;
  opencode: OpencodeStatus | null;
  loading: {
    claude: boolean;
    codex: boolean;
    opencode: boolean;
  };
  errors: {
    claude?: string;
    codex?: string;
    opencode?: string;
  };
}

const initialState: BackendStatusState = {
  claude: null,
  codex: null,
  opencode: null,
  loading: { claude: false, codex: false, opencode: false },
  errors: {},
};

export function useBackendStatus(autoRefreshOn?: boolean) {
  const [state, setState] = useState<BackendStatusState>(initialState);
  // 避免组件卸载后还 setState
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const setIf = useCallback((updater: (prev: BackendStatusState) => BackendStatusState) => {
    if (mountedRef.current) {
      setState(updater);
    }
  }, []);

  const refreshClaude = useCallback(async () => {
    setIf((s) => ({ ...s, loading: { ...s.loading, claude: true } }));
    try {
      const status = await invoke<ClaudeStatus>("claude_status", {});
      setIf((s) => ({
        ...s,
        claude: status,
        loading: { ...s.loading, claude: false },
        errors: { ...s.errors, claude: undefined },
      }));
    } catch (error) {
      setIf((s) => ({
        ...s,
        loading: { ...s.loading, claude: false },
        errors: { ...s.errors, claude: String(error) },
      }));
    }
  }, [setIf]);

  const refreshCodex = useCallback(async () => {
    setIf((s) => ({ ...s, loading: { ...s.loading, codex: true } }));
    try {
      const status = await invoke<CodexStatus>("codex_status", {});
      setIf((s) => ({
        ...s,
        codex: status,
        loading: { ...s.loading, codex: false },
        errors: { ...s.errors, codex: undefined },
      }));
    } catch (error) {
      setIf((s) => ({
        ...s,
        loading: { ...s.loading, codex: false },
        errors: { ...s.errors, codex: String(error) },
      }));
    }
  }, [setIf]);

  const refreshOpencode = useCallback(async () => {
    setIf((s) => ({ ...s, loading: { ...s.loading, opencode: true } }));
    try {
      const status = await invoke<OpencodeStatus>("opencode_status", {});
      setIf((s) => ({
        ...s,
        opencode: status,
        loading: { ...s.loading, opencode: false },
        errors: { ...s.errors, opencode: undefined },
      }));
    } catch (error) {
      setIf((s) => ({
        ...s,
        loading: { ...s.loading, opencode: false },
        errors: { ...s.errors, opencode: String(error) },
      }));
    }
  }, [setIf]);

  const refreshAll = useCallback(() => {
    return Promise.allSettled([refreshClaude(), refreshCodex(), refreshOpencode()]);
  }, [refreshClaude, refreshCodex, refreshOpencode]);

  // verify / login / start / stop 操作（成功后自动 refresh 对应 backend）
  const verifyClaude = useCallback(async (): Promise<VerifyResult> => {
    const result = await invoke<VerifyResult>("claude_verify", {});
    refreshClaude();
    return result;
  }, [refreshClaude]);

  const verifyCodex = useCallback(async (): Promise<VerifyResult> => {
    const result = await invoke<VerifyResult>("codex_verify", {});
    refreshCodex();
    return result;
  }, [refreshCodex]);

  const openClaudeLogin = useCallback(async (): Promise<string> => {
    return invoke<string>("claude_login_open", {});
  }, []);

  const openCodexLogin = useCallback(async (deviceAuth?: boolean): Promise<string> => {
    return invoke<string>("codex_login_open", { deviceAuth: !!deviceAuth });
  }, []);

  const startOpencode = useCallback(
    async (cwd?: string) => {
      const result = await invoke<OpencodeStatus>("opencode_start", { cwd });
      setIf((s) => ({ ...s, opencode: result }));
      return result;
    },
    [setIf]
  );

  const stopOpencode = useCallback(async () => {
    const result = await invoke<OpencodeStatus>("opencode_stop", {});
    setIf((s) => ({ ...s, opencode: result }));
    return result;
  }, [setIf]);

  // 由调用方决定何时 autoRefresh（一般是 modal open 时 trigger）。
  useEffect(() => {
    if (autoRefreshOn) {
      void refreshAll();
    }
  }, [autoRefreshOn, refreshAll]);

  return {
    ...state,
    refreshClaude,
    refreshCodex,
    refreshOpencode,
    refreshAll,
    verifyClaude,
    verifyCodex,
    openClaudeLogin,
    openCodexLogin,
    startOpencode,
    stopOpencode,
  };
}
