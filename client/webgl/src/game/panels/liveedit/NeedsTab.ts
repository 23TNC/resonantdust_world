//! Live-edit tab 2 — NEEDS: label, a bar, and a signed rate.
//!
//! The heaviest of the four (`2026-08-09-live-edit` F3). The bar is filled from the need's value
//! against its **effective** clamp — already narrowed by active conditions and traits, not the
//! authored domain — and the number beside it is the live rate of change, which is a PRODUCT over
//! those same modifiers and is the single figure here the client cannot derive for itself. A bar
//! alone says a pawn is at 40% thirst; the rate says whether that is fine or an emergency.
//!
//! This file does no arithmetic beyond a fill fraction: the sign convention is fixed in the
//! accessor (I4 — negative is always "losing satisfaction", for every need including inverted
//! domains), so the panel colours blindly rather than special-casing each need.

import type { LiveEditTab } from "./LiveEditPanel";
import type { LiveEditSnapshot, NeedRow } from "./snapshot";

const hex = (c: number): string => `#${(c >>> 0).toString(16).padStart(6, "0")}`;
/** The user's colours: losing is red, gaining is green. */
const RATE_DOWN = "#d65a3a";
const RATE_UP = "#58c04a";

/** Fraction of the bar to fill, from the EFFECTIVE clamp. Guards a degenerate span (a condition
 *  can narrow min and max onto each other) rather than dividing by zero. */
function fill(n: NeedRow): number {
  const span = n.max - n.min;
  if (!(span > 0)) return 0;
  return Math.min(1, Math.max(0, (n.value - n.min) / span));
}

/** `+1.4` / `-0.6` / `0` — always signed, so the sign is the reading. */
function formatRate(rate: number): string {
  if (rate === 0) return "0";
  const s = Math.abs(rate) >= 10 ? rate.toFixed(0) : rate.toFixed(1);
  return rate > 0 ? `+${s}` : s;
}

export class NeedsTab implements LiveEditTab {
  readonly id = "needs";
  readonly label = "Needs";
  readonly element = document.createElement("div");

  constructor() {
    this.element.style.cssText =
      "display:flex;flex-direction:column;gap:var(--ui-pad-sm);font:var(--ui-font)/1.5 monospace;";
  }

  render(snap: LiveEditSnapshot): void {
    this.element.textContent = "";
    if (!snap.needs.length) {
      this.element.textContent = "—";
      return;
    }
    for (const n of snap.needs) {
      const row = document.createElement("div");
      row.style.cssText = "display:flex;align-items:center;gap:var(--ui-pad);";

      const label = document.createElement("span");
      label.style.cssText =
        "flex:0 0 auto;min-width:calc(var(--ui-row) * 3);overflow:hidden;" +
        "text-overflow:ellipsis;white-space:nowrap;";
      label.textContent = n.label;

      // The bar: a track with a fill, coloured from the need's own TOML.
      const track = document.createElement("div");
      track.style.cssText =
        "flex:1 1 auto;height:calc(var(--ui-row) * 0.5);min-width:0;border-radius:2px;" +
        "background:rgba(255,255,255,0.10);overflow:hidden;";
      const bar = document.createElement("div");
      bar.style.cssText =
        `width:${(fill(n) * 100).toFixed(1)}%;height:100%;background:${hex(n.color)};`;
      track.appendChild(bar);
      track.title = `${n.value} / ${n.min}..${n.max}`;

      const rate = document.createElement("span");
      rate.style.cssText =
        "flex:0 0 auto;min-width:calc(var(--ui-row) * 1.4);text-align:right;" +
        `font-variant-numeric:tabular-nums;color:${n.rate < 0 ? RATE_DOWN : RATE_UP};`;
      rate.textContent = formatRate(n.rate);

      row.append(label, track, rate);
      this.element.appendChild(row);
    }
  }
}
