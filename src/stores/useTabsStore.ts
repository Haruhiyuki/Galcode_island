// 多 tab store —— 每个 tab 一份独立的会话状态（项目路径 / 输入框 / agent 状态 /
// 流式 blocks / 总结结果），互不干扰。
//
// 设计参考 designcode 的 `composables/useTabs.js`：
//   - tabs 用 Record<id, slice>（不是 Map）方便 zustand selector 浅比较
//   - order 单独存一个数组，TabBar 按 order 渲染
//   - activeTabId 是当前显示哪个 tab；切 tab 用 setActiveTab(id)
//   - createTab 返回新 id，调用方拿到后立即 setActiveTab
//
// run_id 跟 tab_id 是同一个标识：前端创建 tab 时生成 UUID，传给后端
// `start_agent({ runId })`，后端所有事件 payload 也带 runId，前端按它路由
// 到对应 tab slice。

import { create } from "zustand";
import type {
  AgentStatus,
  AgentType,
  LastStage,
  UiState,
} from "../types/agent";
import type { CliBlock } from "../types/blocks";

const MAX_BLOCKS = 200;
const TRIM_TARGET = 180;

/// 单个 tab 的完整状态。每个字段都是会话级 —— tab 之间完全隔离。
export interface TabState {
  id: string;
  /// 显示在 TabBar 的标题；用户可重命名；默认从 task / projectPath basename 推
  title: string;
  /// 该 tab 下次启动时用哪个 backend；可单独切换不影响其他 tab
  agent: AgentType;
  /// 该 tab 的工作目录；为 null 表示还没选
  projectPath: string | null;
  /// 输入框文本
  task: string;
  /// 后端最近 emit 的 agentStatus（idle/running/thinking/...）
  agentStatus: AgentStatus;
  /// UI 阶段（idle/running/done/error/suggesting）
  uiState: UiState;
  /// 桌宠/总结 mode（idle/working/thinking/complete/suggestion/error）
  mode: string;
  /// RunningBubble 显示的当前提示（来自工具描述 / 进度文案）
  bubble: string;
  /// 进度条百分比
  percent: number;
  /// 后端 session_id（claude/codex/opencode 各自的会话标识，用于续接）
  sessionId: string | null;
  /// 完成后的中文翻译输出
  resultZh: string;
  /// 凉宫春日总结
  summaryTranslation: string;
  /// 凉宫春日的语气短句
  emotionText: string;
  /// 完成后给的下一步选项按钮
  suggestionOptions: string[];
  /// 影响 PetCharacter GIF 选择的最终阶段
  lastStage: LastStage;
  /// 流式 blocks（BlockStream 渲染源）
  cliBlocks: CliBlock[];
  /// 非活动 tab 完成后置 true，TabBar 显示小红点；切到该 tab 自动清掉
  hasUnread: boolean;
  /// 创建时间戳，用来排序 / 关闭时回退到上一个
  createdAt: number;
}

interface TabsStoreState {
  tabs: Record<string, TabState>;
  /// TabBar 显示顺序；新建的追加到末尾；关闭时从这里 splice
  order: string[];
  activeTabId: string | null;

  /// 创建一个新 tab。init 可指定初始字段（agent / projectPath / 标题等），
  /// 返回新 tab 的 id；不会自动切到新 tab，调用方如需切换显式调 setActiveTab。
  createTab: (init?: Partial<Omit<TabState, "id" | "createdAt">>) => string;
  /// 关闭 tab；如果是当前 active 会自动切到相邻 tab；最后一个 tab 关闭后
  /// activeTabId 变成 null（调用方应处理这种状态，比如回 WelcomeView）。
  removeTab: (id: string) => void;
  /// 切换当前活动 tab；切到时自动清 hasUnread 标记。
  setActiveTab: (id: string) => void;
  /// 局部更新指定 tab 字段。不存在的 tab 直接忽略（不 throw，避免 IPC 事件
  /// 撞到刚关闭的 tab 时把 store 弄坏）。
  updateTab: (id: string, patch: Partial<TabState>) => void;
  /// 重置某个 tab 的会话级字段（保留 projectPath / agent / title / id），
  /// 用于"清屏"或开始新一轮 turn 前的清理。
  resetTabSession: (id: string) => void;

  // CliBlock 操作 —— 跟旧 useAppStore 的语义一致，但每次都按 tab 路由
  appendCliBlock: (id: string, block: CliBlock) => void;
  upsertCliBlock: (id: string, block: CliBlock) => void;
  clearCliBlocks: (id: string) => void;

  /// 多 tab 路由辅助：根据后端事件 payload 的 runId 找到 tab id；
  /// runId 直接对应 tab.id，所以这里其实就是看 tabs[runId] 是否存在。
  hasTab: (runId: string) => boolean;
  /// 通过 sessionId 反查 runId（IPC 事件早期还没回填 runId 时用）。
  findTabBySessionId: (sessionId: string) => string | null;
}

function generateTabId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  // fallback：足够强的伪随机；只在 SSR / 老环境 fallback
  return `tab-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

function makeDefaultTab(init?: Partial<Omit<TabState, "id" | "createdAt">>): TabState {
  return {
    id: "",
    title: init?.title ?? "新会话",
    agent: init?.agent ?? "claude-code",
    projectPath: init?.projectPath ?? null,
    task: init?.task ?? "",
    agentStatus: init?.agentStatus ?? "idle",
    uiState: init?.uiState ?? "idle",
    mode: init?.mode ?? "idle",
    bubble: init?.bubble ?? "",
    percent: init?.percent ?? 0,
    sessionId: init?.sessionId ?? null,
    resultZh: init?.resultZh ?? "",
    summaryTranslation: init?.summaryTranslation ?? "",
    emotionText: init?.emotionText ?? "",
    suggestionOptions: init?.suggestionOptions ?? [],
    lastStage: init?.lastStage ?? "default",
    cliBlocks: init?.cliBlocks ?? [],
    hasUnread: init?.hasUnread ?? false,
    createdAt: 0,
  };
}

export const useTabsStore = create<TabsStoreState>((set, get) => ({
  tabs: {},
  order: [],
  activeTabId: null,

  createTab: (init) => {
    const id = generateTabId();
    const tab: TabState = {
      ...makeDefaultTab(init),
      id,
      createdAt: Date.now(),
    };
    set((state) => ({
      tabs: { ...state.tabs, [id]: tab },
      order: [...state.order, id],
    }));
    return id;
  },

  removeTab: (id) => {
    set((state) => {
      if (!state.tabs[id]) return state;
      const nextTabs = { ...state.tabs };
      delete nextTabs[id];
      const nextOrder = state.order.filter((tid) => tid !== id);
      let nextActive = state.activeTabId;
      if (state.activeTabId === id) {
        // 切到被关闭 tab 在 order 中的前一个；都没了就 null
        const idx = state.order.indexOf(id);
        nextActive = nextOrder[Math.max(0, idx - 1)] ?? nextOrder[0] ?? null;
      }
      return { tabs: nextTabs, order: nextOrder, activeTabId: nextActive };
    });
  },

  setActiveTab: (id) => {
    set((state) => {
      if (!state.tabs[id]) return state;
      const tab = state.tabs[id];
      // 切过来时自动清未读标记
      const nextTabs = tab.hasUnread
        ? { ...state.tabs, [id]: { ...tab, hasUnread: false } }
        : state.tabs;
      return { activeTabId: id, tabs: nextTabs };
    });
  },

  updateTab: (id, patch) => {
    set((state) => {
      const tab = state.tabs[id];
      if (!tab) return state;
      return { tabs: { ...state.tabs, [id]: { ...tab, ...patch } } };
    });
  },

  resetTabSession: (id) => {
    set((state) => {
      const tab = state.tabs[id];
      if (!tab) return state;
      return {
        tabs: {
          ...state.tabs,
          [id]: {
            ...tab,
            task: "",
            agentStatus: "idle",
            uiState: "idle",
            mode: "idle",
            bubble: "",
            percent: 0,
            sessionId: null,
            resultZh: "",
            summaryTranslation: "",
            emotionText: "",
            suggestionOptions: [],
            lastStage: "default",
            cliBlocks: [],
            hasUnread: false,
          },
        },
      };
    });
  },

  appendCliBlock: (id, block) => {
    set((state) => {
      const tab = state.tabs[id];
      if (!tab) return state;
      const next = [...tab.cliBlocks, block];
      const trimmed = next.length > MAX_BLOCKS ? next.slice(-TRIM_TARGET) : next;
      return { tabs: { ...state.tabs, [id]: { ...tab, cliBlocks: trimmed } } };
    });
  },

  upsertCliBlock: (id, block) => {
    set((state) => {
      const tab = state.tabs[id];
      if (!tab) return state;
      const idx = tab.cliBlocks.findIndex((b) => b.id === block.id);
      let next: CliBlock[];
      if (idx >= 0) {
        next = tab.cliBlocks.slice();
        next[idx] = { ...next[idx], ...block };
      } else {
        next = [...tab.cliBlocks, block];
      }
      const trimmed = next.length > MAX_BLOCKS ? next.slice(-TRIM_TARGET) : next;
      return { tabs: { ...state.tabs, [id]: { ...tab, cliBlocks: trimmed } } };
    });
  },

  clearCliBlocks: (id) => {
    set((state) => {
      const tab = state.tabs[id];
      if (!tab) return state;
      return { tabs: { ...state.tabs, [id]: { ...tab, cliBlocks: [] } } };
    });
  },

  hasTab: (runId) => Boolean(get().tabs[runId]),

  findTabBySessionId: (sessionId) => {
    const { tabs } = get();
    for (const tab of Object.values(tabs)) {
      if (tab.sessionId === sessionId) return tab.id;
    }
    return null;
  },
}));
