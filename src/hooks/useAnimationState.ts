import type { AgentStatus } from "../types/agent";

const STATUS_GIF_MAP: Record<AgentStatus, string> = {
  idle: "/pet/idle.gif",
  starting: "/pet/waking.gif",
  running: "/pet/ready.gif",
  thinking: "/pet/thinking.gif",
  processing: "/pet/working.gif",
  waitingApproval: "/pet/question.gif",
  completed: "/pet/happy.gif",
  error: "/pet/sad.gif",
};

export function useAnimationState(status: AgentStatus): string {
  return STATUS_GIF_MAP[status];
}
