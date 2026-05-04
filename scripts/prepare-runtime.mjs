#!/usr/bin/env node
// 把 Claude Code / OpenCode / Codex 三个 CLI 的 prebuilt binary 拉到
// src-tauri/resources/runtime/<platform>-<arch>/<kind>/<binary>
// 供 Tauri bundle 打进 .app/.dmg/.msi。dev 模式不需要跑这个。
//
// 用法：
//   node scripts/prepare-runtime.mjs              # 当前平台
//   node scripts/prepare-runtime.mjs --skip-claude  # 跳过 claude（许可证敏感时）
//
// 设计：
//   - 用 npm install 到临时目录，避免污染主项目 node_modules
//   - 每个 CLI 在它自己的 npm 包里有平台相关的 binary subpackage
//     (例: @opencode-ai/opencode-darwin-arm64 / @openai/codex-darwin-arm64)
//   - 找到 binary 后复制到 resources/runtime/<key>/<kind>/<binary>
//   - chmod 0o755（Unix 上 npm 包里 binary 默认就是可执行的，保险起见）
//   - 失败容错：单个 CLI 失败不影响其它两个

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, statSync } from "node:fs";
import { chmod, copyFile, mkdir, mkdtemp, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, "..");

const platformMap = { darwin: "darwin", linux: "linux", win32: "windows" };
const archMap = { x64: "x64", arm64: "arm64" };

const platform = platformMap[process.platform] || process.platform;
const arch = archMap[process.arch] || process.arch;
const runtimeKey = `${platform}-${arch}`;
const runtimeDir = path.join(rootDir, "src-tauri", "resources", "runtime", runtimeKey);
const isWindows = platform === "windows";

const args = process.argv.slice(2);
const skipClaude = args.includes("--skip-claude");
const skipCodex = args.includes("--skip-codex");
const skipOpencode = args.includes("--skip-opencode");

function binaryName(base) {
  return isWindows ? `${base}.exe` : base;
}

async function ensureDir(dir) {
  await mkdir(dir, { recursive: true });
}

function npmInstall(tmpDir, packageName) {
  // --no-save / --no-package-lock：不污染 lockfile
  // --prefix tmpDir：装到 tmp 目录里
  console.log(`  → npm install ${packageName}`);
  const result = spawnSync(
    "npm",
    ["install", "--no-save", "--no-package-lock", "--no-fund", "--no-audit", "--prefix", tmpDir, packageName],
    { stdio: "inherit", encoding: "utf8" }
  );
  if (result.status !== 0) {
    throw new Error(`npm install ${packageName} failed (exit ${result.status})`);
  }
}

function findBinaryRecursive(startDir, fileName, maxDepth = 6) {
  // 平台 subpackage 一般在 node_modules/<scope>/<pkg>-<platform>-<arch>/bin/
  // 不同包结构不一样，递归找最稳妥
  if (!existsSync(startDir)) return null;
  const stack = [{ dir: startDir, depth: 0 }];
  while (stack.length) {
    const { dir, depth } = stack.pop();
    if (depth > maxDepth) continue;
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      const full = path.join(dir, entry.name);
      if (entry.isFile() && entry.name === fileName) {
        // 排除明显的 launcher 脚本（参考项目踩过坑：node_modules/.bin/<name> 是 shim）
        try {
          const stat = statSync(full);
          if (stat.size > 1024 * 100) {
            // 100KB+ 大概率是真 binary 不是 shim
            return full;
          }
        } catch {}
      }
      if (entry.isDirectory()) {
        stack.push({ dir: full, depth: depth + 1 });
      }
    }
  }
  return null;
}

async function stageBinary(kind, sourcePath) {
  const destDir = path.join(runtimeDir, kind);
  await ensureDir(destDir);
  const dest = path.join(destDir, binaryName(kind));
  await copyFile(sourcePath, dest);
  if (!isWindows) {
    await chmod(dest, 0o755);
  }
  console.log(`  ✓ ${kind} → ${path.relative(rootDir, dest)}`);
  return dest;
}

async function prepareOpencode(tmpDir) {
  if (skipOpencode) {
    console.log("[opencode] skipped");
    return;
  }
  console.log("[opencode]");
  npmInstall(tmpDir, "opencode-ai");
  const fileName = binaryName("opencode");
  const found = findBinaryRecursive(path.join(tmpDir, "node_modules"), fileName);
  if (!found) {
    throw new Error(`opencode binary not found under ${tmpDir}/node_modules`);
  }
  await stageBinary("opencode", found);
}

async function prepareCodex(tmpDir) {
  if (skipCodex) {
    console.log("[codex] skipped");
    return;
  }
  console.log("[codex]");
  npmInstall(tmpDir, "@openai/codex");
  const fileName = binaryName("codex");
  const found = findBinaryRecursive(path.join(tmpDir, "node_modules"), fileName);
  if (!found) {
    throw new Error(`codex binary not found under ${tmpDir}/node_modules`);
  }
  await stageBinary("codex", found);
}

async function prepareClaude(tmpDir) {
  if (skipClaude) {
    console.log("[claude] skipped (use --skip-claude / Anthropic 专有许可证敏感时建议跳过)");
    return;
  }
  console.log("[claude]");
  npmInstall(tmpDir, "@anthropic-ai/claude-code");
  const fileName = binaryName("claude");
  const found = findBinaryRecursive(path.join(tmpDir, "node_modules"), fileName);
  if (!found) {
    // claude-code 可能是 node 启动的 js 脚本，不是 native binary —— 这种情况也需要 node bundle
    // 当前简化版策略：找不到就跳过，让用户自己安装
    console.warn(`  ⚠ claude binary not found — Claude Code 看起来是 node 脚本而非原生 binary，暂未 bundle，用户需自行 npm i -g @anthropic-ai/claude-code`);
    return;
  }
  await stageBinary("claude", found);
}

async function main() {
  console.log(`Preparing runtime for ${runtimeKey} → ${path.relative(rootDir, runtimeDir)}`);

  // 临时安装目录用 OS tmp，避免污染主项目
  const tmpDir = await mkdtemp(path.join(os.tmpdir(), "galcode-runtime-"));
  console.log(`(tmp install: ${tmpDir})`);

  try {
    const tasks = [
      ["opencode", () => prepareOpencode(tmpDir)],
      ["codex", () => prepareCodex(tmpDir)],
      ["claude", () => prepareClaude(tmpDir)],
    ];
    let failed = 0;
    for (const [name, fn] of tasks) {
      try {
        await fn();
      } catch (error) {
        failed += 1;
        console.error(`  ✗ ${name} failed: ${error.message}`);
      }
    }
    if (failed === tasks.length) {
      throw new Error("All runtime preparations failed");
    }
    if (failed > 0) {
      console.log(
        `\nDone with ${failed}/${tasks.length} failures. Bundle 会缺这些 CLI 的 binary，用户需要自己装。`
      );
    } else {
      console.log("\nDone.");
    }
  } finally {
    await rm(tmpDir, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.stack || error.message : String(error));
  process.exit(1);
});
