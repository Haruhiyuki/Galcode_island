"""
Demo AI agent for galcode_island: prints JSONL progress then a structured result.
Run with: python -u scripts/demo_agent.py --task "your task"
"""

from __future__ import annotations

import argparse
import json
import sys
import time


def emit(obj: dict) -> None:
    print(json.dumps(obj, ensure_ascii=False))
    sys.stdout.flush()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--task", required=True, help="Task description (often English after translation)")
    args = parser.parse_args()

    emit({"stage": "init", "message": "收到任务，正在初始化 Demo Agent…", "percent": 5})
    time.sleep(0.15)
    emit({"stage": "thinking", "message": f"理解需求：{args.task[:120]}…", "percent": 25})
    time.sleep(0.2)
    emit({"stage": "working", "message": "生成示例输出（非真实代码执行）…", "percent": 55})
    time.sleep(0.25)
    emit({"stage": "working", "message": "检查格式与边界情况…", "percent": 80})
    time.sleep(0.15)

    output_en = (
        "Demo result:\n"
        "1) Restate the goal in one sentence.\n"
        "2) Propose a minimal file/layout structure.\n"
        "3) List 3 implementation steps with acceptance checks.\n"
        f"\n(Task received): {args.task}"
    )
    emit({"type": "result", "output_en": output_en})
    emit({"stage": "done", "message": "Demo Agent 完成。", "percent": 100})


if __name__ == "__main__":
    main()
