//! The selection model (ui-select P0, D1) — ONE holder of "what is selected", consumed by the
//! outline pass, the details panel, the title bar, and the right-click move order. Selection is
//! a MODEL, not a click handler: single-click calls {@link replace} with one item today, and the
//! LATER drag-box pass calls the same APIs with many — consumers only ever read the model and
//! react to its change event, so multi-select drops in without reshaping them.
//!
//! Three selectable kinds, one model: a PAWN (a mover, keyed by its stable ENTITY reference —
//! warm prim ids churn on despawn/respawn), a THING (a cold standing prim id), and a TILE
//! (ground — the title bar shows its world coordinates). A tile selection is a selection like
//! any other, replaced by the next click.

/** One selected item. */
export type Selection =
  | { kind: "pawn"; entity: number }
  | { kind: "thing"; primId: number }
  | { kind: "tile"; x: number; y: number };

/** Stable identity key — the Set semantics (toggle/dedupe) hang off this. */
export const selectionKey = (s: Selection): string =>
  s.kind === "pawn" ? `p:${s.entity}` : s.kind === "thing" ? `t:${s.primId}` : `g:${s.x},${s.y}`;

export class SelectionModel {
  private items = new Map<string, Selection>();
  /** The PRIMARY selection — the one single-item consumers (details panel, move order)
   *  read. Under multi-select it stays the first/most-recent anchor item. */
  private primaryKey: string | null = null;
  private listeners = new Set<() => void>();

  /** Subscribe to changes; returns the unsubscribe. Consumers react to THIS, never to input. */
  subscribe(fn: () => void): () => void {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }

  private emit(): void {
    for (const fn of this.listeners) fn();
  }

  get all(): Selection[] {
    return [...this.items.values()];
  }

  get primary(): Selection | null {
    return this.primaryKey !== null ? this.items.get(this.primaryKey) ?? null : null;
  }

  get size(): number {
    return this.items.size;
  }

  has(s: Selection): boolean {
    return this.items.has(selectionKey(s));
  }

  /** Drop everything and select these (the single-click path; the drag-box will pass many). */
  replace(sel: Selection[] | Selection | null): void {
    this.items.clear();
    this.primaryKey = null;
    const list = sel === null ? [] : Array.isArray(sel) ? sel : [sel];
    for (const s of list) {
      const k = selectionKey(s);
      this.items.set(k, s);
      this.primaryKey ??= k;
    }
    this.emit();
  }

  /** Add without dropping (shift-click / drag-box additive mode). */
  add(sel: Selection[] | Selection): void {
    const list = Array.isArray(sel) ? sel : [sel];
    for (const s of list) {
      const k = selectionKey(s);
      this.items.set(k, s);
      this.primaryKey ??= k;
    }
    this.emit();
  }

  /** Toggle one item (ctrl-click). */
  toggle(s: Selection): void {
    const k = selectionKey(s);
    if (this.items.has(k)) {
      this.items.delete(k);
      if (this.primaryKey === k) this.primaryKey = this.items.keys().next().value ?? null;
    } else {
      this.items.set(k, s);
      this.primaryKey ??= k;
    }
    this.emit();
  }

  clear(): void {
    if (this.items.size === 0) return;
    this.items.clear();
    this.primaryKey = null;
    this.emit();
  }
}
