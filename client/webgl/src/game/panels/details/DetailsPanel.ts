//! The details panel (ui-select P2, reworked by inventory F7) — the SELECTED object's
//! IDENTITY: its TOML name (`human_male`, `meat`, `grass`) + its tile, live from the
//! {@link SelectionModel} + the world's providers. The old debug text block (facing/
//! speed/zone/texture rows) is GONE (the user: "I just need to know what something is").
//! A top BUTTON ROW (I11) opens sibling panels — [Inventory] renders only when the
//! selected kind carries the inventory need. Consumes the model's change event only
//! (D1) — it never touches input, so drag-box multi-select lands as "N selected".
//!
//! **Identity ONLY** (`2026-08-09-selection-panels`): the condition cards and the intent strip
//! moved to panels of their own, and BOTH of this panel's reserves went with them — the bottom
//! band it held for cards that were never its children, and the left shift it made for the
//! intent column. What remains is the name, the tile, the [Inventory] button and the emotion
//! wash, which stays because it is the ACTIVE emotion — a property of the selected object
//! rather than of its condition list (F6).

import { DomPanel, Z_TIER_INFO } from "../../../ui/dom/DomPanel";
import type { ConditionCard } from "../conditions/ConditionCards";
import { panelTitle, panelText } from "../panelStrings";
import type { GameContext } from "../../../GameContext";
import type { SelectionModel } from "../../world/SelectionModel";
import { SQUARE } from "../../viewport/squareMath";

/** The world-scene data the panel reads — injected so the panel stays input- and
 *  engine-agnostic (testable, and reusable when souls/items become selectable). */
export interface DetailsProviders {
  pawn(entity: number): {
    kind: number; stem: string; tileX: number; tileY: number; facing: number;
    macroPosition: number; moving: boolean; ticsPerTile: number;
    /** The TOML name (inventory F7 — `human_male`, `wolf`): identification is the
     *  panel's whole job now; `pawn/human/male #11` told the user nothing. */
    name: string;
    /** Does this kind's corpus carry the `inventory` need — the [Inventory] button gate. */
    hasInventory: boolean;
    /** The pawn's ACTIVE conditions (needs-moodlets P5), evaluated lazily by the provider
     *  through the ONE wasm eval. The NEED scalars are deliberately absent — the panel
     *  shows CONSEQUENCES, never bars (the stream's whole point).
     *
     *  **Order is authoritative** (conditions F3, emotions F4): the shared eval has already
     *  sorted by `priority` desc, Σ emotion magnitude desc, `condition_id` asc. The panel
     *  maximizes the first four and minimizes the rest — it never sorts, and must not. */
    conditions: ConditionCard[];
    /** The ACTIVE emotion (emotions F3/F7) — the argmax winner; `color` washes the panel. */
    emotion: { index: number; label: string; color: number };
  } | null;
  thing(primId: number): {
    textureName?: string; x: number; y: number; width: number; height: number; zIndex: number;
    /** The TOML name of the thing at the anchor cell (`shrub`, `meat`), or null unresolved. */
    name: string | null;
  } | null;
  /** The ground tile's TOML name at (x, y) (`grass`, `water`), or null while unstreamed. */
  tileName(x: number, y: number): string | null;
}

const PANEL_KEY = "gameDetailsPanel";

export class DetailsPanel extends DomPanel {
  /** The panel's content root. Still a row so the emotion wash has one element to tint. */
  private readonly rowEl = document.createElement("div");
  private readonly bodyEl = document.createElement("div");
  /** The top BUTTON ROW (inventory F7/I11) — panel-opening buttons land here;
   *  [Inventory] is the first, gated per selection. */
  private readonly buttonRowEl = document.createElement("div");
  private readonly inventoryBtn = document.createElement("button");
  private readonly textEl = document.createElement("div");
  private readonly unsubSel: () => void;
  private readonly timer: number;

  constructor(ctx: GameContext, private readonly selection: SelectionModel,
              private readonly providers: DetailsProviders) {
    super({
      title: panelTitle(PANEL_KEY),
      storageKey: "details",
      zOrder: Z_TIER_INFO, // bug-sweep F1: the user's table — details at 40
      minWidth: 220,
      minHeight: 150,
      taskbar: ctx.taskbar,
      pinned: true,
      uiEditMode: ctx.uiEditMode,
    });
    // No reserves. The bottom band (for cards that were never children) and the left shift
    // (for the intent column) both went with their tenants — leaving either behind would give
    // this panel a permanent empty margin that reads as a layout bug (I7).
    this.rowEl.style.cssText = "display:flex;height:100%;box-sizing:border-box;";
    this.bodyEl.style.cssText =
      "padding:var(--ui-pad) var(--ui-pad);overflow:auto;height:100%;display:flex;" +
      "flex-direction:column;box-sizing:border-box;flex:1 1 auto;";
    // The button row (inventory I11): a named ROW, not "the inventory button" — the
    // next panel button (character sheet…) lands beside it without rewiring.
    this.buttonRowEl.style.cssText = "display:flex;gap:6px;margin-bottom:6px;flex:0 0 auto;";
    this.inventoryBtn.textContent = panelText(PANEL_KEY, "inventoryButton");
    this.inventoryBtn.style.cssText =
      "font:11px monospace;padding:2px 10px;cursor:pointer;background:rgba(255,255,255,0.08);" +
      "color:inherit;border:1px solid rgba(255,255,255,0.25);border-radius:3px;display:none;";
    this.inventoryBtn.addEventListener("mousedown", (e) => {
      e.stopPropagation();
      this.onInventoryClick?.();
    });
    this.buttonRowEl.appendChild(this.inventoryBtn);
    this.textEl.style.cssText = "font:12px/1.7 monospace;white-space:pre;flex:1 1 auto;";
    this.bodyEl.appendChild(this.buttonRowEl);
    this.bodyEl.appendChild(this.textEl);
    this.rowEl.appendChild(this.bodyEl);
    this.setBody(this.rowEl);
    this.unsubSel = selection.subscribe(() => this.render());
    // Pawns move while selected — refresh the live rows on a slow tick (the selection event
    // only fires on selection CHANGES, not on the pawn's motion).
    this.timer = window.setInterval(() => {
      if (this.selection.primary?.kind === "pawn") this.render();
    }, 500);
    this.render();
  }

  /** Injected by the scene (inventory F7) — opens the inventory panel for the
   *  active pawn. The button only renders when the selection HAS an inventory. */
  onInventoryClick: (() => void) | null = null;

  private render(): void {
    const rows: string[] = [];
    /** The panel wash (emotions F7): the active emotion's color, alpha-dimmed over the
     *  panel's dark base. `""` (no wash) for every non-pawn selection. */
    let wash = "";
    const all = this.selection.all;
    const p = this.selection.primary;
    /** The [Inventory] button's gate this render (inventory F7): no inventory need on
     *  the selected object's kind → no button (the user's law). */
    let showInventory = false;
    if (!p) {
      this.textEl.textContent = panelText(PANEL_KEY, "empty");
      this.inventoryBtn.style.display = "none";
      this.rowEl.style.background = "";
      return;
    }
    if (all.length > 1) rows.push(`${all.length} selected — primary:`);
    // The panel's whole job is IDENTIFICATION (inventory F7, the user at plan review:
    // "I just need to know what something is" — `thing #26335, texture (geo)` hid that
    // a green square was a shrub). NAME from the TOML + the TILE; everything else lives
    // in its sibling panels and the wash.
    if (p.kind === "pawn") {
      const info = this.providers.pawn(p.entity);
      if (info) {
        rows.push(info.name, `tile  ${info.tileX}, ${info.tileY}`);
        showInventory = info.hasInventory;
        const c = info.emotion.color;
        wash = `rgba(${(c >> 16) & 0xff}, ${(c >> 8) & 0xff}, ${c & 0xff}, 0.16)`;
      } else {
        rows.push(`0x${p.entity.toString(16)}`, "(despawned)");
      }
    } else if (p.kind === "thing") {
      const t = this.providers.thing(p.primId);
      if (t) {
        const tx = Math.floor((t.x + t.width / 2) / SQUARE);
        const ty = Math.floor((t.y + t.height) / SQUARE);
        rows.push(t.name ?? t.textureName ?? "(unknown thing)", `tile  ${tx}, ${ty}`);
      } else {
        rows.push("(streamed out)");
      }
    } else {
      rows.push(this.providers.tileName(p.x, p.y) ?? "(unstreamed)", `tile  ${p.x}, ${p.y}`);
    }
    this.textEl.textContent = rows.join("\n");
    this.inventoryBtn.style.display = showInventory ? "" : "none";
    this.rowEl.style.background = wash;
  }

  destroy(): void {
    this.unsubSel();
    clearInterval(this.timer);
    super.destroy();
  }
}
