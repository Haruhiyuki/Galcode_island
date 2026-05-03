import { useEffect, useRef } from "react";
import * as PIXI from "pixi.js";
/* 仅用 Cubism 3/4；勿用主入口，否则会要求未加载的 live2d.min.js (Cubism 2) */
import { Live2DModel } from "pixi-live2d-display/cubism4";

if (typeof window !== "undefined" && !(window as Window & { PIXI?: unknown }).PIXI) {
  (window as Window & { PIXI: typeof PIXI }).PIXI = PIXI;
}

/** Dev server may 200-return index.html for missing paths; Live2D also rejects non-JSON. */
async function isCubismModel3EntryUrl(url: string): Promise<boolean> {
  try {
    const r = await fetch(url, { method: "GET" });
    if (!r.ok) return false;
    const ct = (r.headers.get("content-type") || "").toLowerCase();
    if (ct.includes("text/html")) return false;
    const t = (await r.text()).trim();
    if (!t.startsWith("{") || t.startsWith("<!")) return false;
    const o = JSON.parse(t) as { FileReferences?: unknown };
    return o.FileReferences != null && typeof o.FileReferences === "object";
  } catch {
    return false;
  }
}

const MOOD_EMOJI: Record<string, string> = {
  default: "\u{1F642}",
  init: "\u{2728}",
  thinking: "\u{1F914}",
  working: "\u{1F4BB}",
  done: "\u{1F389}",
  error: "\u{1F635}",
  suggest: "\u{1F4A1}",
  log: "\u{1F4CE}",
};

export function moodFromUi(uiState: string, lastStage: string): string {
  if (uiState === "error") return "error";
  if (uiState === "suggesting") return "suggest";
  if (uiState === "done") return "done";
  if (uiState === "running") {
    const s = String(lastStage ?? "").toLowerCase();
    if (s === "init") return "init";
    if (s === "thinking") return "thinking";
    if (s === "working" || s === "executing") return "working";
    if (s === "done") return "done";
    if (s === "error") return "error";
    if (s === "suggest" || s === "suggesting") return "suggest";
    if (s === "log") return "log";
    return "default";
  }
  return "default";
}

interface HaruhiStageProps {
  mood: string;
}

export function HaruhiStage({ mood }: HaruhiStageProps): JSX.Element {
  const hostRef = useRef<HTMLDivElement>(null);
  const appRef = useRef<PIXI.Application | null>(null);
  const emojiRef = useRef<PIXI.Text | null>(null);
  const live2dRef = useRef<Live2DModel | null>(null);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    let app: PIXI.Application | null = null;
    let cancelled = false;
    let ro: ResizeObserver | null = null;

    try {
      const w = Math.max(host.clientWidth, 320);
      const h = Math.max(host.clientHeight, 220);

      app = new PIXI.Application({
        width: w,
        height: h,
        backgroundAlpha: 0,
        antialias: true,
        resolution: window.devicePixelRatio || 1,
        autoDensity: true,
      });
      appRef.current = app;
      host.appendChild(app.view as HTMLCanvasElement);

      const text = new PIXI.Text(MOOD_EMOJI[mood] ?? MOOD_EMOJI.default, {
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

      (async () => {
        try {
          const url = new URL("/models/haruhi/haruhi.model3.json", window.location.origin).toString();
          if (cancelled || !app) return;
          if (!(await isCubismModel3EntryUrl(url))) return;
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
          /* Cubism / model missing: keep emoji fallback */
        }
      })();

      ro = new ResizeObserver(() => {
        if (!appRef.current || !hostRef.current) return;
        const nw = Math.max(hostRef.current.clientWidth, 320);
        const nh = Math.max(hostRef.current.clientHeight, 220);
        appRef.current.renderer.resize(nw, nh);
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
    } catch (err) {
      console.warn("[HaruhiStage] PIXI/WebGL 初始化失败，使用 DOM 表情兜底", err);
      appRef.current = null;
      const fb = document.createElement("div");
      fb.textContent = MOOD_EMOJI[mood] ?? MOOD_EMOJI.default;
      fb.style.cssText =
        "display:flex;align-items:center;justify-content:center;font-size:clamp(3rem,15vw,5rem);user-select:none;";
      host.appendChild(fb);
      return () => {
        cancelled = true;
        fb.remove();
      };
    }

    return () => {
      cancelled = true;
      ro?.disconnect();
      live2dRef.current = null;
      emojiRef.current = null;
      if (app) {
        try {
          app.destroy(true, { children: true, texture: true });
        } catch {
          /* noop */
        }
      }
      appRef.current = null;
    };
  }, []);

  useEffect(() => {
    const emoji = emojiRef.current;
    if (emoji) {
      emoji.text = MOOD_EMOJI[mood] ?? MOOD_EMOJI.default;
    }
    const model = live2dRef.current;
    if (model && typeof (model as Live2DModel & { expression?: (name: string) => void }).expression === "function") {
      try {
        (model as Live2DModel & { expression: (name: string) => void }).expression("default");
      } catch {
        /* expression name may not exist on this model */
      }
    }
    const app = appRef.current;
    if (model && app) {
      const nw = app.screen.width;
      const nh = app.screen.height;
      const base = Math.min((nw * 0.92) / model.width, (nh * 0.92) / model.height);
      const bump =
        mood === "thinking" || mood === "working" ? 1.035 : mood === "error" ? 0.97 : 1;
      model.scale.set(base * bump);
    }
  }, [mood]);

  return <div ref={hostRef} className="live2d-panel" />;
}
