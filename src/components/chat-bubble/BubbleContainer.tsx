import type { ReactNode } from "react";

interface BubbleContainerProps {
  children: ReactNode;
}

export function BubbleContainer({ children }: BubbleContainerProps): JSX.Element {
  return <div className="flex flex-col gap-2 text-sm">{children}</div>;
}
