# Smoke-test Demo Agent stdout (JSONL) without Tauri.
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
& python -u (Join-Path $root "scripts\demo_agent.py") --task "Hello from smoke test"
