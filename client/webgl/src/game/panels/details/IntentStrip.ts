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

/** Base circle diameter at `size = 1.0`, css px. */
const BASE_D = 26;
/** The strip's fixed column width — the panel content shifts right by this. */
export const STRIP_W = 40;
const RING_STROKE = 3;
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
      `width:${STRIP_W}px;flex:0 0 ${STRIP_W}px;display:flex;` +
      "flex-direction:column-reverse;align-items:center;gap:6px;" +
      "padding:6px 0;box-sizing:border-box;overflow:hidden;";
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
    const d = Math.max(10, Math.round(BASE_D * visual.size));
    const box = d + RING_STROKE * 2 + 2;
    const wrap = document.createElement("div");
    wrap.style.cssText =
      `position:relative;width:${box}px;height:${box}px;flex:0 0 auto;cursor:pointer;`;
    const dot = document.createElement("div");
    dot.style.cssText =
      `position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);` +
      `width:${d}px;height:${d}px;border-radius:50%;` +
      `background:${hex(visual.background, NEUTRAL_BG)};` +
      `opacity:${active ? 1 : 0.75};`;
    wrap.appendChild(dot);
    // The ACTIVE entry's ring (executing phase only; `progress = "none"` renders none).
    if (active && entry.phase === 2 && visual.progress !== "none") {
      const r = (d + RING_STROKE) / 2;
      const c = 2 * Math.PI * r;
      const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
      svg.setAttribute("width", String(box));
      svg.setAttribute("height", String(box));
      // Start at 12 o'clock; "ccw" mirrors the sweep (F5).
      const flip = visual.progress === "ccw" ? "scale(-1,1)" : "";
      svg.style.cssText =
        `position:absolute;inset:0;transform:rotate(-90deg) ${flip};pointer-events:none;`;
      const circle = document.createElementNS("http://www.w3.org/2000/svg", "circle");
      circle.setAttribute("cx", String(box / 2));
      circle.setAttribute("cy", String(box / 2));
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
