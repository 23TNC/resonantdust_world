//! The LIVE EDIT panel — everything the shared eval knows about the selected object, in one
//! place, live (`2026-08-09-live-edit`). Opened by the `/edit` chat command.
//!
//! A fifth selection surface, and deliberately a different KIND from the other four. Details /
//! intentions / conditions each answer one question at a glance while you play; this answers all
//! of them at once, on demand, for someone tuning the corpus — which is why it is a command
//! rather than a panel that is simply open.
//!
//! Design: `docs/components/client/webgl/design/live-edit.md`.
//!
//! Two rules shape the code:
//!
//!  - **ONE eval snapshot per refresh, sliced four ways** (F5). Every tab describes the same pawn
//!    at the same tic through the same eval; four tabs fetching independently would run it four
//!    times a poll and could show four different instants inside one panel. Only the ACTIVE tab
//!    renders, so the heaviest tab costs nothing while hidden.
//!  - **The tab strip is this panel's OWN widget**, not `DomPanel.addTab` — which is mutually
//!    exclusive with `setBody`, and the preview has to persist across tab switches rather than be
//!    duplicated into every tab's content. Styled to match the built-in strip so the app stays
//!    coherent.
//!
//! READ-ONLY (F6). The name anticipates mutation and the layout leaves room for it, but nothing
//! here writes: a write surface over a read surface that is subtly wrong produces edits nobody
//! intended, and this one carries two derived quantities (the need rate, the effective clamp)
//! that are easy to get quietly wrong.

import { DomPanel, Z_TIER_SYSTEM } from "../../../ui/dom/DomPanel";
import { TAB_BTN_CSS, TAB_BTN_ACTIVE_CSS, TABS_CSS } from "../../../ui/dom/DomPanelStyles";
import { panelTitle } from "../panelStrings";
import type { GameContext } from "../../../GameContext";
import type { SelectionModel } from "../../world/SelectionModel";
import type { LiveEditSnapshot, LiveEditProviders } from "./snapshot";
import { EMPTY_SNAPSHOT } from "./snapshot";
import { TraitsTab } from "./TraitsTab";
import { NeedsTab } from "./NeedsTab";
import { ConditionsTab } from "./ConditionsTab";
import { EmotionsTab } from "./EmotionsTab";

const PANEL_KEY = "liveEditPanel";

/** What every tab implements. Deliberately tiny: a tab owns an element and knows how to render
 *  a snapshot into it. Nothing fetches — the panel hands it the slice. */
export interface LiveEditTab {
  readonly id: string;
  readonly label: string;
  readonly element: HTMLElement;
  render(snap: LiveEditSnapshot): void;
}

export class LiveEditPanel extends DomPanel {
  private readonly previewEl = document.createElement("div");
  private readonly stripEl = document.createElement("div");
  private readonly contentEl = document.createElement("div");
  private readonly editTabs: LiveEditTab[];
  private readonly tabButtons = new Map<string, HTMLButtonElement>();
  private activeId: string;
  private readonly unsubSel: () => void;
  private readonly timer: number;
  /** The last snapshot rendered, by identity — so a poll that changes nothing does not rebuild
   *  the DOM under the user's cursor (the tooltip lesson from the conditions cards). */
  private lastKey = "";

  constructor(ctx: GameContext, private readonly selection: SelectionModel,
              private readonly providers: LiveEditProviders) {
    super({
      title: panelTitle(PANEL_KEY),
      storageKey: "liveEdit",
      // A tuning surface, so it belongs with the other system panels rather than over the
      // gameplay ones (bug-sweep F1's table).
      zOrder: Z_TIER_SYSTEM,
      taskbar: ctx.taskbar,
      pinned: false,
      closable: true,
      taskbarIcon: "✎",
      uiEditMode: ctx.uiEditMode,
      // Generous — a preview above a tabbed table needs room (I7).
      defaultCell: { col: 16, row: 4, cols: 24, rows: 22 },
      // The first panel with a real reason to raise the 1x1 floor: below this the preview and
      // the tab strip stop being simultaneously usable.
      minCols: 12,
      minRows: 10,
    });

    // ── the preview region (top center) ──
    this.previewEl.style.cssText =
      "flex:0 0 45%;display:flex;align-items:center;justify-content:center;" +
      "min-height:0;overflow:hidden;border-bottom:1px solid #3a3a4a;" +
      "position:relative;box-sizing:border-box;";

    // ── the tab strip, styled like the built-in one ──
    Object.assign(this.stripEl.style, TABS_CSS);
    this.stripEl.style.flex = "0 0 auto";

    this.contentEl.style.cssText =
      "flex:1 1 auto;min-height:0;overflow:auto;padding:var(--ui-pad-sm) var(--ui-pad);" +
      "box-sizing:border-box;";

    this.editTabs = [
      new TraitsTab(),
      new NeedsTab(),
      new ConditionsTab(),
      new EmotionsTab(),
    ];
    this.activeId = this.storageGet("liveEditTab") ?? this.editTabs[0].id;
    if (!this.editTabs.some((t) => t.id === this.activeId)) this.activeId = this.editTabs[0].id;

    for (const tab of this.editTabs) {
      const btn = document.createElement("button");
      Object.assign(btn.style, TAB_BTN_CSS);
      btn.textContent = tab.label;
      btn.addEventListener("click", (e) => {
        e.stopPropagation();
        this.selectEditTab(tab.id);
      });
      this.stripEl.appendChild(btn);
      this.tabButtons.set(tab.id, btn);
      this.contentEl.appendChild(tab.element);
    }

    const root = document.createElement("div");
    root.style.cssText = "display:flex;flex-direction:column;height:100%;box-sizing:border-box;";
    root.appendChild(this.previewEl);
    root.appendChild(this.stripEl);
    root.appendChild(this.contentEl);
    this.setBody(root);
    this.applyTabStyles();
    // The preview is NOT mounted yet — see live-edit `blockers.md`. A second `Viewport` has its
    // own GL context and camera but NO CONTENT: `MoverLayer` and `WorldBridge` each hold ONE
    // viewport and push every prim into it, so a second instance renders empty space. Mounting
    // a live-but-blank canvas here would read as a broken panel, so the region says what it is
    // waiting for instead. `PreviewViewport` is built and ready for whichever way that call goes.
    const pending = document.createElement("div");
    pending.style.cssText =
      "font:var(--ui-font)/1.5 monospace;color:#7a7a8a;text-align:center;padding:var(--ui-pad);";
    pending.textContent = "preview — pending a content feed (see blockers)";
    this.previewEl.appendChild(pending);

    this.unsubSel = selection.subscribe(() => this.refresh(true));
    // Needs decay and conditions expire while a pawn is selected, and the selection event only
    // fires on selection CHANGES — so poll, at the cadence the other selection surfaces use.
    this.timer = window.setInterval(() => this.refresh(false), 500);
    this.refresh(true);
  }

  /** The preview's mount point, for whatever the spike (F1) decides to put there. */
  get previewHost(): HTMLElement { return this.previewEl; }

  private selectEditTab(id: string): void {
    if (this.activeId === id) return;
    this.activeId = id;
    this.storageSet("liveEditTab", id);
    this.applyTabStyles();
    // Render on activation: a hidden tab is deliberately not kept up to date (F5).
    this.lastKey = "";
    this.refresh(true);
  }

  private applyTabStyles(): void {
    for (const tab of this.editTabs) {
      const btn = this.tabButtons.get(tab.id);
      if (btn) {
        Object.assign(btn.style, TAB_BTN_CSS);
        if (tab.id === this.activeId) Object.assign(btn.style, TAB_BTN_ACTIVE_CSS);
      }
      tab.element.style.display = tab.id === this.activeId ? "" : "none";
    }
  }

  /** Take ONE snapshot and hand it to the active tab (F5). `force` skips the identity check —
   *  used when the selection or the active tab changed, where the key may match by coincidence. */
  private refresh(force: boolean): void {
    const p = this.selection.primary;
    const snap = p?.kind === "pawn"
      ? (this.providers.snapshot(p.entity) ?? EMPTY_SNAPSHOT)
      : EMPTY_SNAPSHOT;
    if (!force && snap.key === this.lastKey) return;
    this.lastKey = snap.key;
    const active = this.editTabs.find((t) => t.id === this.activeId);
    active?.render(snap);
  }

  destroy(): void {
    this.unsubSel();
    clearInterval(this.timer);
    super.destroy();
  }
}
