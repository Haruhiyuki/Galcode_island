import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { HaruhiStage, moodFromUi } from "./HaruhiStage.jsx";
import "./App.css";

function pushLog(setLogs, line) {
  setLogs((prev) => {
    const next = [...prev, line];
    return next.slice(-80);
  });
}

function mapAgentStatusToStage(st) {
  const s = String(st ?? "").toLowerCase();
  if (s === "thinking") return "thinking";
  if (s === "processing") return "working";
  if (s === "completed") return "done";
  if (s === "starting" || s === "running") return "init";
  if (s === "waitingapproval") return "thinking";
  if (s === "error") return "error";
  return "default";
}

export default function App() {
  const [task, setTask] = useState("用 Python 写一个简单的下载网页的小脚本说明。");
  const [projectPath, setProjectPath] = useState(".");
  const [sessionId, setSessionId] = useState(null);
  const sessionIdRef = useRef(null);

  const [uiState, setUiState] = useState("idle");
  const [percent, setPercent] = useState(0);
  const [bubble, setBubble] = useState("嗨，我是春日桌宠！输入中文任务，我会调用 Demo Agent。");
  const [resultZh, setResultZh] = useState("");
  const [summaryText, setSummaryText] = useState("");
  const [emotionText, setEmotionText] = useState("");
  const [suggestion, setSuggestion] = useState("");
  const [logs, setLogs] = useState([]);
  const [lastStage, setLastStage] = useState("default");

  const mood = useMemo(
    () => moodFromUi(uiState, lastStage),
    [uiState, lastStage],
  );

  useEffect(() => {
    sessionIdRef.current = sessionId;
  }, [sessionId]);

  useEffect(() => {
    const unsubs = [];
    const run = async () => {
      const forSession = (sid, fn) => {
        if (!sid || sid !== sessionIdRef.current) return;
        fn();
      };

      unsubs.push(
        await listen("agent://status-changed", (e) => {
          const p = e.payload;
          forSession(p?.sessionId, () => {
            setUiState("running");
            setLastStage(mapAgentStatusToStage(p.status));
            if (typeof p.percent === "number") {
              setPercent(Math.max(0, Math.min(100, p.percent)));
            }
            const hint = p.toolDescription ?? p.toolName;
            if (hint) setBubble(String(hint));
          });
        }),
      );

      unsubs.push(
        await listen("agent://log", (e) => {
          const p = e.payload;
          forSession(p?.sessionId, () => {
            pushLog(setLogs, `[${p.level}] ${p.message}`);
          });
        }),
      );

      unsubs.push(
        await listen("agent://session-complete", (e) => {
          const p = e.payload;
          forSession(p?.sessionId, () => {
            setUiState("done");
            setPercent(100);
            setLastStage("done");
            const zh = p.resultZh ?? "";
            setResultZh(zh);
            setSummaryText(p.summary ?? "");
            setEmotionText(p.emotion ?? "");
            setSuggestion(p.suggestionZh ?? "");
            setBubble(p.emotion || "任务完成！");
            pushLog(
              setLogs,
              `[session-complete] ${(p.summary ?? "").slice(0, 320)}`,
            );
          });
        }),
      );

      unsubs.push(
        await listen("agent://error", (e) => {
          const p = e.payload;
          forSession(p?.sessionId, () => {
            const msg = p?.message ?? String(e.payload ?? "未知错误");
            setUiState("error");
            setLastStage("error");
            setBubble(msg);
            pushLog(setLogs, `[agent://error] ${msg}`);
          });
        }),
      );

      unsubs.push(
        await listen("agent-progress", (e) => {
          const p = e.payload;
          if (p?.sessionId && p.sessionId !== sessionIdRef.current) return;
          setUiState("running");
          if (p?.stage) setLastStage(p.stage);
          if (typeof p?.percent === "number") {
            setPercent(Math.max(0, Math.min(100, p.percent)));
          }
          if (p?.message) setBubble(p.message);
          if (p?.rawLine) pushLog(setLogs, p.rawLine);
        }),
      );

      unsubs.push(
        await listen("agent-done", (e) => {
          const p = e.payload;
          if (sessionIdRef.current && p?.sessionId && p.sessionId !== sessionIdRef.current) {
            return;
          }
          const zh = p?.resultZh ?? "";
          if (zh) setResultZh(zh);
        }),
      );

      unsubs.push(
        await listen("suggestion-ready", (e) => {
          const p = e.payload;
          const text = p?.textZh ?? "";
          if (text) {
            setSuggestion(text);
            setUiState("suggesting");
            setLastStage("suggest");
          }
        }),
      );
    };
    run();
    return () => {
      unsubs.forEach((u) => {
        try {
          u();
        } catch {
          /* noop */
        }
      });
    };
  }, []);

  const pickFolder = useCallback(async () => {
    try {
      const path = await invoke("select_project_folder");
      if (path) setProjectPath(path);
    } catch (err) {
      pushLog(setLogs, `[error] select_project_folder: ${String(err)}`);
    }
  }, []);

  const start = useCallback(async () => {
    setLogs([]);
    setResultZh("");
    setSummaryText("");
    setEmotionText("");
    setSuggestion("");
    setPercent(0);
    setUiState("running");
    setLastStage("init");
    setBubble("启动 Agent…");
    setSessionId(null);
    sessionIdRef.current = null;
    try {
      const res = await invoke("start_agent", {
        userInputZh: task,
        cwd: projectPath || ".",
      });
      const sid = res?.sessionId ?? null;
      setSessionId(sid);
      sessionIdRef.current = sid;
    } catch (err) {
      setUiState("error");
      setBubble(String(err));
      pushLog(setLogs, `[error] ${String(err)}`);
    }
  }, [task, projectPath]);

  const stop = useCallback(async () => {
    try {
      const sid = sessionIdRef.current;
      await invoke("stop_agent", sid ? { sessionId: sid } : {});
      setBubble("已请求停止。");
      setUiState("idle");
    } catch (err) {
      pushLog(setLogs, `[error] stop: ${String(err)}`);
    }
  }, []);

  const translatePreview = useCallback(async () => {
    try {
      const out = await invoke("translate_only", { textZh: task });
      pushLog(setLogs, `[translate_only] ${out}`);
      setBubble(`翻译预览：${out}`);
    } catch (err) {
      pushLog(setLogs, `[error] translate_only: ${String(err)}`);
    }
  }, [task]);

  const toggleClickThrough = useCallback(async () => {
    try {
      await invoke("set_click_through", { enabled: true });
      pushLog(setLogs, "[window] click-through enabled");
    } catch (err) {
      pushLog(setLogs, `[error] set_click_through: ${String(err)}`);
    }
  }, []);

  return (
    <div className="app-shell">
      <header
        className="drag-bar"
        data-tauri-drag-region
        onPointerDown={(e) => {
          if (e.button !== 0) return;
          e.preventDefault();
          getCurrentWindow().startDragging();
        }}
      >
        凉宫春日 AI 桌宠 · 拖动标题栏移动窗口
      </header>

      <div className="app-body">
        <div>
          <HaruhiStage mood={mood} />
          <div className="bubble">{bubble}</div>
        </div>

        <div className="panel">
          <div>
            <span className={`status-pill status-${uiState}`}>{uiState}</span>
            {sessionId ? (
              <span className="log-line" style={{ marginLeft: 8, opacity: 0.75 }}>
                session: {sessionId.slice(0, 8)}…
              </span>
            ) : null}
          </div>

          <div className="progress-wrap">
            <div className="progress-bar">
              <div className="progress-fill" style={{ width: `${percent}%` }} />
            </div>
            <div className="progress-label">{Math.round(percent)}%</div>
          </div>

          <div className="row-actions" style={{ alignItems: "center" }}>
            <span style={{ flex: 1, fontSize: 12, opacity: 0.85, overflow: "hidden", textOverflow: "ellipsis" }}>
              {projectPath}
            </span>
            <button type="button" className="secondary" onClick={pickFolder}>
              选择文件夹
            </button>
          </div>

          <textarea
            className="task-input"
            value={task}
            onChange={(e) => setTask(e.target.value)}
            placeholder="用中文描述你想让 Agent 做的事…"
          />

          <div className="row-actions">
            <button type="button" onClick={start} disabled={uiState === "running"}>
              启动 Agent
            </button>
            <button type="button" className="secondary" onClick={stop}>
              停止
            </button>
            <button type="button" className="secondary" onClick={translatePreview}>
              翻译预览
            </button>
            <button type="button" className="secondary" onClick={toggleClickThrough}>
              点击穿透
            </button>
          </div>

          {summaryText ? (
            <div className="bubble" style={{ maxHeight: 100 }}>
              <strong>总结</strong>
              <div style={{ marginTop: 6, whiteSpace: "pre-wrap" }}>{summaryText}</div>
            </div>
          ) : null}

          {emotionText ? (
            <div className="bubble" style={{ maxHeight: 72 }}>
              <strong>情绪反馈</strong>
              <div style={{ marginTop: 6 }}>{emotionText}</div>
            </div>
          ) : null}

          {resultZh ? (
            <div className="bubble" style={{ maxHeight: 140 }}>
              <strong>结果（中文）</strong>
              <div style={{ marginTop: 6, whiteSpace: "pre-wrap" }}>{resultZh}</div>
            </div>
          ) : null}

          {suggestion ? (
            <div className="bubble" style={{ maxHeight: 160 }}>
              <strong>建议</strong>
              <div style={{ marginTop: 6, whiteSpace: "pre-wrap" }}>{suggestion}</div>
            </div>
          ) : null}

          <div className="log-panel">
            {logs.length === 0 ? (
              <div className="log-line" style={{ opacity: 0.7 }}>
                订阅 agent://* 事件与兼容旧 agent-progress。
              </div>
            ) : (
              logs.map((line, i) => (
                <div key={`${i}-${line.slice(0, 24)}`} className="log-line">
                  {line}
                </div>
              ))
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
