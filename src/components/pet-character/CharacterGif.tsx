import { useEffect, useMemo, useState } from "react";
import { useAppStore } from "../../stores/useAppStore";

/* 凉宫桌宠动图：将 GIF 放到 public/pet/ 下与文件名一致即可（见 技术路径.txt） */
const PET_GIF: Record<string, string> = {
  init: "/pet/waking.gif",
  thinking: "/pet/thinking.gif",
  working: "/pet/working.gif",
  done: "/pet/happy.gif",
  error: "/pet/sad.gif",
  log: "/pet/ready.gif",
  default: "/pet/idle.gif",
};

const FALLBACK_SVG = "/pet/mascot.svg";

export function CharacterGif(): JSX.Element {
  const lastStage = useAppStore((s) => s.lastStage);
  const [layer, setLayer] = useState(0);
  const primary = useMemo(() => {
    return PET_GIF[lastStage] ?? PET_GIF.default;
  }, [lastStage]);

  useEffect(() => {
    setLayer(0);
  }, [primary]);

  if (layer >= 2) {
    return (
      <div className="flex h-full w-full items-center justify-center text-5xl select-none" aria-hidden>
        {"\u{1F916}"}
      </div>
    );
  }

  const src = layer === 0 ? primary : FALLBACK_SVG;

  return (
    <img
      src={src}
      alt={`桌宠状态: ${lastStage}`}
      className="h-full w-full object-contain"
      onError={() => setLayer((l) => l + 1)}
    />
  );
}
