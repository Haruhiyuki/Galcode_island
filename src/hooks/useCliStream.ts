// 订阅 `galcode://cli-output` 事件，把后端 emit 的 block JSON 归一成
// CliBlock 写到对应 tab 的 cliBlocks。
//
// **关键**：这个 hook 只在 App 顶层 mount 一次，listener 永久有效。
// 之前在 BlockStream 里 mount，组件随 StatusMonitor 显示/隐藏反复重建
// listener，会丢早到的事件。
//
// 多 tab 路由：CliStreamEvent.runId 直接是 tab.id；找不到对应 tab 时
// 兜底通过 streamId 反查（streamId 格式 `stream-{sessionId}`，前端从
// tab.sessionId 推导）。再找不到只剩单 tab fallback（同 useAgentIPC）。
//
// 流式 update 模式：
//   - text/thought/agentMessage 等增量类 block：后端发一连串同 id 的更新，content
//     是累积后的完整文本，前端直接 upsert（覆盖前一个）
//   - command output delta：output 字段累积更新
//   - todo：items 数组覆盖

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { useTabsStore } from "../stores/useTabsStore";
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

/// 多 tab 路由：runId → tab.id 直接对应；没拿到 runId 时通过 streamId
/// 反查（streamId = `stream-{sessionId}`）；都没命中走单 tab 兜底。
function resolveTabId(runId: string, streamId: string): string | null {
  const store = useTabsStore.getState();
  if (runId && store.tabs[runId]) return runId;

  if (streamId && streamId.startsWith("stream-")) {
    const sessionId = streamId.slice("stream-".length);
    const byId = store.findTabBySessionId(sessionId);
    if (byId) return byId;
  }

  // 单 tab 兜底
  if (store.order.length === 1) return store.order[0];
  return null;
}

/// 在 App 顶层挂一次。
export function useCliStream(): void {
  useEffect(() => {
    const unsubs: UnlistenFn[] = [];
    const run = async () => {
      console.log("[cli-stream] listener registered (App-level)");
      const unsub = await listen<CliStreamEvent>("galcode://cli-output", (e) => {
        const payload = e.payload;
        if (!payload?.line) return;

        const tabId = resolveTabId(payload.runId ?? "", payload.streamId ?? "");
        if (!tabId) {
          console.warn(
            "[cli-stream] dropped, no tab match",
            "runId=", payload.runId,
            "streamId=", payload.streamId,
          );
          return;
        }

        const tabs = useTabsStore.getState();

        if (payload.channel === "stderr") {
          const trimmed = payload.line.trim();
          if (!trimmed) return;
          stderrCounter += 1;
          tabs.appendCliBlock(tabId, {
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
        if (!block) return;

        if (block.type === "thought") {
          tabs.updateTab(tabId, { mode: "thinking" });
        }
        tabs.upsertCliBlock(tabId, block);
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
