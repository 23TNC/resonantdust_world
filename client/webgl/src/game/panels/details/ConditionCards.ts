//! The details panel's CONDITION CARDS — a horizontal strip along the bottom of the panel,
//! showing what a selected pawn is currently feeling / suffering (`2026-08-04-conditions` P4).
//!
//! **The strip is a SIBLING of the panel, not a child** ([F7]). `PANEL_CSS` sets
//! `overflow: hidden` on the panel root, so anything parented inside `DetailsPanel` is clipped
//! at the panel border and a strip wider than the panel would merely scroll inside it. The user
//! asked for cards that "actually draw past the details", so this element is appended to the
//! panel's HOST and positioned against the panel's rect — outside the clip, free to extend right
//! across the world.
//!
//! It lives in the overlay in BOTH states — collapsed (top 4 maximized, the rest minimized) and
//! expanded (all maximized). One layout, one code path, and no reparent at the moment the user
//! clicks; a strip that lived in the body until expansion would also inherit the body's scroll
//! and slide away from the bottom edge as the text rows scrolled.
//!
//! Ordering is NOT decided here. `needs_eval::active_conditions` has already sorted by
//! `priority` desc, `|mood|` desc, `condition_id` asc (conditions F3) — this file renders the
//! order it is given. A sort in TS would be a second implementation of a corpus rule.

/** One condition as the panel receives it — the shared eval's row, labelled. */
export interface ConditionCard {
  label: string;
  /** Mood offset while active, `-1..1`. One effect of several to come (F6) — not the identity
   *  of the condition, which is why the card leads with the LABEL and not this number. */
  mood: number;
  /** Tics left on a TIMED grant; `0` = DERIVED (alive exactly while its band holds). */
  remaining: number;
  /** The authored sort key, carried for display. Never re-sorted here. */
  priority: number;
}

// ── layout constants ────────────────────────────────────────────────────────────────
// Sized so FOUR maximized cards plus their padding fit the details panel's default width
// (~355 px): 8 + 4×80 + 3×6 = 346. Past four the strip simply keeps going — that is the whole
// point of being a sibling.

/** Maximized card width, px. */
export const CARD_W = 80;
/** Card height, px — two 12px monospace lines plus padding. */
export const CARD_H = 42;
/** Gap between cards, px. */
export const CARD_GAP = 6;
/** Inset from the panel's LEFT edge, px. */
export const PAD_LEFT = 8;
/** The intent strip's column width (intent-queue-ui) — the cards' origin clears it so
 *  the bottom-most circle and the first card never overlap. Mirrors `IntentStrip`'s
 *  `STRIP_W` (kept as a literal to avoid a layout import cycle). */
const INTENT_STRIP_W = 40;
/** Inset from the panel's BOTTOM edge, px. */
export const PAD_BOTTOM = 8;
/** How many cards show maximized before the rest minimize (the user's number). */
export const MAXIMIZED = 4;
/** A minimized card's width as a FRACTION of a maximized one — the horizontal saving. */
export const MINIMIZED_FRACTION = 0.35;
/** Minimized card width, px (derived; kept as a constant so tests and CSS agree). */
export const CARD_W_MIN = Math.round(CARD_W * MINIMIZED_FRACTION);
/** Gap kept between the strip and the viewport edges when it has to be clamped, px. */
export const EDGE_MARGIN = 8;

const CARD_BG = "rgba(28, 31, 42, 0.96)";
const CARD_BG_HOVER = "rgba(44, 48, 64, 0.98)";
const CARD_BORDER = "#3a3a4a";
const CARD_BORDER_HOVER = "#6b6b86";
const GOOD = "#8fce7a";
const BAD = "#e08a7a";

/** What `ConditionCards` needs from the panel it hangs off — passed in rather than reached for,
 *  so this file never imports `DomPanel` and stays testable in isolation. */
export interface CardsHost {
  /** The panel's `storageKey`, so the expanded flag persists beside its other prefs
   *  (`<key>.conditionsExpanded`). `null` disables persistence, as it does on the panel. */
  storageKey: string | null;
  /** The panel root's CURRENT viewport rect. */
  rect(): DOMRect;
  /** The panel root's current z-index, so the strip can sit exactly one above it. */
  zIndex(): number;
  /** Whether the panel is presently on screen (open, not minimized, not taskbar-hidden). */
  visible(): boolean;
  /** Panel rect changed — drag, resize, snap flip, window resize, minimize toggle. */
  onRectChange(cb: () => void): () => void;
  /** Panel focus changed (its z-index may have moved within its band). */
  onFocus(cb: () => void): () => void;
  /** Panel minimized / restored. */
  onMinimizeChange(cb: () => void): () => void;
  /** Panel opened / closed. */
  onOpenChange(cb: () => void): () => void;
}

export class ConditionCards {
  private readonly el = document.createElement("div");
  private readonly unsubs: (() => void)[] = [];
  private cards: ConditionCard[] = [];
  /** All cards maximized. Collapsed = top `MAXIMIZED` maximized, the rest minimized.
   *
   *  PANEL state, not selection state (F5): the user set it deliberately, so selecting a
   *  different pawn must not silently re-collapse it and make them click again. Persisted with
   *  the panel's other prefs. */
  private expanded: boolean;
  /** A debug pin owns the strip — see [`pin`]. */
  private pinned = false;

  constructor(private readonly host: CardsHost) {
    this.expanded = this.loadExpanded();
    this.el.dataset.rdConditionStrip = "1";
    this.el.style.cssText = [
      "position:fixed",
      "display:none",
      `gap:${CARD_GAP}px`,
      "align-items:flex-end",
      "font:12px/1.35 ui-monospace, monospace",
      // The container is a pass-through: only the cards themselves take pointer events, so the
      // gaps between them (and the strip's empty tail) never swallow a world click.
      "pointer-events:none",
    ].join(";");
    // SIBLING of the panel — the same host element `DomPanel` mounts into. Being outside the
    // panel root is precisely what lets the strip paint past the panel's right edge.
    const anchor = document.getElementById("app") ?? document.body;
    anchor.appendChild(this.el);

    // Debug hook (repo convention: `window.__*`). `__cards(6)` pins six synthetic conditions,
    // `__cards(rows)` pins a given set, `__cards(null)` releases back to the live pawn.
    (window as unknown as Record<string, unknown>).__cards = (
      arg: number | ConditionCard[] | null,
    ) => {
      this.pin(typeof arg === "number" ? synthetic(arg) : arg);
      return this.cards.length;
    };

    const reflow = () => this.reflow();
    this.unsubs.push(
      this.host.onRectChange(reflow),
      this.host.onFocus(reflow),
      this.host.onMinimizeChange(reflow),
      this.host.onOpenChange(reflow),
    );
    window.addEventListener("resize", reflow);
    this.unsubs.push(() => window.removeEventListener("resize", reflow));
  }

  /** Replace the displayed set. Pass `[]` (or a non-pawn selection) to clear the strip. */
  setCards(cards: ConditionCard[]): void {
    if (this.pinned) return; // a debug pin owns the strip until it is released
    // Cheap identity check — the panel re-renders on a 500 ms timer while a pawn is selected,
    // and rebuilding these nodes every tick would drop a hover/click on the floor.
    if (sameCards(this.cards, cards)) {
      this.reflow();
      return;
    }
    this.cards = cards;
    this.rebuild();
    this.reflow();
  }

  /** **Debug only** — hold a synthetic set on the strip, ignoring the panel's live renders,
   *  until called with `null`. The authored corpus has three conditions and two of them are
   *  DERIVED on one need, so a real pawn cannot carry enough at once to exercise the
   *  4-maximized rule or the viewport clamp. Reached as `window.__cards(n | rows | null)`. */
  pin(cards: ConditionCard[] | null): void {
    this.pinned = cards !== null;
    this.cards = cards ?? [];
    this.rebuild();
    this.reflow();
  }

  destroy(): void {
    for (const off of this.unsubs) off();
    this.unsubs.length = 0;
    this.el.remove();
  }

  // ── internals ──────────────────────────────────────────────────────────────────────

  private rebuild(): void {
    this.el.replaceChildren();
    this.cards.forEach((c, i) => this.el.appendChild(this.buildCard(c, i)));
  }

  /** One card: the label, then its mood offset and (for a timed grant) the tics it has left.
   *  A minimized card keeps the same node and only narrows — the label ellipsises, so the strip
   *  still reads as "there are more of these" rather than going blank. */
  private buildCard(c: ConditionCard, index: number): HTMLElement {
    const minimized = !this.expanded && index >= MAXIMIZED;
    const el = document.createElement("div");
    el.dataset.rdCondition = c.label;
    el.dataset.rdMinimized = minimized ? "1" : "0";
    el.title = `${c.label}  ${signed(c.mood)}${c.remaining > 0 ? `  ${c.remaining}t` : ""}`;
    el.style.cssText = [
      `width:${minimized ? CARD_W_MIN : CARD_W}px`,
      `height:${CARD_H}px`,
      "flex:0 0 auto",
      "box-sizing:border-box",
      "padding:5px 6px",
      `background:${CARD_BG}`,
      `border:1px solid ${CARD_BORDER}`,
      "border-radius:3px",
      "color:#ecd6aa",
      "overflow:hidden",
      "pointer-events:auto",
      "cursor:pointer",
      "display:flex",
      "flex-direction:column",
      "justify-content:space-between",
      "transition:background 90ms linear, border-color 90ms linear",
    ].join(";");

    const label = document.createElement("div");
    label.textContent = c.label;
    label.style.cssText = "overflow:hidden;text-overflow:ellipsis;white-space:nowrap";

    const stat = document.createElement("div");
    stat.textContent = `${signed(c.mood)}${c.remaining > 0 ? `  ${c.remaining}t` : ""}`;
    stat.style.cssText =
      `white-space:nowrap;overflow:hidden;color:${c.mood >= 0 ? GOOD : BAD};font-size:11px`;

    el.append(label, stat);
    el.addEventListener("pointerenter", () => {
      el.style.background = CARD_BG_HOVER;
      el.style.borderColor = CARD_BORDER_HOVER;
    });
    el.addEventListener("pointerleave", () => {
      el.style.background = CARD_BG;
      el.style.borderColor = CARD_BORDER;
    });
    // The user's rule: clicking a MINIMIZED card maximizes them all. It is a TOGGLE (F5) —
    // clicking any card while expanded collapses back — because a state with no exit is a trap,
    // and the card is already the obvious hit target so no extra chrome is needed.
    el.addEventListener("click", (ev) => {
      ev.stopPropagation(); // never let a card click fall through to a world select
      this.setExpanded(!this.expanded);
    });
    return el;
  }

  /** Flip the expanded state, persist it, and re-lay the strip. */
  private setExpanded(next: boolean): void {
    if (this.expanded === next) return;
    this.expanded = next;
    this.saveExpanded(next);
    this.rebuild();
    this.reflow();
  }

  private storageName(): string | null {
    return this.host.storageKey ? `${this.host.storageKey}.conditionsExpanded` : null;
  }

  private loadExpanded(): boolean {
    const key = this.storageName();
    if (!key) return false;
    // Guarded like the panel's own persistence — a private-browsing context throws here, and
    // "forgets the preference" must never mean "no cards".
    try { return localStorage.getItem(key) === "1"; } catch { return false; }
  }

  private saveExpanded(v: boolean): void {
    const key = this.storageName();
    if (!key) return;
    try { localStorage.setItem(key, v ? "1" : "0"); } catch { /* no persistence available */ }
  }

  /** Re-anchor to the panel's bottom-left, clamp at the SCREEN edge, and mirror the panel's
   *  visibility. Cheap enough to run on every drag frame — one rect read, a few style writes,
   *  and the natural width computed arithmetically rather than measured (no layout thrash). */
  private reflow(): void {
    if (!this.cards.length || !this.host.visible()) {
      this.el.style.display = "none";
      return;
    }
    const r = this.host.rect();
    this.el.style.display = "flex";
    this.el.style.top = `${r.bottom - PAD_BOTTOM - CARD_H}px`;
    // One above the panel it belongs to, so it draws over the panel's own bottom edge but does
    // not leapfrog whatever the user focuses next.
    this.el.style.zIndex = String(this.host.zIndex() + 1);

    // The clamp is the VIEWPORT, not the panel (B1 #4 with #1 as the inner fallback). Two
    // distinct cases, and conflating them is what makes a right-anchored panel feel broken:
    const natural = this.naturalWidth();
    const room = window.innerWidth - EDGE_MARGIN * 2;
    if (natural <= room) {
      // 1. The strip FITS on screen but its natural origin would push it off the right — e.g.
      //    the panel is snapped to the right edge. Slide the origin left instead of scrolling:
      //    every card stays visible and the strip still hugs the panel's bottom.
      //    intent-queue-ui: the origin clears the INTENT strip's column (user call,
      //    2026-08-07 — the cards must not overlap the intentions).
      const wanted = r.left + PAD_LEFT + INTENT_STRIP_W;
      const maxLeft = window.innerWidth - EDGE_MARGIN - natural;
      this.el.style.left = `${Math.max(EDGE_MARGIN, Math.min(wanted, maxLeft))}px`;
      this.el.style.width = "";
      this.el.style.overflowX = "";
      // Fits ⇒ stay fully pass-through: only the cards take pointer events, so the gaps and
      // the strip's tail never swallow a click meant for the world.
      this.el.style.pointerEvents = "none";
    } else {
      // 2. The strip is wider than the SCREEN — no placement helps, so it becomes a scroller
      //    (B1 method #1, applied at the screen edge instead of the panel edge). It has to take
      //    pointer events to be scrollable at all; acceptable here because at this width it
      //    already spans the viewport, so the world it covers is a 42px band at the bottom.
      this.el.style.left = `${EDGE_MARGIN}px`;
      this.el.style.width = `${room}px`;
      this.el.style.overflowX = "auto";
      this.el.style.pointerEvents = "auto";
    }
  }

  /** The width the strip WANTS, from the card counts — no DOM measurement, so this is safe to
   *  call inside `reflow` without forcing a synchronous layout on every drag frame. */
  private naturalWidth(): number {
    const n = this.cards.length;
    if (n === 0) return 0;
    const big = this.expanded ? n : Math.min(n, MAXIMIZED);
    return big * CARD_W + (n - big) * CARD_W_MIN + (n - 1) * CARD_GAP;
  }
}

/** `+0.20` / `−0.40` — the minus is U+2212, matching the panel's text rows. */
function signed(mood: number): string {
  return `${mood >= 0 ? "+" : "−"}${Math.abs(mood).toFixed(2)}`;
}

function sameCards(a: ConditionCard[], b: ConditionCard[]): boolean {
  if (a.length !== b.length) return false;
  return a.every((c, i) =>
    c.label === b[i].label && c.mood === b[i].mood && c.remaining === b[i].remaining);
}

/** `n` plausible conditions for a layout drill — descending priority, alternating sign, and a
 *  timer on every third so the minimized/maximized forms both get exercised. */
function synthetic(n: number): ConditionCard[] {
  const names = ["Dehydrated", "Thirsty", "Quenched", "Exhausted", "Rested", "Hungry",
                 "Well Fed", "Cold", "Warm", "Sore", "Content", "Restless"];
  return Array.from({ length: n }, (_, i) => ({
    label: names[i % names.length],
    mood: (i % 2 === 0 ? -1 : 1) * (0.4 - i * 0.03),
    remaining: i % 3 === 0 ? 1200 - i * 7 : 0,
    priority: (n - i) * 10,
  }));
}
