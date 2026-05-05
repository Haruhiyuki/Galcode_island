// 订阅 `galcode://cli-output` 事件，把后端 emit 的 block JSON 归一成
// 统一的 CliBlock 列表，写到 useAppStore.cliBlocks（全局 store）。
//
// **关键**：这个 hook 只在 App 顶层 mount 一次，listener 永久有效。
// 之前在 BlockStream 里 mount，组件随 StatusMonitor 显示/隐藏反复重建
// listener，会丢早到的事件。
//
// 流式 update 模式：
//   - text/thought/agentMessage 等增量类 block：后端发一连串同 id 的更新，content
//     是累积后的完整文本，前端直接 upsert（覆盖前一个）
//   - command output delta：output 字段累积更新
//   - todo：items 数组覆盖
//
// sessionId 切换时清空旧 blocks（在 InputBubble.handleLaunch 里 setSessionId(null)
// 触发，下面的 store subscriber 会自动 clearCliBlocks）。

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import { useAppStore } from "../stores/useAppStore";
import type { CliBlock, CliStreamEvent } from "../types/blocks";

let stderrCounter = 0;

interface RawWrappedBlock {
  type: "galcode.block";
  block: Partial<CliBlock>;
}

interface RawOpencodeEvent {
  type: string;
  id?: string;
  tool?: string;
  path?: string;
  detail?: string;
  message?: string;
  status?: string;
  suppressLogLine?: boolean;
}

function normalizeBlock(parsed: unknown): CliBlock | null {
  if (!parsed || typeof parsed !== "object") return null;
  const obj = parsed as Record<string, unknown>;
  const topType = obj.type as string | undefined;

  if (topType === "galcode.block" && obj.block && typeof obj.block === "object") {
    const wrapped = (parsed as RawWrappedBlock).block;
    if (!wrapped.id || !wrapped.type) return null;
    return {
      ...wrapped,
      id: wrapped.id,
      type: wrapped.type as CliBlock["type"],
    } as CliBlock;
  }

  if (typeof topType === "string" && topType.startsWith("opencode.")) {
    const ev = parsed as RawOpencodeEvent;
    const subType = topType.slice("opencode.".length);
    if (subType !== "tool" && subType !== "file" && subType !== "status" && subType !== "error") {
      return null;
    }
    const id = ev.id ?? `opencode-${subType}-${Date.now()}`;
    return {
      id,
      type: subType as CliBlock["type"],
      backend: "opencode",
      suppressLogLine: ev.suppressLogLine,
      tool: ev.tool,
      path: ev.path,
      detail: ev.detail,
      message: ev.message,
      status: ev.status,
    };
  }

  return null;
}

/// 在 App 顶层挂一次。
export function useCliStream(): void {
  const sessionIdRef = useRef<string | null>(null);

  useEffect(() => {
    return useAppStore.subscribe((state) => {
      if (state.sessionId !== sessionIdRef.current) {
        sessionIdRef.current = state.sessionId;
        useAppStore.getState().clearCliBlocks();
      }
    });
  }, []);

  useEffect(() => {
    const unsubs: UnlistenFn[] = [];
    const run = async () => {
      console.log("[cli-stream] listener registered (App-level)");
      const unsub = await listen<CliStreamEvent>("galcode://cli-output", (e) => {
        const payload = e.payload;
        if (!payload?.line) return;

        console.log(
          "[cli-stream]",
          payload.backend,
          payload.channel,
          "len:",
          payload.line.length,
          "preview:",
          payload.line.slice(0, 100)
        );

        if (payload.channel === "stderr") {
          const trimmed = payload.line.trim();
          if (!trimmed) return;
          stderrCounter += 1;
          useAppStore.getState().appendCliBlock({
            id: `stderr-${payload.streamId || payload.backend}-${stderrCounter}`,
            type: "stderr",
            backend: payload.backend,
            channel: "stderr",
            message: trimmed,
          });
          return;
        }

        let parsed: unknown;
        try {
          parsed = JSON.parse(payload.line);
        } catch {
          return;
        }

        const block = normalizeBlock(parsed);
        if (!block) {
          const t = (parsed as { type?: string })?.type;
          if (t) console.log("[cli-stream] skipped JSON, top type=", t);
          return;
        }

        console.log("[cli-stream] block accepted:", block.type, block.id);
        if (block.type === "thought") {
          useAppStore.getState().setMode("thinking");
        }
        useAppStore.getState().upsertCliBlock(block);
      });
      unsubs.push(unsub);
    };
    void run();
    return () => {
      for (const u of unsubs) {
        try {
          u();
        } catch {
          /* noop */
        }
      }
    };
  }, []);
}
