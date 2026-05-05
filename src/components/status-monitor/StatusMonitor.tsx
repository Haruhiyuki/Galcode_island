// 任务流：单一干净的滚动文本区，没有外层装饰、状态条、进度条、日志框这些
// 重复 / 抢空间的子组件。流式过程 + 最终总结 + 建议按钮 都在 BlockStream 里。
// AgentStatusBadge / TodoProgress / LogStream 已下线（前两个跟 PetCharacter 状态
// 重复，LogStream 在事件管线收敛后没人 push 了）。

import { BlockStream } from "./BlockStream";

export function StatusMonitor(): JSX.Element {
  return (
    <div className="h-full min-h-0">
      <BlockStream />
    </div>
  );
}
