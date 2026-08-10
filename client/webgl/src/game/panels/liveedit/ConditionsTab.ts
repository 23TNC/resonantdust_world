//! Live-edit tab 3 — CONDITIONS: pie squares in a grid, name on hover.
//!
//! "Drawn as they are in the conditions bar with pie chart coloring" (user) — so this reuses
//! `ConditionCards` outright rather than re-drawing the pie. That class's whole job is already
//! colour-square + emotion-pie + everything-tooltip, and it became reusable when
//! `2026-08-09-selection-panels` F3 cut its host contract down to a single field.
//!
//! Reuse here is not just economy: a second pie implementation would drift from the conditions
//! bar, and the user's requirement is precisely that the two LOOK THE SAME.

import { ConditionCards } from "../conditions/ConditionCards";
import type { LiveEditTab } from "./LiveEditPanel";
import type { LiveEditSnapshot } from "./snapshot";

export class ConditionsTab implements LiveEditTab {
  readonly id = "conditions";
  readonly label = "Conditions";
  readonly element = document.createElement("div");
  private readonly cards = new ConditionCards({ storageKey: "liveEditConditions" });

  constructor() {
    this.element.style.cssText = "display:flex;align-items:flex-start;";
    this.element.appendChild(this.cards.element);
  }

  render(snap: LiveEditSnapshot): void {
    // Order is the eval's (conditions F3 / emotions F4) and must not be re-sorted here.
    this.cards.setCards(snap.conditions);
  }

  destroy(): void { this.cards.destroy(); }
}
