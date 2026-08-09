//! The intent-queue STRIP (intent-queue-ui F5) — the vertical column of circles pinned
//! along the details panel's LEFT edge. Bottom circle = the ACTIVE event; pending
//! intents stack above. Look per circle comes WHOLE from the interaction TOML's
//! `queue = { … }` block (via the wasm `queueVisual` accessor); the ACTIVE entry's
//! progress ring is an SVG stroke whose percentage is COMPUTED from the fanned
//! `(started, fire)` tics through the learned tic estimate EVERY frame — never
//! incremented — so a hidden tab snaps to truth on re-show (I5). A circle click
//! reports the entry's worker-minted id upward (the CANCEL_INTENT sender — P4).

import type { QueueEntry } from "../../world/IntentQueues";
import { Z_CHROME_BASE } from "../../../ui/dom/DomPanel";

/** The TOML visuals the wasm accessor returns (defaults already applied). */
export interface QueueVisual {
  hover: string;
  size: number;
  background: number | null;
  progress: "cw" | "ccw" | "none";
  progressColor: number | null;
  progressFill: boolean;
  cancelable: boolean;
}

/** The circle's box as a multiple of a grid ROW at `size = 1.0` — the cap it
 *  grows to in a roomy panel. Reproduces the old fixed 34px box at a ~28px row.
 *  A circle is `min(100%, this)`, so in a ONE-CELL-WIDE panel it shrinks to the
 *  panel instead of overflowing it: the strip must never need a scrollbar
 *  (user, 2026-08-09 — "the icons it displays need to fit inside of it"). */
const BOX_ROWS = 1.2;
/** Everything inside the circle is drawn in a fixed 100x100 viewBox and scaled
 *  by CSS, so the ring maths is resolution-independent and the strip's width
 *  can be whatever the panel is. Proportions preserved from the old 34px box:
 *  a 26px dot and a 3px ring stroke. */
const VB = 100;
const DOT_PCT = 26 / 34;
const RING_STROKE = (3 / 34) * VB;
const NEUTRAL_BG = "#3a3f46";
const NEUTRAL_RING = "#9aa4b0";

const hex = (c: number | null, fallback: string): string =>
  c === null ? fallback : `#${c.toString(16).padStart(6, "0")}`;

export class IntentStrip {
  readonly el = document.createElement("div");
  private readonly tooltip = document.createElement("div");
  private entries: QueueEntry[] = [];
  /** The ACTIVE entry's ring elements, re-driven per frame while one exists. */
  private ring: { circle: SVGCircleElement; entry: QueueEntry; visual: QueueVisual } | null = null;
  private raf = 0;

  constructor(
    private readonly visualOf: (interactionRef: number) => QueueVisual | null,
    /** The learned now-tic (u16), or null before the estimate anchors. */
    private readonly nowTic: () => number | null,
    private readonly onCircleClick: (entryId: number) => void,
  ) {
    // column-reverse: the FIRST entry (the active one) renders at the BOTTOM.
    this.el.style.cssText =
      // Fills the panel rather than claiming a fixed column. The old
      // `width: 40px` was sized for living inside the details panel; a panel
      // narrower than that would have scrolled.
      "width:100%;display:flex;" +
      "flex-direction:column-reverse;align-items:center;gap:var(--ui-gap);" +
      "padding:var(--ui-gap) 0;box-sizing:border-box;overflow:hidden;";
    this.tooltip.style.cssText =
      // bug-sweep F1: tooltips are CHROME, one above the pie menu.
      `position:fixed;display:none;z-index:${Z_CHROME_BASE + 11};pointer-events:none;` +
      "background:#1c1f24;color:#d7dde5;border:1px solid #444;border-radius:4px;" +
      "padding:2px 8px;font:11px/1.6 monospace;white-space:nowrap;";
    document.body.appendChild(this.tooltip);
  }

  /** Replace the strip's entries (entry 0 = active). Cheap full rebuild — ≤ 6 circles. */
  setEntries(entries: QueueEntry[]): void {
    this.entries = entries;
    this.el.textContent = "";
    this.ring = null;
    entries.forEach((entry, i) => {
      const visual = this.visualOf(entry.interactionRef) ?? {
        hover: `#${entry.interactionRef.toString(16)}`,
        size: 1.0,
        background: null,
        progress: "none" as const,
        progressColor: null,
        progressFill: true,
        cancelable: false,
      };
      this.el.appendChild(this.circle(entry, visual, i === 0));
    });
    this.driveRing();
  }

  destroy(): void {
    cancelAnimationFrame(this.raf);
    this.tooltip.remove();
    this.el.remove();
  }

  private circle(entry: QueueEntry, visual: QueueVisual, active: boolean): HTMLElement {
    // `min(100%, cap)` is the whole fix: the circle takes the panel's width
    // when the panel is narrow and its authored size when there is room. The
    // TOML's `size` scales the CAP, not an absolute pixel count.
    const cap = `calc(var(--ui-row) * ${(BOX_ROWS * visual.size).toFixed(3)})`;
    const wrap = document.createElement("div");
    wrap.style.cssText =
      `position:relative;width:min(100%, ${cap});aspect-ratio:1;` +
      "flex:0 0 auto;cursor:pointer;";
    const dot = document.createElement("div");
    dot.style.cssText =
      `position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);` +
      `width:${(DOT_PCT * 100).toFixed(1)}%;height:${(DOT_PCT * 100).toFixed(1)}%;` +
      "border-radius:50%;" +
      `background:${hex(visual.background, NEUTRAL_BG)};` +
      `opacity:${active ? 1 : 0.75};`;
    wrap.appendChild(dot);
    // The ACTIVE entry's ring (executing phase only; `progress = "none"` renders none).
    if (active && entry.phase === 2 && visual.progress !== "none") {
      // viewBox units, so the ring scales with the CSS box and the dash maths
      // below stays independent of how wide the panel happens to be.
      const r = (VB * DOT_PCT + RING_STROKE) / 2;
      const c = 2 * Math.PI * r;
      const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
      svg.setAttribute("viewBox", `0 0 ${VB} ${VB}`);
      svg.setAttribute("width", "100%");
      svg.setAttribute("height", "100%");
      // Start at 12 o'clock; "ccw" mirrors the sweep (F5).
      const flip = visual.progress === "ccw" ? "scale(-1,1)" : "";
      svg.style.cssText =
        `position:absolute;inset:0;transform:rotate(-90deg) ${flip};pointer-events:none;`;
      const circle = document.createElementNS("http://www.w3.org/2000/svg", "circle");
      circle.setAttribute("cx", String(VB / 2));
      circle.setAttribute("cy", String(VB / 2));
      circle.setAttribute("r", String(r));
      circle.setAttribute("fill", "none");
      circle.setAttribute("stroke", hex(visual.progressColor, NEUTRAL_RING));
      circle.setAttribute("stroke-width", String(RING_STROKE));
      circle.setAttribute("stroke-dasharray", String(c));
      circle.setAttribute("stroke-dashoffset", String(c));
      svg.appendChild(circle);
      wrap.appendChild(svg);
      this.ring = { circle, entry, visual };
    }
    wrap.addEventListener("mouseenter", (e) => {
      this.tooltip.textContent = visual.hover;
      this.tooltip.style.display = "block";
      this.tooltip.style.left = `${e.clientX + 12}px`;
      this.tooltip.style.top = `${e.clientY - 6}px`;
    });
    wrap.addEventListener("mousemove", (e) => {
      this.tooltip.style.left = `${e.clientX + 12}px`;
      this.tooltip.style.top = `${e.clientY - 6}px`;
    });
    wrap.addEventListener("mouseleave", () => {
      this.tooltip.style.display = "none";
    });
    wrap.addEventListener("pointerdown", (e) => {
      e.stopPropagation();
      this.onCircleClick(entry.entryId);
    });
    return wrap;
  }

  /** Re-drive the ring each frame from the CURRENT tic estimate (I5 — computed, never
   *  incremented). `progressFill` chooses whether the arc grows or shrinks. */
  private driveRing(): void {
    cancelAnimationFrame(this.raf);
    if (!this.ring) return;
    const step = (): void => {
      const ring = this.ring;
      if (!ring) return;
      const now = this.nowTic();
      if (now !== null) {
        const total = (ring.entry.fireTic - ring.entry.startedTic) & 0xffff;
        const elapsed = (now - ring.entry.startedTic) & 0xffff;
        let p = total > 0 ? Math.min(elapsed / total, 1) : 1;
        if (elapsed > 0x8000) p = 0; // serial-window guard: "before started" reads 0
        if (!ring.visual.progressFill) p = 1 - p;
        const c = Number(ring.circle.getAttribute("stroke-dasharray"));
        ring.circle.setAttribute("stroke-dashoffset", String(c * (1 - p)));
      }
      this.raf = requestAnimationFrame(step);
    };
    this.raf = requestAnimationFrame(step);
  }

  /** The current computed ring percentage (drill probe — I5's acceptance). */
  ringPercent(): number | null {
    if (!this.ring) return null;
    const now = this.nowTic();
    if (now === null) return null;
    const total = (this.ring.entry.fireTic - this.ring.entry.startedTic) & 0xffff;
    const elapsed = (now - this.ring.entry.startedTic) & 0xffff;
    if (elapsed > 0x8000) return 0;
    return total > 0 ? Math.min(elapsed / total, 1) : 1;
  }
}
