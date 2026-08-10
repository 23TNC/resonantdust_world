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

/** Tics per wall hour at the project's 6 Hz tic (`codec::tic::TIC_HZ`). The accessor returns
 *  the rate per TIC — the honest unit, but unreadable: thirst drains at 0.0023/tic, which any
 *  sane rounding shows as "-0.0". Per HOUR is what the corpus authors in (`deplete = 21600`
 *  means "the whole domain in one hour"), so it is the unit the numbers can be compared to. */
const TICS_PER_HOUR = 6 * 60 * 60;

/** `+50/h` / `-12.5/h` / `0` — always signed, because the sign IS the reading. */
function formatRate(ratePerTic: number): string {
  const perHour = ratePerTic * TICS_PER_HOUR;
  if (Math.abs(perHour) < 0.05) return "0";
  const mag = Math.abs(perHour);
  const s = (mag >= 100 ? perHour.toFixed(0) : mag >= 10 ? perHour.toFixed(1) : perHour.toFixed(2));
  return `${perHour > 0 ? "+" : ""}${s}/h`;
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
      // Spell the unit out on hover: "/h" is doing a lot of work for two characters, and
      // "is that per tic?" is the first thing anyone asks (user, 2026-08-09).
      track.title =
        `${n.label}: ${n.value.toFixed(1)} of ${n.min}..${n.max}\n` +
        `${formatRate(n.rate)} — ${(n.rate * TICS_PER_HOUR).toFixed(2)} units per wall-clock hour`;

      const rate = document.createElement("span");
      rate.style.cssText =
        "flex:0 0 auto;min-width:calc(var(--ui-row) * 2.4);text-align:right;" +
        `font-variant-numeric:tabular-nums;color:${n.rate < 0 ? RATE_DOWN : RATE_UP};`;
      rate.textContent = formatRate(n.rate);
      rate.title = `${(n.rate * TICS_PER_HOUR).toFixed(2)} units per wall-clock hour`;

      row.append(label, track, rate);
      this.element.appendChild(row);
    }
  }
}
