//! The details panel (ui-select P2) — re-implemented on `DomPanel`, displaying the SELECTED
//! object's details live from the {@link SelectionModel} + the world's providers. One panel,
//! three shapes: a PAWN (kind/stem, entity ref, authoritative tile, facing, speed, zone
//! address, in-flight flag — refreshed on a short timer while selected, since pawns move), a
//! THING (texture, position, footprint), a TILE (world coordinates + world px). Empty state
//! when nothing is selected. Consumes the model's change event only (D1) — it never touches
//! input, so the future drag-box multi-select lands here as "N selected" without rewiring.

import { DomPanel } from "../../../ui/dom/DomPanel";
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
  } | null;
  thing(primId: number): {
    textureName?: string; x: number; y: number; width: number; height: number; zIndex: number;
  } | null;
}

const PANEL_KEY = "gameDetailsPanel";
const FACING = ["south", "east", "north", "west"];

export class DetailsPanel extends DomPanel {
  private readonly bodyEl = document.createElement("div");
  private readonly unsubSel: () => void;
  private readonly timer: number;

  constructor(ctx: GameContext, private readonly selection: SelectionModel,
              private readonly providers: DetailsProviders) {
    super({
      title: panelTitle(PANEL_KEY),
      storageKey: "details",
      minWidth: 220,
      minHeight: 150,
      taskbar: ctx.taskbar,
      pinned: true,
      uiEditMode: ctx.uiEditMode,
    });
    this.bodyEl.style.cssText = "padding:8px 12px;font:12px/1.7 monospace;white-space:pre;overflow:auto;height:100%;box-sizing:border-box;";
    this.setBody(this.bodyEl);
    this.unsubSel = selection.subscribe(() => this.render());
    // Pawns move while selected — refresh the live rows on a slow tick (the selection event
    // only fires on selection CHANGES, not on the pawn's motion).
    this.timer = window.setInterval(() => {
      if (this.selection.primary?.kind === "pawn") this.render();
    }, 500);
    this.render();
  }

  private render(): void {
    const rows: string[] = [];
    const all = this.selection.all;
    const p = this.selection.primary;
    if (!p) {
      this.bodyEl.textContent = panelText(PANEL_KEY, "empty");
      return;
    }
    if (all.length > 1) rows.push(`${all.length} selected — primary:`);
    if (p.kind === "pawn") {
      const info = this.providers.pawn(p.entity);
      rows.push(`pawn      0x${p.entity.toString(16)}`);
      if (info) {
        rows.push(
          `kind      ${info.stem} (#${info.kind})`,
          `tile      ${info.tileX}, ${info.tileY}`,
          `facing    ${FACING[info.facing & 3]}`,
          `speed     ${info.ticsPerTile} tics/tile`,
          `zone      ${info.macroPosition}`,
          `state     ${info.moving ? "moving" : "resting"}`,
        );
      } else {
        rows.push("(despawned)");
      }
    } else if (p.kind === "thing") {
      const t = this.providers.thing(p.primId);
      rows.push(`thing     #${p.primId}`);
      if (t) {
        rows.push(
          `texture   ${t.textureName ?? "(geo)"}`,
          `tile      ${Math.floor((t.x + t.width / 2) / SQUARE)}, ${Math.floor((t.y + t.height) / SQUARE)}`,
          `footprint ${Math.round(t.width / SQUARE)}×${Math.round(t.height / SQUARE)} tiles`,
        );
      } else {
        rows.push("(streamed out)");
      }
    } else {
      rows.push(
        `tile      ${p.x}, ${p.y}`,
        `world px  ${p.x * SQUARE}, ${p.y * SQUARE}`,
      );
    }
    this.bodyEl.textContent = rows.join("\n");
  }

  destroy(): void {
    this.unsubSel();
    clearInterval(this.timer);
    super.destroy();
  }
}
