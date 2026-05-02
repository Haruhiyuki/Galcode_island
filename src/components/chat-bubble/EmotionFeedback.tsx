interface EmotionFeedbackProps {
  emotion: string;
}

export function EmotionFeedback({ emotion }: EmotionFeedbackProps): JSX.Element {
  return (
    <div className="max-h-18 overflow-y-auto rounded-lg border border-zinc-300/50 bg-white/45 p-3 dark:border-zinc-700/50 dark:bg-zinc-900/35">
      <strong className="text-xs text-zinc-500 dark:text-zinc-400">情绪反馈</strong>
      <div className="mt-1 text-zinc-700 dark:text-zinc-200">{emotion}</div>
    </div>
  );
}
