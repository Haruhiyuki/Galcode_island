import { useAppStore } from "../../stores/useAppStore";
import { BubbleContainer } from "./BubbleContainer";
import { BubbleContent } from "./BubbleContent";
import { EmotionFeedback } from "./EmotionFeedback";

export function ChatBubble(): JSX.Element {
  const summaryText = useAppStore((s) => s.summaryText);
  const emotionText = useAppStore((s) => s.emotionText);
  const resultZh = useAppStore((s) => s.resultZh);
  const suggestion = useAppStore((s) => s.suggestion);

  return (
    <section className="flex flex-col gap-2 rounded-xl border border-zinc-300/70 bg-white/65 p-4 backdrop-blur dark:border-zinc-700/70 dark:bg-zinc-900/55">
      <BubbleContainer>
        {summaryText ? <BubbleContent title="总结" text={summaryText} /> : null}
        {emotionText ? <EmotionFeedback emotion={emotionText} /> : null}
        {resultZh ? <BubbleContent title="结果（中文）" text={resultZh} /> : null}
        {suggestion ? <BubbleContent title="建议" text={suggestion} /> : null}
      </BubbleContainer>
    </section>
  );
}
