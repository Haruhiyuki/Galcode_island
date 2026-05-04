// 订阅 `galcode://cli-output` 事件，把后端 emit 的 block JSON 归一成
// 统一的 CliBlock 列表，按 id 去重（后续事件是 update，覆盖前一个）。
//
// 流式 update 模式：
//   - text/thought/agentMessage 等增量类 block：后端发一连串同 id 的更新，content
//     是累积后的完整文本，前端直接覆盖即可（不要 push 多条）
//   - command output delta：output 字段累积更新
//   - todo：items 数组覆盖
//
// 自动切换会话：当 useAppStore.sessionId 变化时清空旧 blocks，避免上一轮的
// 流串到下一轮显示。

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { useAppStore } from "../stores/useAppStore";
import type { CliBlock, CliStreamEvent } from "../types/blocks";

// thought 出现时让宠物切到 thinking 表情。session-complete 会自然把 mode 重置。
function notifyThoughtToPet(): void {
  const setMode = useAppStore.getState().setMode;
  setMode("thinking");
}

let stderrCounter = 0;

interface RawWrappedBlock {
  type: "galcode.block";
  block: Partial<CliBlock>;
}

interface RawOpencodeEvent {
  type: string;             // "opencode.tool" / "opencode.file" / "opencode.status" / "opencode.error"
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

  // 包装格式：{ type: "galcode.block", block: {...} }
  if (topType === "galcode.block" && obj.block && typeof obj.block === "object") {
    const wrapped = (parsed as RawWrappedBlock).block;
    if (!wrapped.id || !wrapped.type) return null;
    return {
      ...wrapped,
      id: wrapped.id,
      type: wrapped.type as CliBlock["type"],
    } as CliBlock;
  }

  // OpenCode 专属：{ type: "opencode.xxx", ... }
  if (typeof topType === "string" && topType.startsWith("opencode.")) {
    const ev = parsed as RawOpencodeEvent;
    const subType = topType.slice("opencode.".length); // tool / file / status / error
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

export function useCliStream(): { blocks: CliBlock[] } {
  const [blocks, setBlocks] = useState<CliBlock[]>([]);
  const sessionIdRef = useRef<string | null>(null);

  // 当前会话变化时清空 block 列表（避免上轮流串到下轮）
  useEffect(() => {
    return useAppStore.subscribe((state) => {
      if (state.sessionId !== sessionIdRef.current) {
        sessionIdRef.current = state.sessionId;
        setBlocks([]);
      }
    });
  }, []);

  useEffect(() => {
    const unsubs: UnlistenFn[] = [];
    const run = async () => {
      const unsub = await listen<CliStreamEvent>("galcode://cli-output", (e) => {
        const payload = e.payload;
        if (!payload?.line) return;

        // stderr 行：直接包成 stderr block（不尝试 JSON 解析，多半是诊断 / warning）
        if (payload.channel === "stderr") {
          const trimmed = payload.line.trim();
          if (!trimmed) return;
          stderrCounter += 1;
          const block: CliBlock = {
            id: `stderr-${payload.streamId || payload.backend}-${stderrCounter}`,
            type: "stderr",
            backend: payload.backend,
            channel: "stderr",
            message: trimmed,
          };
          setBlocks((prev) => {
            const appended = [...prev, block];
            return appended.length > 200 ? appended.slice(-180) : appended;
          });
          return;
        }

        let parsed: unknown;
        try {
          parsed = JSON.parse(payload.line);
        } catch {
          return; // stdout 上的非 JSON 行（如 OpenCode "Turn started"）忽略
        }

        const block = normalizeBlock(parsed);
        if (!block) return;

        // thought 类型：联动桌宠切 thinking 表情
        if (block.type === "thought") {
          notifyThoughtToPet();
        }

        setBlocks((prev) => {
          const idx = prev.findIndex((b) => b.id === block.id);
          if (idx >= 0) {
            // 同 id 的后续事件覆盖（流式 update）
            const next = prev.slice();
            next[idx] = { ...prev[idx], ...block };
            return next;
          }
          // 新 block：限制总数（防止内存爆炸 / DOM 卡顿）
          const appended = [...prev, block];
          if (appended.length > 200) {
            return appended.slice(-180);
          }
          return appended;
        });
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

  return { blocks };
}
