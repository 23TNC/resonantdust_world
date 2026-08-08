//! The client's inventory mirror (inventory F7) — `PawnInventory` frames folded into a
//! per-pawn slot map. `item = 0` is the slot-emptied relay (drop/removal); a pawn's
//! `StateGone` does NOT clear its map here — the panel keys off the SELECTION, and a
//! despawned pawn's stale rows are unreachable garbage collected with the store.
//! FILLED comes from these ROWS; FREE is the `inventory` NEED — never conflate (I2).

import type { WasmClient, PawnInventory } from "../../client/WasmClient";

export class InventoryStore {
  /** entity → (slot → item definition_reference). */
  private readonly slots = new Map<number, Map<number, number>>();
  private readonly subs = new Set<() => void>();
  private readonly unsub: () => void;

  constructor(client: WasmClient) {
    this.unsub = client.onPawnInventory((f: PawnInventory) => {
      let m = this.slots.get(f.entityReference);
      if (!m) {
        m = new Map();
        this.slots.set(f.entityReference, m);
      }
      if (f.item === 0) m.delete(f.slot);
      else m.set(f.slot, f.item);
      for (const cb of this.subs) cb();
    });
  }

  /** The pawn's filled slots — a LIVE map reference; treat as read-only. */
  slotsOf(entity: number): ReadonlyMap<number, number> {
    return this.slots.get(entity) ?? new Map();
  }

  subscribe(cb: () => void): () => void {
    this.subs.add(cb);
    return () => this.subs.delete(cb);
  }

  destroy(): void {
    this.unsub();
    this.subs.clear();
  }
}
