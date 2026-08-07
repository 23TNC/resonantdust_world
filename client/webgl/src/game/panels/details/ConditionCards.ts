//! The details panel's CONDITION CARDS — a horizontal strip along the bottom of the panel,
//! showing what a selected pawn is currently feeling / suffering (`2026-08-04-conditions` P4,
//! rebuilt as EMOTION PIE SQUARES by `2026-08-07-emotions` F5/F6).
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
//! `priority` desc, Σ emotion magnitude desc, `condition_id` asc (conditions F3, emotions F4) —
//! this file renders the order it is given. A sort in TS would be a second implementation of a
//! corpus rule.
//!
//! **The card is the chart** (emotions F5): a fixed-size SQUARE whose background is a CSS
//! `conic-gradient` pie — one slice per emotion modifier, proportional to magnitude, slices in
//! EMOTION-INDEX order from 12 o'clock. One modifier = solid; none = solid `fine` gray. NO card
//! text — the everything-tooltip (F6) is the whole read surface.

/** One emotion slice, colors/labels resolved by the provider (the corpus owns look). */
export interface EmotionSlice {
  /** The emotion's u4 declaration index — the slice ORDER key (F5). */
  index: number;
  /** 1..15 authored; 0 only for the synthetic `Fine +0` of a modifier-free condition. */
  magnitude: number;
  label: string;
  /** `0xRRGGBB`. */
  color: number;
}

/** One condition as the panel receives it — the shared eval's row, labelled + resolved. */
export interface ConditionCard {
  /** The u32 gameplay definition_reference — the identity `sameCards` compares. */
  id: number;
  label: string;
  /** Σ emotion magnitude — the sort's tie-break (emotions F4), shown nowhere; carried so a
   *  changed corpus re-renders via `sameCards`. */
  magnitudeSum: number;
  /** Tics left on a TIMED grant; `0` = DERIVED (alive exactly while its band holds). */
  remaining: number;
  /** The authored sort key, carried for the tooltip. Never re-sorted here. */
  priority: number;
  /** The pie slices (F5). Empty never arrives — the provider synthesizes `Fine +0` (gray). */
  emotions: EmotionSlice[];
  /** Pre-formatted need-modifier lines for the tooltip (F6) — `"thirst rate ×0.5"`. */
  needLines: string[];
}

// ── layout constants ────────────────────────────────────────────────────────────────
// SQUARES now (emotions F5). Sized so the strip's height matches the old card row and
// eight maximized squares fit the panel's default width (~355 px): 8+40 + 8×(34+6) ≈ 368.

/** Maximized card side, px — a fixed-size SQUARE (F5). */
export const CARD_W = 34;
/** Card height = width (the square law). Kept as its own name for the reflow math. */
export const CARD_H = CARD_W;
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
/** A minimized card's side as a FRACTION of a maximized one — still a square. */
export const MINIMIZED_FRACTION = 0.55;
/** Minimized card side, px (derived; kept as a constant so tests and CSS agree). */
export const CARD_W_MIN = Math.round(CARD_W * MINIMIZED_FRACTION);
/** Gap kept between the strip and the viewport edges when it has to be clamped, px. */
export const EDGE_MARGIN = 8;

const CARD_BORDER = "#3a3a4a";
const CARD_BORDER_HOVER = "#b8bfd0";
/** The modifier-free fallback if the provider ever fails to synthesize `Fine +0` (F5). */
const FINE_GRAY = 0x9aa4b0;

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
  /** The everything-tooltip (emotions F6) — ONE shared fixed-position div, the IntentStrip
   *  pattern: cursor-following, pointer-transparent, torn down with the strip. */
  private readonly tooltip = document.createElement("div");
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
    this.tooltip.style.cssText =
      "position:fixed;display:none;z-index:60001;pointer-events:none;" +
      "background:#1c1f24;color:#d7dde5;border:1px solid #444;border-radius:4px;" +
      "padding:4px 8px;font:11px/1.6 monospace;white-space:pre;";
    document.body.appendChild(this.tooltip);
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
    this.tooltip.remove();
    this.el.remove();
  }

  // ── internals ──────────────────────────────────────────────────────────────────────

  private rebuild(): void {
    this.tooltip.style.display = "none"; // its card may be gone
    this.el.replaceChildren();
    this.cards.forEach((c, i) => this.el.appendChild(this.buildCard(c, i)));
  }

  /** One card: a pie SQUARE (F5) — no text; the tooltip carries everything (F6). A minimized
   *  card keeps the same node and only shrinks, so the strip still reads as "there are more of
   *  these" rather than going blank. */
  private buildCard(c: ConditionCard, index: number): HTMLElement {
    const minimized = !this.expanded && index >= MAXIMIZED;
    const side = minimized ? CARD_W_MIN : CARD_W;
    const el = document.createElement("div");
    el.dataset.rdCondition = c.label;
    el.dataset.rdMinimized = minimized ? "1" : "0";
    el.style.cssText = [
      `width:${side}px`,
      `height:${side}px`,
      "flex:0 0 auto",
      "box-sizing:border-box",
      `background:${pieBackground(c.emotions)}`,
      `border:1px solid ${CARD_BORDER}`,
      "border-radius:3px",
      "overflow:hidden",
      "pointer-events:auto",
      "cursor:pointer",
      "transition:border-color 90ms linear",
    ].join(";");

    el.addEventListener("mouseenter", (e) => {
      el.style.borderColor = CARD_BORDER_HOVER;
      this.tooltip.textContent = tooltipText(c);
      this.tooltip.style.display = "block";
      this.tooltip.style.left = `${e.clientX + 12}px`;
      this.tooltip.style.top = `${e.clientY - 6}px`;
    });
    el.addEventListener("mousemove", (e) => {
      this.tooltip.style.left = `${e.clientX + 12}px`;
      this.tooltip.style.top = `${e.clientY - 6}px`;
    });
    el.addEventListener("mouseleave", () => {
      el.style.borderColor = CARD_BORDER;
      this.tooltip.style.display = "none";
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
      //    already spans the viewport, so the world it covers is a thin band at the bottom.
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

const hex = (c: number): string => `#${(c >>> 0).toString(16).padStart(6, "0")}`;

/** The pie (emotions F5): slices proportional to magnitude, EMOTION-INDEX order from
 *  12 o'clock (conic-gradient's 0deg, clockwise). One slice — or a zero total, the synthetic
 *  `Fine +0` — renders solid. */
export function pieBackground(slices: EmotionSlice[]): string {
  const ordered = [...slices].sort((a, b) => a.index - b.index);
  const total = ordered.reduce((s, m) => s + m.magnitude, 0);
  if (total === 0) return hex(ordered[0]?.color ?? FINE_GRAY);
  if (ordered.length === 1) return hex(ordered[0].color);
  let at = 0;
  const stops = ordered.map((m) => {
    const from = (at / total) * 360;
    at += m.magnitude;
    const to = (at / total) * 360;
    return `${hex(m.color)} ${from.toFixed(2)}deg ${to.toFixed(2)}deg`;
  });
  return `conic-gradient(${stops.join(", ")})`;
}

/** The everything-tooltip's text (emotions F6): the label, one `+N <Emotion>` line per
 *  modifier (`+` always; +0 only as the synthetic `Fine +0`), the priority, the remaining
 *  tics of a TIMED grant, and the need-modifier lines. */
export function tooltipText(c: ConditionCard): string {
  const lines = [c.label];
  for (const m of [...c.emotions].sort((a, b) => a.index - b.index)) {
    lines.push(`+${m.magnitude} ${m.label}`);
  }
  lines.push(`priority ${c.priority}`);
  if (c.remaining > 0) lines.push(`${c.remaining}t remaining`);
  lines.push(...c.needLines);
  return lines.join("\n");
}

function sameCards(a: ConditionCard[], b: ConditionCard[]): boolean {
  if (a.length !== b.length) return false;
  return a.every((c, i) =>
    c.id === b[i].id && c.magnitudeSum === b[i].magnitudeSum && c.remaining === b[i].remaining);
}

/** `n` plausible conditions for a layout drill — descending priority, varied pies, and a
 *  timer on every third so the minimized/maximized forms both get exercised. */
function synthetic(n: number): ConditionCard[] {
  const names = ["Dehydrated", "Thirsty", "Quenched", "Exhausted", "Rested", "Hungry",
                 "Well Fed", "Cold", "Warm", "Sore", "Content", "Restless"];
  const palette = [0xe8a33a, 0x3a6ee8, 0xd8342c, 0x8a4fd8, 0x3ab84f, 0x8a8f3c];
  return Array.from({ length: n }, (_, i) => {
    const emotions: EmotionSlice[] = Array.from({ length: (i % 3) + 1 }, (_, j) => ({
      index: (i + j) % 16,
      magnitude: 1 + ((i + j) % 5),
      label: `Emotion${(i + j) % 16}`,
      color: palette[(i + j) % palette.length],
    }));
    return {
      id: 0x8003_0000 + i,
      label: names[i % names.length],
      magnitudeSum: emotions.reduce((s, m) => s + m.magnitude, 0),
      remaining: i % 3 === 0 ? 1200 - i * 7 : 0,
      priority: (n - i) * 10,
      emotions,
      needLines: i % 2 === 0 ? ["thirst rate ×0.5"] : [],
    };
  });
}
