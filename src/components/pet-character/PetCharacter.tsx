import { useMemo } from "react";
import { useAppStore } from "../../stores/useAppStore";
import { HaruhiStage, moodFromUi } from "./HaruhiStage";

export function PetCharacter(): JSX.Element {
  const uiState = useAppStore((s) => s.uiState);
  const lastStage = useAppStore((s) => s.lastStage);

  const mood = useMemo(() => moodFromUi(uiState, lastStage), [uiState, lastStage]);

  return (
    <div className="relative rounded-xl border border-zinc-300/70 bg-white/65 p-4 backdrop-blur dark:border-zinc-700/70 dark:bg-zinc-900/55">
      <HaruhiStage mood={mood} />
    </div>
  );
}
