# Todo — cold-rework

**P1–P3 done + live-verified; P4 foundation (mint + `set_tile`) done + live-verified** — see
[`completed.md`](completed.md). Decisions in [`forks.md`](forks.md).

---

## P4 · Mutation — the remaining event-driven path

**Both cold types mutate end-to-end, live-verified** — `set_tile`/`set_thing` (biome-preserving mint →
`state` override) → `fold` (PACK back into the baseline). See [`completed.md`](completed.md). What's
left, by kind:

**Render refinement**
- **Baseline suppression** — the client hiding a baseline cell a `state` override *removes* (currently
  the override draws on top; a removal / `kind == 0` leaves the baseline showing). Needs type-specific,
  position-keyed baseline-prim tracking + suppress/restore that plays with the `fold` re-expand — real
  render state-management, best done deliberately.

**The big pipeline integration** (touches the delicate live wolf pipeline)
- **Event-driven `UNPACK`** — route a mutate through edge → orchestrator → worker (the worker calls the
  mint), instead of the direct `set_tile`/`set_thing` reducer, with a deterministic-from-event id
  (replay-safe). The **CREATE/mint spawn-id claim is currently *deferred* in the worker/orchestrator**,
  so this is genuinely new logic in the most correctness-critical code.
- **Core two-phase** — core issues `UNPACK`@N=0 (pre-empted), learns the id by watching the position in
  `state`, then issues the op. Pairs with the above.

**Stopgap promotions** (low urgency, recorded)
- `server_reference` **master-assigned** (F2) — the `tile`/`thing` const only bites at multi-shard.
- `state_log` **tombstone-not-drop** in `fold` (design keeps it briefly for a slow reader / takeover).
- `event_shard`'s hardcoded `SERVER_REFERENCE` → the master-assigned pattern.
- edge **multi-region → multi-shard connection pool** (deferred P2; single-shard works today).

**Done when:** a scripted mutation flows edge → orchestrator → worker → mint → compose → promote →
render, and GC folds it back into the baseline with the `state` row dropped — all through events.
