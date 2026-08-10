//! Live-edit tab 1 — TRAITS: colour squares in a grid, name on hover.
//!
//! Colour is AUTHORED on the trait def (`2026-08-09-live-edit` F2). Traits carried no colour
//! before this stream — emotions did, and conditions borrow theirs through the emotion pie, which
//! is exactly why this tab needed engine work and the conditions/emotions tabs did not.
//!
//! An unauthored trait renders neutral rather than refusing, so a half-coloured corpus still
//! shows you what a pawn has.

import type { LiveEditTab } from "./LiveEditPanel";
import type { LiveEditSnapshot } from "./snapshot";

/** Square side, in row units — matches the conditions cards' rhythm. */
const SQUARE = "calc(var(--ui-row) * 1.2)";

const hex = (c: number): string => `#${(c >>> 0).toString(16).padStart(6, "0")}`;

export class TraitsTab implements LiveEditTab {
  readonly id = "traits";
  readonly label = "Traits";
  readonly element = document.createElement("div");
  /** One shared cursor-following tooltip, the IntentStrip / ConditionCards pattern. */
  private readonly tooltip = document.createElement("div");

  constructor() {
    this.element.style.cssText =
      "display:flex;flex-wrap:wrap;gap:var(--ui-gap);align-content:flex-start;";
    this.tooltip.style.cssText =
      "position:fixed;display:none;z-index:2147483000;pointer-events:none;" +
      "background:#1c1f24;color:#d7dde5;border:1px solid #444;border-radius:4px;" +
      "padding:2px 8px;font:var(--ui-font)/1.6 monospace;white-space:nowrap;";
    document.body.appendChild(this.tooltip);
  }

  render(snap: LiveEditSnapshot): void {
    this.element.textContent = "";
    if (!snap.traits.length) {
      this.element.textContent = "—";
      return;
    }
    for (const t of snap.traits) {
      const sq = document.createElement("div");
      sq.style.cssText =
        `width:${SQUARE};height:${SQUARE};flex:0 0 auto;border-radius:3px;` +
        `border:1px solid #3a3a4a;background:${hex(t.color)};cursor:default;`;
      sq.addEventListener("mouseenter", (e) => {
        this.tooltip.textContent = t.label;
        this.tooltip.style.display = "block";
        this.tooltip.style.left = `${e.clientX + 12}px`;
        this.tooltip.style.top = `${e.clientY - 6}px`;
      });
      sq.addEventListener("mousemove", (e) => {
        this.tooltip.style.left = `${e.clientX + 12}px`;
        this.tooltip.style.top = `${e.clientY - 6}px`;
      });
      sq.addEventListener("mouseleave", () => { this.tooltip.style.display = "none"; });
      this.element.appendChild(sq);
    }
  }

  destroy(): void { this.tooltip.remove(); }
}
