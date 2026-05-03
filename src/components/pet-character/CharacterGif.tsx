import { useMemo } from "react";
import { useAppStore } from "../../stores/useAppStore";

const PET_GIF_MAP: Record<string, string[]> = {
  init: ["/pet/thinking/thinking_1.gif"],
  thinking: ["/pet/thinking/thinking_1.gif", "/pet/thinking/thinking_2.gif"],
  working: ["/pet/thinking/thinking_1.gif"],
  done: ["/pet/complete/complete_1.gif", "/pet/complete/complete_2.gif"],
  error: ["/pet/error/error_1.gif", "/pet/error/error_2.gif"],
  default: ["/pet/waiting/waiting_1.gif"],
};

export function CharacterGif(): JSX.Element {
  const lastStage = useAppStore((s) => s.lastStage);

  const src = useMemo(() => {
    const candidates = PET_GIF_MAP[lastStage] ?? PET_GIF_MAP.default;
    return candidates[0];
  }, [lastStage]);

  return (
    <img
      src={src}
      alt={`桌宠状态: ${lastStage}`}
      className="h-full w-full object-contain"
    />
  );
}
