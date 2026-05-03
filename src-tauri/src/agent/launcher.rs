use super::config::AgentConfig;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn resolve_demo_script() -> PathBuf {
    if let Ok(p) = std::env::var("AGENT_SCRIPT") {
        let pb = PathBuf::from(&p);
        if pb.exists() {
            return pb;
        }
    }
    for candidate in [
        Path::new("scripts/demo_agent.py"),
        Path::new("../scripts/demo_agent.py"),
    ] {
        if candidate.exists() {
            return candidate.to_path_buf();
        }
    }
    PathBuf::from("scripts/demo_agent.py")
}

pub fn spawn_demo_process(
    cfg: &AgentConfig,
    cwd: &Path,
    script: &Path,
    task_en: &str,
) -> Result<std::process::Child, String> {
    if !script.exists() {
        return Err(format!(
            "Cannot find Demo Agent script: {} (set AGENT_SCRIPT env var to override)",
            script.display()
        ));
    }
    let mut cmd = Command::new(&cfg.executable);
    cmd.args([
        "-u",
        script.to_str().ok_or("Invalid script path")?,
        "--task",
    ])
    .arg(task_en)
    .current_dir(cwd)
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit());
    for (k, v) in &cfg.env_vars {
        cmd.env(k, v);
    }
    cmd.spawn()
        .map_err(|e| format!("Failed to start Agent: {} (executable={})", e, cfg.executable))
}

/// Result of spawning opencode in a visible terminal.
pub struct OpencodeLaunch {
    pub output_file: PathBuf,
    pub script_file: PathBuf,
}

/// Write a PowerShell script (UTF-8 BOM) that:
/// 1. Finds opencode via Get-Command / where.exe / common npm paths
/// 2. Runs `opencode run` via cmd.exe /c (resolves .cmd wrappers)
/// 3. Tees stdout+stderr to both the visible terminal and a JSONL file
/// 4. Writes a JSON error record to the output file on failure
/// 5. Holds the window open with Read-Host
pub fn spawn_opencode_terminal(
    cfg: &AgentConfig,
    cwd: &Path,
    task_en: &str,
) -> Result<OpencodeLaunch, String> {
    let temp_dir = std::env::temp_dir();
    let session_id = uuid::Uuid::new_v4().to_string();
    let id_short = &session_id[..8];

    let output_file = temp_dir.join(format!("galcode_out_{}.jsonl", id_short));
    let script_file = temp_dir.join(format!("galcode_run_{}.ps1", id_short));

    eprintln!(
        "[galcode] spawn_opencode_terminal: script={}, output={}",
        script_file.display(),
        output_file.display()
    );

    // Escape for embedding in PowerShell single-quoted strings:
    //   '  → ''   (PowerShell single-quote escape)
    let task_escaped = task_en.replace('\'', "''");
    let cwd_escaped = cwd.to_string_lossy().replace('\'', "''");
    let output_escaped = output_file.to_string_lossy().replace('\'', "''");
    let exe = &cfg.executable;

    // Script template. Double braces {{ }} become single { } after format!.
    // The entire logic is wrapped in try/finally so the window always stays open.
    let ps_script = format!(
        r#"$ErrorActionPreference = "Continue"
$OutputEncoding = [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::InputEncoding = [System.Text.Encoding]::UTF8
chcp 65001 > $null 2>&1

Set-Location -LiteralPath '{cwd}'

Write-Host "========================================="
Write-Host "  Galcode Island - Agent Terminal"
Write-Host "  Dir: {cwd}"
Write-Host "  Task: {task}"
Write-Host "========================================="
Write-Host ""

$task = '{task}'
$outputFile = '{output}'
$exeName = '{exe}'

try {{
    Write-Host "[DEBUG] Searching for $exeName ..."

    $foundPath = $null

    # Method 1: Get-Command (PowerShell native)
    try {{
        $foundPath = (Get-Command $exeName -ErrorAction Stop).Source
        Write-Host "[DEBUG] Get-Command -> $foundPath"
    }} catch {{
        Write-Host "[DEBUG] Get-Command failed: $_"
    }}

    # Method 2: where.exe (Windows)
    if (-not $foundPath) {{
        $whereLines = where.exe $exeName 2>&1 | Out-String
        Write-Host "[DEBUG] where.exe output: $whereLines"
        if ($whereLines -match '\S') {{
            $foundPath = ($whereLines -split "`r?`n" | Select-Object -First 1).Trim()
            Write-Host "[DEBUG] where.exe -> $foundPath"
        }}
    }}

    # Method 3: Common npm global install paths
    if (-not $foundPath) {{
        $candidates = @(
            "$env:APPDATA\npm\$exeName.cmd",
            "$env:LOCALAPPDATA\pnpm\$exeName.cmd",
            "$env:USERPROFILE\AppData\Roaming\npm\$exeName.cmd"
        )
        foreach ($c in $candidates) {{
            Write-Host "[DEBUG] Checking $c"
            if (Test-Path $c) {{
                $foundPath = $c
                Write-Host "[DEBUG] Found at $c"
                break
            }}
        }}
    }}

    if (-not $foundPath) {{
        $errMsg = "Cannot find $exeName. Please install it and ensure it is in PATH."
        Write-Host ""
        Write-Host "ERROR: $errMsg"
        Write-Host "Current PATH directories:"
        $env:PATH -split ';' | ForEach-Object {{ Write-Host "  $_" }}
        $errObj = @{{ type = "error"; message = $errMsg }} | ConvertTo-Json -Compress
        Add-Content -LiteralPath $outputFile -Value $errObj -Encoding UTF8
    }} else {{
        Write-Host "[DEBUG] Launching: $foundPath run --format json --dir . --dangerously-skip-permissions `"$task`""
        Write-Host ""

        $safeTask = $task -replace '"', '""'

        $cmdExe = [Environment]::GetEnvironmentVariable("ComSpec")
        if (-not $cmdExe) {{ $cmdExe = "cmd.exe" }}

        Write-Host "[DEBUG] ComSpec: $cmdExe"
        Write-Host "[DEBUG] Working dir: $(Get-Location)"
        Write-Host ""

        & $cmdExe /c "`"$foundPath`" run --format json --dir . --dangerously-skip-permissions `"$safeTask`"" 2>&1 | ForEach-Object {{
            $line = $_.ToString()
            Add-Content -LiteralPath $outputFile -Value $line -Encoding UTF8
            Write-Host $line
        }}

        Write-Host ""
        Write-Host "======== Agent process finished ========"
    }}
}} catch {{
    Write-Host ""
    Write-Host "SCRIPT ERROR: $_"
    Write-Host $_.ScriptStackTrace
    $errObj = @{{ type = "error"; message = "Script error: $_" }} | ConvertTo-Json -Compress
    try {{ Add-Content -LiteralPath $outputFile -Value $errObj -Encoding UTF8 }} catch {{}}
}} finally {{
    Write-Host ""
    Read-Host "Press Enter to close window"
}}"#,
        cwd = cwd_escaped,
        task = task_escaped,
        exe = exe,
        output = output_escaped,
    );

    // Write with UTF-8 BOM so PowerShell 5.1 reads Chinese characters correctly.
    let bom: [u8; 3] = [0xEF, 0xBB, 0xBF];
    let mut file_bytes = Vec::with_capacity(bom.len() + ps_script.len());
    file_bytes.extend_from_slice(&bom);
    file_bytes.extend_from_slice(ps_script.as_bytes());
    std::fs::write(&script_file, &file_bytes)
        .map_err(|e| format!("Cannot write launch script: {}", e))?;

    open_terminal_window(&script_file)?;

    Ok(OpencodeLaunch {
        output_file,
        script_file,
    })
}

/// Open a new visible terminal window that runs the given PowerShell script.
#[cfg(target_os = "windows")]
fn open_terminal_window(script_file: &Path) -> Result<(), String> {
    let script_path = script_file.to_string_lossy().to_string();
    eprintln!("[galcode] open_terminal_window: {}", script_path);

    // cmd /C start "" powershell ... — empty "" prevents title misinterpretation
    let _child = Command::new("cmd")
        .args([
            "/C",
            "start",
            "",
            "powershell",
            "-NoExit",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script_path,
        ])
        .spawn()
        .map_err(|e| format!("Cannot launch PowerShell terminal: {}", e))?;

    log::info!("Terminal spawned: {}", script_path);
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn open_terminal_window(script_file: &Path) -> Result<(), String> {
    let script_path = script_file.to_string_lossy().to_string();
    let output = Command::new("open")
        .args(["-a", "Terminal", &script_path])
        .output()
        .map_err(|e| format!("Cannot launch terminal: {}", e))?;
    if !output.status.success() {
        return Err(format!("Terminal launch failed: {}", output.status));
    }
    Ok(())
}

/// Read new lines from the output file since last check. Tracks position via line_count.
pub fn read_new_lines(output_file: &Path, line_count: &mut usize) -> Result<Vec<String>, String> {
    let content = match std::fs::read_to_string(output_file) {
        Ok(c) => c,
        Err(_) => {
            // Try UTF-16 LE (PowerShell 5.1 Add-Content default without -Encoding)
            let bytes = match std::fs::read(output_file) {
                Ok(b) => b,
                Err(_) => return Ok(vec![]),
            };
            if bytes.len() < 2 {
                return Ok(vec![]);
            }
            let bom_skip = if bytes[0] == 0xFF && bytes[1] == 0xFE {
                2
            } else {
                0
            };
            let utf16: Vec<u16> = bytes[bom_skip..]
                .chunks_exact(2)
                .filter(|c| c.len() == 2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16(&utf16).unwrap_or_default()
        }
    };

    let content = content.trim_start_matches('\u{FEFF}');

    let all_lines: Vec<&str> = content.lines().collect();
    if *line_count >= all_lines.len() {
        return Ok(vec![]);
    }

    let new_lines: Vec<String> = all_lines[*line_count..]
        .iter()
        .map(|s| s.to_string())
        .collect();
    *line_count = all_lines.len();

    Ok(new_lines)
}
