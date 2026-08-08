//! The inventory panel (inventory F7) — the ACTIVE pawn's held items as a GRID of
//! squares. Capacity (the grid size) is the `inventory` need's EFFECTIVE max through
//! the ONE wasm eval (`needMax` — a level-2 trait widens the grid); FILLED squares
//! come from the {@link InventoryStore} ROWS (never `max − need` arithmetic — I2).
//! Hidden entirely while the active object has no inventory need. A slot click
//! reports up — the scene opens the slot pie menu (drop) on it.

import { DomPanel } from "../../ui/dom/DomPanel";
import { panelTitle, panelText } from "./panelStrings";
import type { GameContext } from "../../GameContext";

const PANEL_KEY = "gameInventoryPanel";
const CELL = 44;
const GAP = 6;
const PER_ROW = 3;

/** What the scene injects — the panel stays selection- and wasm-agnostic. */
export interface InventoryDeps {
  /** The effective capacity of the pawn's inventory need, or null = no inventory. */
  capacity(entity: number): number | null;
  /** The pawn's filled slots (slot → item definition_reference). */
  slots(entity: number): ReadonlyMap<number, number>;
  /** An item def's display name (the TOML name) — the square's tooltip. */
  itemName(item: number): string;
  /** An item def's placeholder colour (`#rrggbb`), or "" for the neutral fill. */
  itemColor(item: number): string;
  /** A FILLED slot was clicked at client (x, y) — the scene opens the slot menu. */
  onSlotClick(entity: number, slot: number, item: number, x: number, y: number): void;
}

export class InventoryPanel extends DomPanel {
  private readonly gridEl = document.createElement("div");
  private target: number | null = null;

  constructor(ctx: GameContext, private readonly deps: InventoryDeps) {
    super({
      title: panelTitle(PANEL_KEY),
      storageKey: "inventory",
      minWidth: 3 * (CELL + GAP) + 24,
      minHeight: 2 * (CELL + GAP) + 48,
      taskbar: ctx.taskbar,
      pinned: true,
      uiEditMode: ctx.uiEditMode,
    });
    this.gridEl.style.cssText =
      `display:grid;grid-template-columns:repeat(${PER_ROW}, ${CELL}px);gap:${GAP}px;` +
      "padding:12px;align-content:start;";
    this.setBody(this.gridEl);
  }

  /** The ACTIVE pawn this panel mirrors; null (or an inventory-less pawn) HIDES the
   *  panel (the user's law: no inventory → no panel). */
  setTarget(entity: number | null): void {
    const cap = entity !== null ? this.deps.capacity(entity) : null;
    this.target = cap !== null ? entity : null;
    if (this.target === null) {
      if (this.isOpen) this.close();
      return;
    }
    this.render();
  }

  /** Open for the current target (the details panel's [Inventory] button). No-op
   *  without one — the button is gated the same way, so this is belt-and-braces. */
  show(): void {
    if (this.target === null) return;
    if (!this.isOpen) this.open();
    this.render();
  }

  /** Re-render on store changes (the scene subscribes and calls through). */
  refresh(): void {
    if (this.target !== null && this.isOpen) this.render();
  }

  private render(): void {
    const entity = this.target;
    this.gridEl.textContent = "";
    if (entity === null) return;
    const cap = this.deps.capacity(entity);
    if (cap === null || cap <= 0) {
      this.gridEl.textContent = panelText(PANEL_KEY, "empty");
      return;
    }
    const slots = this.deps.slots(entity);
    for (let s = 0; s < cap; s++) {
      const cell = document.createElement("div");
      const item = slots.get(s);
      cell.style.cssText =
        `width:${CELL}px;height:${CELL}px;box-sizing:border-box;border-radius:4px;` +
        "border:1px solid rgba(255,255,255,0.18);";
      if (item !== undefined) {
        const color = this.deps.itemColor(item);
        cell.style.background = color || "rgba(255,255,255,0.10)";
        // The placeholder-outline law (food-chain F9) carries into the UI: a filled
        // square reads as an OBJECT, not a glitch.
        cell.style.border = "2px solid #000";
        cell.title = this.deps.itemName(item);
        cell.style.cursor = "pointer";
        cell.addEventListener("mousedown", (e) => {
          e.stopPropagation();
          this.deps.onSlotClick(entity, s, item, e.clientX, e.clientY);
        });
      } else {
        cell.style.background = "rgba(0,0,0,0.25)";
      }
      this.gridEl.appendChild(cell);
    }
  }
}
