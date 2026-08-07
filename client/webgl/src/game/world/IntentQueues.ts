//! The per-pawn intent-queue mirror (intent-queue-ui F1) — the details panel's strip
//! reads this. DISPLAY TRUTH ONLY: fed by promoted `QUEUE_STATE` snapshots, replaced
//! whole per fan, self-correcting on the next mutation; never authority (the worker's
//! ephemeral map is — lumberjack F1). Snapshots are deduped by `eventTic` serial order
//! so a replayed older fan (a re-subscribe) cannot roll the strip backwards.

import type { WasmClient, QueueState } from "../../client/WasmClient";

/** One strip circle (ACTIONS.md § palette `QUEUE_STATE`, decoded from the stride-4
 *  entry words). Phase: 0 pending / 1 walking / 2 executing. */
export interface QueueEntry {
  /** The worker-minted display identity — what a cancel click sends (F4). */
  entryId: number;
  /** The interaction's gameplay `definition_reference` — keys the TOML visuals. */
  interactionRef: number;
  phase: number;
  /** Executing only (phase 2): the schedule and fire tics of the completion. */
  startedTic: number;
  fireTic: number;
}

/** Serial u16 "a after b" — the tic ring wraps (mirrors codec `tic_after`). */
function ticAfter(a: number, b: number): boolean {
  return ((a - b) & 0xffff) !== 0 && ((a - b) & 0xffff) < 0x8000;
}

export class IntentQueues {
  /** entity → its latest snapshot. Entry 0 = the ACTIVE event (the strip's bottom). */
  private readonly queues = new Map<number, { eventTic: number; entries: QueueEntry[] }>();
  private readonly subs = new Set<() => void>();
  private readonly unsub: () => void;

  constructor(client: WasmClient) {
    this.unsub = client.onQueueState((q) => this.onSnapshot(q));
  }

  /** The pawn's current entries (entry 0 = active), or `[]`. */
  entriesOf(entity: number): QueueEntry[] {
    return this.queues.get(entity)?.entries ?? [];
  }

  /** Subscribe to any-change; returns an unsubscribe. */
  subscribe(cb: () => void): () => void {
    this.subs.add(cb);
    return () => this.subs.delete(cb);
  }

  dispose(): void {
    this.unsub();
    this.queues.clear();
  }

  private onSnapshot(q: QueueState): void {
    const prev = this.queues.get(q.entityReference);
    // Serial-ordered dedup: a REPLAYED older fan must not roll the strip back. Equal
    // tics accept (two mutations can fan in one composing tic — last wins).
    if (prev && ticAfter(prev.eventTic, q.eventTic)) return;
    const entries: QueueEntry[] = [];
    for (let i = 0; i + 3 < q.entries.length; i += 4) {
      const timing = q.entries[i + 3];
      entries.push({
        entryId: q.entries[i],
        interactionRef: q.entries[i + 1],
        phase: q.entries[i + 2],
        startedTic: (timing >>> 16) & 0xffff,
        fireTic: timing & 0xffff,
      });
    }
    if (entries.length === 0) this.queues.delete(q.entityReference);
    else this.queues.set(q.entityReference, { eventTic: q.eventTic, entries });
    for (const cb of this.subs) cb();
  }
}
