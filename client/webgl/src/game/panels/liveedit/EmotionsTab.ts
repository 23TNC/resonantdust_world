//! Live-edit tab 4 — EMOTIONS: labels with current values.
//!
//! The cheapest of the four, because emotions already author `color` (required) and the eval
//! already returns the whole `[index, sum0..sum15]` vector plus the argmax winner. Nothing new
//! was needed on the engine side for this tab.

import type { LiveEditTab } from "./LiveEditPanel";
import type { LiveEditSnapshot } from "./snapshot";

const hex = (c: number): string => `#${(c >>> 0).toString(16).padStart(6, "0")}`;

export class EmotionsTab implements LiveEditTab {
  readonly id = "emotions";
  readonly label = "Emotions";
  readonly element = document.createElement("div");

  constructor() {
    this.element.style.cssText =
      "display:flex;flex-direction:column;gap:var(--ui-pad-sm);font:var(--ui-font)/1.5 monospace;";
  }

  render(snap: LiveEditSnapshot): void {
    this.element.textContent = "";
    if (!snap.emotions.length) {
      this.element.textContent = "—";
      return;
    }
    for (const e of snap.emotions) {
      const row = document.createElement("div");
      row.style.cssText =
        "display:flex;align-items:center;gap:var(--ui-pad);" +
        (e.active ? "font-weight:bold;" : "opacity:0.75;");
      const swatch = document.createElement("span");
      swatch.style.cssText =
        `width:var(--ui-font);height:var(--ui-font);flex:0 0 auto;border-radius:2px;` +
        `background:${hex(e.color)};`;
      const name = document.createElement("span");
      name.style.cssText = "flex:1 1 auto;";
      // The ACTIVE emotion is the argmax winner — marked, not re-sorted.
      name.textContent = e.active ? `${e.label} ◂` : e.label;
      const val = document.createElement("span");
      val.style.cssText = "flex:0 0 auto;font-variant-numeric:tabular-nums;";
      val.textContent = String(e.value);
      row.append(swatch, name, val);
      this.element.appendChild(row);
    }
  }
}
