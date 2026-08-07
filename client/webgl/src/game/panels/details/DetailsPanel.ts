//! The details panel (ui-select P2) — re-implemented on `DomPanel`, displaying the SELECTED
//! object's details live from the {@link SelectionModel} + the world's providers. One panel,
//! three shapes: a PAWN (kind/stem, entity ref, authoritative tile, facing, speed, zone
//! address, in-flight flag — refreshed on a short timer while selected, since pawns move), a
//! THING (texture, position, footprint), a TILE (world coordinates + world px). Empty state
//! when nothing is selected. Consumes the model's change event only (D1) — it never touches
//! input, so the future drag-box multi-select lands here as "N selected" without rewiring.

import { DomPanel } from "../../../ui/dom/DomPanel";
import { ConditionCards, CARD_H, PAD_BOTTOM } from "./ConditionCards";
import type { ConditionCard } from "./ConditionCards";
import { IntentStrip } from "./IntentStrip";
import type { QueueVisual } from "./IntentStrip";
import { panelTitle, panelText } from "../panelStrings";
import type { GameContext } from "../../../GameContext";
import type { SelectionModel } from "../../world/SelectionModel";
import type { IntentQueues } from "../../world/IntentQueues";
import { SQUARE } from "../../viewport/squareMath";
import { getContent } from "../../definitions/contentBoot";

/** The world-scene data the panel reads — injected so the panel stays input- and
 *  engine-agnostic (testable, and reusable when souls/items become selectable). */
export interface DetailsProviders {
  pawn(entity: number): {
    kind: number; stem: string; tileX: number; tileY: number; facing: number;
    macroPosition: number; moving: boolean; ticsPerTile: number;
    /** The pawn's ACTIVE conditions + summed mood (needs-moodlets P5), evaluated lazily by
     *  the provider through the ONE wasm eval. The NEED scalars are deliberately absent —
     *  the panel shows CONSEQUENCES, never bars (the stream's whole point).
     *
     *  **Order is authoritative** (conditions F3): the shared eval has already sorted by
     *  `priority` desc, `|mood|` desc, `condition_id` asc. The panel maximizes the first four
     *  and minimizes the rest — it never sorts, and must not. */
    conditions: { label: string; mood: number; remaining: number; priority: number }[];
    mood: number;
  } | null;
  thing(primId: number): {
    textureName?: string; x: number; y: number; width: number; height: number; zIndex: number;
  } | null;
}

const PANEL_KEY = "gameDetailsPanel";
const FACING = ["south", "east", "north", "west"];

export class DetailsPanel extends DomPanel {
  /** The flex ROW (intent-queue-ui F5): the strip pinned LEFT, the text content
   *  SHIFTED RIGHT beside it. */
  private readonly rowEl = document.createElement("div");
  private readonly bodyEl = document.createElement("div");
  private readonly unsubSel: () => void;
  private readonly unsubQueues: () => void;
  private readonly timer: number;
  /** The condition strip — a SIBLING of this panel, not a child, so it can draw past the
   *  panel's right edge (conditions F7 / B1). Owned here, destroyed here. */
  private readonly cards: ConditionCards;
  /** The intent-queue strip (intent-queue-ui F5) — circles from the TOML visuals. */
  private readonly strip: IntentStrip;

  constructor(ctx: GameContext, private readonly selection: SelectionModel,
              private readonly providers: DetailsProviders,
              private readonly queues: IntentQueues) {
    super({
      title: panelTitle(PANEL_KEY),
      storageKey: "details",
      minWidth: 220,
      minHeight: 150,
      taskbar: ctx.taskbar,
      pinned: true,
      uiEditMode: ctx.uiEditMode,
    });
    // The body reserves the strip's band at the bottom so a long text block scrolls to a stop
    // ABOVE the cards instead of underneath them — the strip floats over the panel, so without
    // the reserve the last row would hide behind it.
    this.rowEl.style.cssText = "display:flex;height:100%;box-sizing:border-box;";
    this.bodyEl.style.cssText =
      "padding:8px 12px;font:12px/1.7 monospace;white-space:pre;overflow:auto;height:100%;" +
      `box-sizing:border-box;flex:1 1 auto;padding-bottom:${CARD_H + PAD_BOTTOM * 2}px;`;
    this.strip = new IntentStrip(
      (ref) => {
        try {
          return getContent().queueVisual(ref) as QueueVisual | null;
        } catch {
          return null;
        }
      },
      () => {
        const d = ctx.client.ticDelta(0);
        if (d === null) return null;
        return ((Math.floor(d) % 0x10000) + 0x10000) % 0x10000;
      },
      (entryId) => this.onCancelClick(entryId),
    );
    this.rowEl.appendChild(this.strip.el);
    this.rowEl.appendChild(this.bodyEl);
    this.setBody(this.rowEl);
    this.cards = new ConditionCards({
      storageKey: "details",
      rect: () => this.panel.getBoundingClientRect(),
      zIndex: () => Number.parseInt(this.panel.style.zIndex, 10) || 0,
      // `display: none` covers the taskbar-hide path, which has no public flag of its own.
      visible: () => this.isOpen && !this.isMinimized && this.panel.style.display !== "none",
      onRectChange: (cb) => this.onRectChange(() => cb()),
      onFocus: (cb) => this.onFocus(cb),
      onMinimizeChange: (cb) => this.onMinimizeChange(() => cb()),
      onOpenChange: (cb) => this.onOpenChange(() => cb()),
    });
    this.unsubSel = selection.subscribe(() => this.render());
    // The strip re-renders on every queue fan for the selected pawn.
    this.unsubQueues = queues.subscribe(() => {
      if (this.selection.primary?.kind === "pawn") this.render();
    });
    // Pawns move while selected — refresh the live rows on a slow tick (the selection event
    // only fires on selection CHANGES, not on the pawn's motion).
    this.timer = window.setInterval(() => {
      if (this.selection.primary?.kind === "pawn") this.render();
    }, 500);
    this.render();
  }

  /** A strip-circle click attempts a cancel (intent-queue-ui F3/F4 — P4 wires the
   *  verb; the strip already reports the worker-minted entry id). */
  private onCancelClick(entryId: number): void {
    const p = this.selection.primary;
    if (p?.kind !== "pawn") return;
    this.cancelSender?.(p.entity, entryId);
  }

  /** Injected by the scene (P4) — sends `CANCEL_INTENT pawn entry_id`. */
  cancelSender: ((pawn: number, entryId: number) => void) | null = null;

  private render(): void {
    const rows: string[] = [];
    /** The conditions this render resolved — empty for every non-pawn selection, which is what
     *  clears the strip. Collected here and pushed ONCE at the end so there is exactly one
     *  place the strip can be set from. */
    let cards: ConditionCard[] = [];
    const all = this.selection.all;
    const p = this.selection.primary;
    if (!p) {
      this.bodyEl.textContent = panelText(PANEL_KEY, "empty");
      this.cards.setCards([]);
      this.strip.setEntries([]);
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
          `mood      ${Math.round(info.mood * 100)}%`,
        );
        // Conditions are NOT text rows any more — they render as cards in the sibling strip
        // (P4). The order arrives already sorted by the shared eval; pass it through untouched.
        cards = info.conditions;
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
    this.cards.setCards(cards);
    // The strip shows the SELECTED pawn's queue only; anything else clears it.
    this.strip.setEntries(p.kind === "pawn" ? this.queues.entriesOf(p.entity) : []);
  }

  /** The strip's computed ring percentage (drill probe — intent-queue-ui I5). */
  ringPercent(): number | null {
    return this.strip.ringPercent();
  }

  destroy(): void {
    this.unsubSel();
    this.unsubQueues();
    clearInterval(this.timer);
    this.cards.destroy();
    this.strip.destroy();
    super.destroy();
  }
}
