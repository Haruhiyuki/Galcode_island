import { useEffect, useRef } from "react";
import * as PIXI from "pixi.js";
import { Live2DModel } from "pixi-live2d-display";

const MOOD_EMOJI = {
  default: "🙂",
  init: "✨",
  thinking: "🤔",
  working: "💻",
  done: "🎉",
  error: "😵",
  suggest: "💡",
  log: "📎",
};

function stageForAgent(stage) {
  if (!stage) return "default";
  const s = String(stage).toLowerCase();
  if (s === "init") return "init";
  if (s === "thinking") return "thinking";
  if (s === "working" || s === "executing") return "working";
  if (s === "done") return "done";
  if (s === "error") return "error";
  if (s === "suggest" || s === "suggesting") return "suggest";
  if (s === "log") return "log";
  return "default";
}

export function moodFromUi(uiState, lastStage) {
  if (uiState === "error") return "error";
  if (uiState === "suggesting") return "suggest";
  if (uiState === "done") return "done";
  if (uiState === "running") return stageForAgent(lastStage);
  return "default";
}

export function HaruhiStage({ mood }) {
  const hostRef = useRef(null);
  const appRef = useRef(null);
  const emojiRef = useRef(null);
  const live2dRef = useRef(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return undefined;

    const w = Math.max(host.clientWidth, 320);
    const h = Math.max(host.clientHeight, 220);

    const app = new PIXI.Application({
      width: w,
      height: h,
      backgroundAlpha: 0,
      antialias: true,
      resolution: window.devicePixelRatio || 1,
      autoDensity: true,
    });
    appRef.current = app;
    host.appendChild(app.view);

    const text = new PIXI.Text(MOOD_EMOJI[mood] || MOOD_EMOJI.default, {
      fontSize: Math.min(96, Math.floor(Math.min(w, h) * 0.28)),
      fill: 0xffffff,
      align: "center",
      dropShadow: true,
      dropShadowBlur: 6,
      dropShadowDistance: 2,
      dropShadowColor: 0x000000,
    });
    text.anchor.set(0.5);
    text.x = app.screen.width / 2;
    text.y = app.screen.height / 2;
    app.stage.addChild(text);
    emojiRef.current = text;

    let cancelled = false;
    (async () => {
      try {
        const url = new URL("/models/haruhi/haruhi.model3.json", window.location.origin).toString();
        const probe = await fetch(url, { method: "GET" });
        if (!probe.ok || cancelled) return;
        const model = await Live2DModel.from(url);
        if (cancelled) {
          model.destroy();
          return;
        }
        app.stage.removeChild(text);
        emojiRef.current = null;
        model.anchor.set(0.5, 0.55);
        model.position.set(app.screen.width / 2, app.screen.height * 0.62);
        const scale = Math.min(
          (app.screen.width * 0.92) / model.width,
          (app.screen.height * 0.92) / model.height,
        );
        model.scale.set(scale);
        app.stage.addChild(model);
        live2dRef.current = model;
      } catch {
        /* Cubism / model missing: keep emoji */
      }
    })();

    const ro = new ResizeObserver(() => {
      if (!appRef.current || !hostRef.current) return;
      const nw = Math.max(hostRef.current.clientWidth, 320);
      const nh = Math.max(hostRef.current.clientHeight, 220);
      app.renderer.resize(nw, nh);
      if (emojiRef.current) {
        emojiRef.current.x = nw / 2;
        emojiRef.current.y = nh / 2;
      }
      if (live2dRef.current) {
        const m = live2dRef.current;
        m.position.set(nw / 2, nh * 0.62);
        const sc = Math.min((nw * 0.92) / m.width, (nh * 0.92) / m.height);
        m.scale.set(sc);
      }
    });
    ro.observe(host);

    return () => {
      cancelled = true;
      ro.disconnect();
      live2dRef.current = null;
      emojiRef.current = null;
      app.destroy(true, { children: true, texture: true });
      appRef.current = null;
    };
  }, []);

  useEffect(() => {
    const emoji = emojiRef.current;
    if (emoji) {
      emoji.text = MOOD_EMOJI[mood] || MOOD_EMOJI.default;
    }
    const model = live2dRef.current;
    if (model && typeof model.expression === "function") {
      const map = {
        init: "default",
        thinking: "default",
        working: "default",
        done: "default",
        error: "default",
        suggest: "default",
        log: "default",
        default: "default",
      };
      try {
        model.expression(map[mood] || "default");
      } catch {
        /* expression name may not exist on this model */
      }
    }
  }, [mood]);

  return <div ref={hostRef} className="live2d-panel" />;
}
