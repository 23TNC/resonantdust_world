# Todo — cold-rework

**P1–P3 done + live-verified; P4 foundation (mint + `set_tile`) done + live-verified** — see
[`completed.md`](completed.md). Decisions in [`forks.md`](forks.md).

---

## P4 · Mutation — the remaining event-driven path

The mint + a direct `set_tile` mutation primitive are live (a cold cell mutates through the overlay,
server-minted). What's left to make it the *real* pipeline path:

- **Event-driven `UNPACK`** — route a mutate through edge → orchestrator → worker (the worker calls the
  mint), instead of the direct `set_tile` reducer. Deterministic-from-event id (replay-safe) instead of
  the bare counter.
- **`thing`-side mutation** — the same mint + override on the `thing` shard (scatter), mirroring
  `set_tile`.
- **GC fold (`PACK`)** — a `master`/GC pass that folds settled `state_log` back into the baseline
  (`cold_tile`/`cold_thing`), drops the `state` row, and tombstones the `state_log`.
- **Baseline suppression** — the client hiding a baseline cell a `state` override *removes* (currently
  the override draws on top; a removal leaves the baseline showing).
- **Core two-phase** — core issues `UNPACK`@N=0 (pre-empted), learns the id by watching the position in
  `state`, then issues the op gated on the row.
- **`server_reference` master-assigned** (F2) — replace the `tile`/`thing` const stopgap once a family
  has more than one shard.

**Done when:** a scripted mutation flows edge → orchestrator → worker → mint → compose → promote →
render, and GC folds it back into the baseline with the `state` row dropped — all through events.
