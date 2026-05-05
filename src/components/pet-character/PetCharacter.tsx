import { useCallback, useEffect, useMemo, useState } from "react";
import { motion } from "framer-motion";
import { useActiveTab } from "../../hooks/useActiveTab";
import type { AgentStatus } from "../../types/agent";

const OTHERS_GIFS: string[] = [
  "/pet/others/对手指.gif",
  "/pet/others/thinking_2.gif",
  "/pet/others/启动.gif",
  "/pet/others/想要.gif",
  "/pet/others/戳戳.gif",
  "/pet/others/thinking_1.gif",
];

type PetVisualState = "thinking" | "complete" | "error" | "waiting" | "welcome";

function getVisualState(uiState: string, mode: string, agentStatus: AgentStatus): PetVisualState {
  if (uiState === "error" || mode === "error") return "error";
  if (uiState === "done" || mode === "complete") return "complete";
  if (uiState === "running" || mode === "thinking" || mode === "working") return "thinking";
  if (agentStatus === "idle" && uiState === "idle") return "welcome";
  return "waiting";
}

function pickRandomDefaultGif(state: PetVisualState): string {
  if (state === "welcome") return "/pet/welcome/welcome.gif";
  const maxMap: Record<string, number> = {
    thinking: 2,
    complete: 3,
    waiting: 2,
    error: 2,
  };
  const max = maxMap[state] || 1;
  const n = Math.floor(Math.random() * max) + 1;
  return `/pet/${state}/${state}_${n}.gif`;
}

function pickRandomOthersGif(): string {
  return OTHERS_GIFS[Math.floor(Math.random() * OTHERS_GIFS.length)];
}

const THINKING_STATUSES: ReadonlySet<AgentStatus> = new Set<AgentStatus>([
  "idle",
  "starting",
  "running",
  "thinking",
  "processing",
]);

export function PetCharacter(): JSX.Element {
  const tab = useActiveTab();
  const uiState = tab.uiState;
  const agentStatus = tab.agentStatus;
  const mode = tab.mode;

  const visualState = useMemo(
    () => getVisualState(uiState, mode, agentStatus),
    [uiState, mode, agentStatus],
  );

  const [displayGif, setDisplayGif] = useState<string>(() =>
    pickRandomDefaultGif(visualState),
  );
  const canSwapExpression = THINKING_STATUSES.has(agentStatus);

  // Reset to default GIF whenever system state changes
  useEffect(() => {
    setDisplayGif(pickRandomDefaultGif(visualState));
  }, [visualState, uiState, agentStatus, mode]);

  const handleClick = useCallback(() => {
    if (canSwapExpression) {
      setDisplayGif(pickRandomOthersGif());
    }
  }, [canSwapExpression]);

  return (
    <motion.div
      whileHover={{ scale: 1.03 }}
      whileTap={{ scale: 0.9 }}
      transition={{
        type: "spring",
        damping: 10,
        stiffness: 400,
        mass: 0.7,
      }}
      onClick={handleClick}
      className="relative flex h-52 w-52 cursor-pointer select-none items-center justify-center"
      role="img"
      aria-label="桌宠角色"
    >
      <motion.img
        key={displayGif}
        src={displayGif}
        alt="桌宠"
        initial={{ opacity: 0, scale: 0.85 }}
        animate={{ opacity: 1, scale: 1 }}
        transition={{ duration: 0.2, ease: "easeOut" }}
        className="h-40 w-40 object-contain drop-shadow-xl"
        draggable={false}
      />
    </motion.div>
  );
}
