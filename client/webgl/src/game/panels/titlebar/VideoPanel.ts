import type { App } from "../../../app/App";
import { DomPanel } from "../../../ui/dom/DomPanel";
import { CyclingSelect } from "../../../ui/dom/CyclingSelect";
import type { PanelTaskbar } from "../../../ui/dom/PanelTaskbar";
import type { UiEditMode } from "../../../ui/dom/UiEditMode";
import { panelTitle, panelText } from "../panelStrings";

/** localStorage keys backing the two persisted numeric settings. The
 *  fullscreen state isn't persisted — it's a live property of the
 *  document, restored by the browser, not by us. */
const LS_MAX_FPS = "video.maxFps";
const LS_RENDER_SCALE = "video.renderScale";

/** Frame-rate cap presets. `0` is Pixi's "no cap" sentinel (`ticker.maxFPS`
 *  treats 0 as unlimited), rendered as "Unlimited". */
const FPS_OPTIONS: ReadonlyArray<{ value: number; label: string }> = [
  { value: 30, label: "30" },
  { value: 60, label: "60" },
  { value: 120, label: "120" },
  { value: 0, label: "∞" },
];

/** Fixed render-scale presets. The device's native ratio is appended at
 *  construction (only when it exceeds 1× so it doesn't duplicate the "100%"
 *  entry). Lower scales trade sharpness for fill-rate on weak GPUs. */
const SCALE_OPTIONS: ReadonlyArray<{ value: number; label: string }> = [
  { value: 0.5, label: "50%" },
  { value: 0.75, label: "75%" },
  { value: 1, label: "100%" },
];

const ROW_CSS: Partial<CSSStyleDeclaration> = {
  display: "flex",
  justifyContent: "space-between",
  alignItems: "center",
  gap: "12px",
  padding: "6px 12px",
  borderBottom: "1px solid #23252e",
};

const LABEL_CSS: Partial<CSSStyleDeclaration> = {
  color: "#a0a0b0",
  flex: "0 0 auto",
};

/** Glyph button for the fullscreen toggle — transparent so only the ▣ / ▢
 *  reads, matching the debug panel's toggle rows. */
const TOGGLE_BTN_CSS: Partial<CSSStyleDeclaration> = {
  background: "none",
  border: "none",
  color: "#ecd6aa",
  cursor: "pointer",
  font: "inherit",
  padding: "0",
};

/** Wrapper holding a cycling-select on the right of a row, sized so the
 *  control keeps a stable width regardless of the current label. */
const CONTROL_CSS: Partial<CSSStyleDeclaration> = {
  flex: "0 0 auto",
  minWidth: "120px",
};

/** Read a persisted number, falling back when absent or unparseable. */
function readNum(key: string, fallback: number): number {
  const raw = localStorage.getItem(key);
  if (raw === null) return fallback;
  const n = Number(raw);
  return Number.isFinite(n) ? n : fallback;
}

/**
 * Video settings panel. A pinned title-bar tool (like the debug HUD) that
 * exposes the handful of renderer knobs that are actually live-adjustable in
 * this client:
 *   - **Frame-rate limit** → `app.ticker.maxFPS`
 *   - **Render scale** → `app.renderer.resolution` (re-applied via a resize)
 *   - **Fullscreen** → the Fullscreen API on the canvas host
 *
 * The two numeric settings persist to localStorage and are re-applied to the
 * live `Application` in the constructor, so a saved cap / scale takes effect at
 * boot (the panel is built once in `main.ts`, before the first scene). All
 * chrome (title bar, drag, persistence, close) lives in {@link DomPanel}.
 */
export class VideoPanel {
  private readonly panel: DomPanel;
  private readonly app: App;
  private readonly fullscreenBtn: HTMLButtonElement;
  /** Bound so it can be removed in `destroy`; keeps the toggle glyph in sync
   *  with fullscreen changes triggered outside the panel (F11, Esc). */
  private readonly onFullscreenChange = (): void => this.syncFullscreenGlyph();

  get isOpen(): boolean { return this.panel.isOpen; }

  constructor(app: App, taskbar?: PanelTaskbar, uiEditMode?: UiEditMode) {
    this.app = app;
    this.panel = new DomPanel({
      title: panelTitle("videoPanel"),
      storageKey: "videoPanel",
      defaultRect: { right: "72px", top: "32px", width: "260px" },
      resizable: false,
      taskbar,
      pinned: true,
      taskbarIcon: "🖥",
      taskbarSide: "right",
      uiEditMode,
    });

    const body = document.createElement("div");

    // ── Frame-rate limit ──────────────────────────────────────────
    const savedFps = readNum(LS_MAX_FPS, 60);
    this.applyMaxFps(savedFps);
    this.addSelectRow(
      body,
      panelText("videoPanel", "frameRate"),
      FPS_OPTIONS,
      savedFps,
      (v) => {
        this.applyMaxFps(v);
        localStorage.setItem(LS_MAX_FPS, String(v));
      },
    );

    // ── Render scale ──────────────────────────────────────────────
    // Append the device's native ratio as a "Native" option when it's a
    // super-sampling factor (>1×) — otherwise it's just the 100% entry.
    const dpr = window.devicePixelRatio || 1;
    const scaleOptions = dpr > 1
      ? [...SCALE_OPTIONS, { value: dpr, label: `${panelText("videoPanel", "native")} (${Math.round(dpr * 100)}%)` }]
      : SCALE_OPTIONS;
    const savedScale = readNum(LS_RENDER_SCALE, dpr);
    this.applyRenderScale(savedScale);
    this.addSelectRow(
      body,
      panelText("videoPanel", "renderScale"),
      scaleOptions,
      savedScale,
      (v) => {
        this.applyRenderScale(v);
        localStorage.setItem(LS_RENDER_SCALE, String(v));
      },
    );

    // ── Fullscreen ────────────────────────────────────────────────
    const row = document.createElement("div");
    Object.assign(row.style, ROW_CSS);
    const label = document.createElement("span");
    Object.assign(label.style, LABEL_CSS);
    label.textContent = panelText("videoPanel", "fullscreen");
    this.fullscreenBtn = document.createElement("button");
    Object.assign(this.fullscreenBtn.style, TOGGLE_BTN_CSS);
    this.fullscreenBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      void this.toggleFullscreen();
    });
    row.appendChild(label);
    row.appendChild(this.fullscreenBtn);
    body.appendChild(row);
    this.syncFullscreenGlyph();
    document.addEventListener("fullscreenchange", this.onFullscreenChange);

    this.panel.setBody(body);
  }

  toggle(): void { this.panel.toggle(); }
  open():   void { this.panel.open();   }
  close():  void { this.panel.close();  }
  destroy(): void {
    document.removeEventListener("fullscreenchange", this.onFullscreenChange);
    this.panel.destroy();
  }

  /** Build a label + {@link CyclingSelect} row. `T` is the option value type
   *  (number here, but generic so future string/enum settings reuse it). */
  private addSelectRow<T>(
    parent: HTMLDivElement,
    labelText: string,
    options: ReadonlyArray<{ value: T; label: string }>,
    initial: T,
    onChange: (value: T) => void,
  ): void {
    const row = document.createElement("div");
    Object.assign(row.style, ROW_CSS);
    const label = document.createElement("span");
    Object.assign(label.style, LABEL_CSS);
    label.textContent = labelText;

    const control = document.createElement("div");
    Object.assign(control.style, CONTROL_CSS);
    const select = new CyclingSelect<T>({ options, initial, onChange });
    control.appendChild(select.element);

    row.appendChild(label);
    row.appendChild(control);
    parent.appendChild(row);
  }

  /** Apply the frame-rate cap to the shared ticker. `0` → uncapped. */
  private applyMaxFps(fps: number): void {
    this.app.ticker.maxFPS = fps;
  }

  /** Render scale (device-pixels per CSS-pixel). The shell owns no global
   *  renderer — the viewport self-canvases — so this records the choice for the
   *  viewport to read once it lands (webgl-engine W4). Persisted by the caller. */
  private renderScale = 1;
  private applyRenderScale(scale: number): void {
    if (!(scale > 0)) return;
    this.renderScale = scale;
    // TODO(webgl-engine W4): push `renderScale` to the viewport renderer's DPR.
  }

  private async toggleFullscreen(): Promise<void> {
    try {
      if (document.fullscreenElement) {
        await document.exitFullscreen();
      } else {
        // The canvas host (`#app`) so DOM panels overlaid on the canvas stay
        // inside the fullscreen surface; fall back to the document element.
        const target = document.getElementById("app") ?? document.documentElement;
        await target.requestFullscreen();
      }
    } catch (err) {
      console.warn("[VideoPanel] fullscreen toggle failed", err);
    }
    this.syncFullscreenGlyph();
  }

  private syncFullscreenGlyph(): void {
    this.fullscreenBtn.textContent = document.fullscreenElement ? "▣" : "▢";
  }
}
