//! The CONDITIONS panel — the selected object's active conditions as emotion-pie cards
//! (`2026-08-09-selection-panels` F3). One of three sibling selection surfaces: details says
//! what a thing IS, intentions shows its queue, this shows what it is feeling.
//!
//! It exists because the cards used to live OUTSIDE the details panel — appended to the host
//! and positioned against details' rect — since `PANEL_CSS`'s `overflow: hidden` would clip a
//! strip wider than its parent. A panel of their own, as wide as the screen, removes the reason:
//! the cards flow left-to-right inside it and never escape a clip.
//!
//! Authored full-width along the bottom at `backgroundOpacity: 0` + `clickThrough: true`, so it
//! sits over the world without occluding it and passes clicks through — while the cards
//! themselves keep `pointer-events: auto` and stay live (design invariant 9).

import { DomPanel, Z_TIER_INFO } from "../../../ui/dom/DomPanel";
import { ConditionCards } from "./ConditionCards";
import type { ConditionCard } from "./ConditionCards";
import { panelTitle } from "../panelStrings";
import type { GameContext } from "../../../GameContext";
import type { SelectionModel } from "../../world/SelectionModel";

const PANEL_KEY = "conditionsPanel";

/** The one thing this panel reads. Deliberately narrower than `DetailsProviders`: a panel that
 *  only needs conditions should not be able to reach a pawn's tile or inventory. */
export interface ConditionsProviders {
  pawn(entity: number): { conditions: ConditionCard[] } | null;
}

export class ConditionsPanel extends DomPanel {
  private readonly cards: ConditionCards;
  private readonly unsubSel: () => void;
  private readonly timer: number;
  /** The last rendered set's identity, so an unchanged poll does not rebuild the pie DOM and
   *  destroy a tooltip mid-read (I8). */
  private lastKey = "";

  constructor(ctx: GameContext, private readonly selection: SelectionModel,
              private readonly providers: ConditionsProviders) {
    super({
      title: panelTitle(PANEL_KEY),
      storageKey: "conditions",
      // bug-sweep F1's table: a sibling of details, on the INFO tier. Being click-through
      // means overlapping it costs the user nothing.
      zOrder: Z_TIER_INFO,
      taskbar: ctx.taskbar,
      pinned: true,
      taskbarIcon: "☯",
      uiEditMode: ctx.uiEditMode,
      // The stream's whole shape (F3): full width along the bottom of the field, transparent,
      // click-through, no bar. The corpus can override all four.
      defaultCell: { col: 0, row: 28, cols: 58, rows: 3 },
      backgroundOpacity: 0,
      clickThrough: true,
    });
    this.cards = new ConditionCards({ storageKey: "conditions" });
    const holder = document.createElement("div");
    // Cards run left-to-right and wrap; the body scrolls horizontally only if a pawn ever
    // carries more than a full screen of conditions (I3). `align-items: flex-end` keeps the
    // minimized cards sitting on the same baseline as the maximized ones.
    holder.style.cssText =
      "display:flex;align-items:flex-end;overflow-x:auto;overflow-y:hidden;" +
      "padding:var(--ui-pad-sm) var(--ui-pad);height:100%;box-sizing:border-box;" +
      // The holder is a pass-through like the card container it wraps, so the gaps between
      // cards never swallow a click meant for the world.
      "pointer-events:none;";
    holder.appendChild(this.cards.element);
    this.setBody(holder);

    this.unsubSel = selection.subscribe(() => this.render());
    // Conditions expire and are granted while a pawn is selected, and the selection event only
    // fires on selection CHANGES — so poll, at the same cadence details always used.
    this.timer = window.setInterval(() => this.render(), 500);
    this.render();
  }

  private render(): void {
    const p = this.selection.primary;
    const cards = p?.kind === "pawn" ? (this.providers.pawn(p.entity)?.conditions ?? []) : [];
    // Identity check before the rebuild (I8): the poll runs twice a second, and re-rendering
    // unchanged cards would tear down the hovered card's tooltip on every tick.
    const key = cards.map((c) => `${c.id}:${c.remaining}:${c.magnitudeSum}`).join(",");
    if (key === this.lastKey) return;
    this.lastKey = key;
    this.cards.setCards(cards);
  }

  destroy(): void {
    this.unsubSel();
    clearInterval(this.timer);
    this.cards.destroy();
    super.destroy();
  }
}
