//! The INTENTIONS panel — the selected pawn's intent QUEUE
//! (`2026-08-09-selection-panels` F4). One of three sibling selection surfaces: details says
//! what a thing IS, this shows what it means to do, conditions shows what it feels.
//!
//! It exists because a queue is a list that grows, and it was previously a 40px column squeezed
//! against the details panel's left edge — with details' own body permanently shifted right to
//! make room for a tenant that was not its business.
//!
//! `IntentStrip` moves in UNCHANGED (F4). Its progress ring is COMPUTED every frame from the
//! fanned `(started, fire)` tics through the learned tic estimate — never incremented — so a
//! hidden tab snaps to truth on re-show. That contract is subtle enough that re-laying it out
//! horizontally is a separate job from re-housing it; a strip that reflows to its panel's aspect
//! is the named successor.

import { DomPanel, Z_TIER_INFO } from "../../../ui/dom/DomPanel";
import { IntentStrip } from "./IntentStrip";
import type { QueueVisual } from "./IntentStrip";
import { panelTitle } from "../panelStrings";
import type { GameContext } from "../../../GameContext";
import type { SelectionModel } from "../../world/SelectionModel";
import type { IntentQueues } from "../../world/IntentQueues";
import { getContent } from "../../definitions/contentBoot";

const PANEL_KEY = "intentionsPanel";

export class IntentionsPanel extends DomPanel {
  private readonly strip: IntentStrip;
  private readonly unsubSel: () => void;
  private readonly unsubQueues: () => void;

  /** Injected by the scene — sends `CANCEL_INTENT pawn entry_id`. */
  cancelSender: ((pawn: number, entryId: number) => void) | null = null;

  constructor(ctx: GameContext, private readonly selection: SelectionModel,
              private readonly queues: IntentQueues) {
    super({
      title: panelTitle(PANEL_KEY),
      storageKey: "intentions",
      zOrder: Z_TIER_INFO, // a sibling of details (bug-sweep F1's table)
      taskbar: ctx.taskbar,
      pinned: true,
      taskbarIcon: "⚙",
      uiEditMode: ctx.uiEditMode,
      // A tall, narrow column beside details — the shape the strip already is.
      defaultCell: { col: 12, row: 20, cols: 4, rows: 11 },
    });
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
    const holder = document.createElement("div");
    // NEVER scroll. The strip is fluid — its circles take `min(100%, cap)`, so
    // they fit whatever width the panel has, down to one grid cell. A
    // scrollbar in a one-cell-wide panel would consume most of the panel
    // (user, 2026-08-09: the icons "need to fit inside of it without creating
    // scroll bars"). Vertical overflow clips: more queued intents than the
    // panel is tall is a sizing choice, not something to grow a bar for.
    holder.style.cssText = "display:flex;height:100%;box-sizing:border-box;overflow:hidden;";
    holder.appendChild(this.strip.el);
    this.setBody(holder);

    this.unsubSel = selection.subscribe(() => this.render());
    this.unsubQueues = queues.subscribe(() => this.render());
    this.render();
  }

  private onCancelClick(entryId: number): void {
    const p = this.selection.primary;
    if (p?.kind !== "pawn") return;
    this.cancelSender?.(p.entity, entryId);
  }

  private render(): void {
    const p = this.selection.primary;
    // The strip shows the SELECTED pawn's queue only; anything else clears it.
    this.strip.setEntries(p?.kind === "pawn" ? this.queues.entriesOf(p.entity) : []);
  }

  /** The strip's computed ring percentage (drill probe — intent-queue-ui I5). */
  ringPercent(): number | null {
    return this.strip.ringPercent();
  }

  destroy(): void {
    this.unsubSel();
    this.unsubQueues();
    this.strip.destroy();
    super.destroy();
  }
}
