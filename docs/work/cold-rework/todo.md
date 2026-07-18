# Todo — cold-rework

**P1–P3 done + live-verified; P4 foundation (mint + `set_tile`) done + live-verified** — see
[`completed.md`](completed.md). Decisions in [`forks.md`](forks.md).

---

## P4 · Mutation — the remaining event-driven path

The tile lifecycle is live end-to-end: **mint (`set_tile`, biome-preserving) → `state` override →
`fold` (PACK) back into the baseline**, all browser-verified. What's left:

- **Event-driven `UNPACK`** — route a mutate through edge → orchestrator → worker (the worker calls the
  mint), instead of the direct `set_tile` reducer. Deterministic-from-event id (replay-safe) instead of
  the bare counter. **Touches the delicate live wolf pipeline — the CREATE/mint spawn-id claim is
  currently *deferred* in the worker/orchestrator; this is the biggest remaining piece.**
- **`thing`-side mutation** — mirror `set_tile`/`fold` on the `thing` shard (scatter).
- **Baseline suppression** — the client hiding a baseline cell a `state` override *removes* (currently
  the override draws on top; a removal leaves the baseline showing).
- **Core two-phase** — core issues `UNPACK`@N=0 (pre-empted), learns the id by watching the position in
  `state`, then issues the op gated on the row.
- **`server_reference` master-assigned** (F2) — replace the `tile`/`thing` const stopgap once a family
  has more than one shard.

Done items (mint + `fold` + biome-preservation) are in [`completed.md`](completed.md).

**Done when:** a scripted mutation flows edge → orchestrator → worker → mint → compose → promote →
render, and GC folds it back into the baseline with the `state` row dropped — all through events.
