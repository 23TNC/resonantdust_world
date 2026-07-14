# Completion log — closing the gap to the full design

Each entry: a piece of the design that was deferred in the initial hot-path slice, now
**implemented to match the design**, with how it was verified and the commit. As pieces land
here they leave [remaining.md](remaining.md). Ordered correctness-first.

The initial slice (S0–S7 hot path) is in the [status table](README.md); this log tracks the
mechanisms that make the code match [`../spacetime-tables/`](../spacetime-tables/) in full.

## Planned order

1. **Deterministic composition + tic-gated execution** — resolve only a *sealed* tic
   (`master ≥ event_tic`), and fold *all* a target's events in `event_reference` order (not
   last-writer-wins).
2. **Cross-shard convergent writes** — idempotent per `(source_shard, event_reference)`; row
   completes when all target shards applied.
3. **Full control flow** — the forward `SKIP` branch (`? then : else`) + multi-action vectors.
4. **Actor-reading verbs** — `DAMAGE` + the operand-read defer (`read_rule`, `≤ T−1`).
5. **`RUNNING`-row recovery** — re-claim on lease expiry.
6. **Reconciliation tail** — `region_zone` keying (#4), `hot_reference` re-key (#5),
   geographic `server_reference` (#10), `PACK` trigger.
7. **Integration** — worker/master run-compose, pixijs client.

---

## Completed

### 1. Deterministic composition + tic-gated execution ✅

**What.** The worker's `execute` now (a) resolves a row only when `master_tic ≥ event_tic` — the
tic is *sealed*, so no further event can target it (they'd land at `master+3`); and (b) computes
each target's value as `base@(tic−1)` folded over **all** applicable events targeting that
`(entity, tic)`, in **`event_reference` order** — not arrival order, not last-writer-wins. An
event is "applicable" if it has no `await` gate or its await is complete. Any row that triggers
the resolve computes the same fold, so it's idempotent and order-free (matches
[lifecycle.md](../spacetime-tables/lifecycle.md) §composition + §strict staging).

**How verified (live).** Seeded entity 100 at loc 17 (tic 0); appended **two moves at the same
tic** (→34, then →51). While `master=0 < event_tic=3` the entity **stayed at loc 17** (gated, not
resolved). After bumping master to 3 (tic sealed), the two moves **folded by `event_reference`**
(34 then 51) → **loc 51** at tic 3. Both the gate and the deterministic fold confirmed.

**Note.** This realizes the designed `+3` latency (a row now waits ~gap tics to seal before
resolving), replacing the earlier resolve-immediately simplification.
