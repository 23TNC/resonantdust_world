# Component — `client/npc`

_Path: `client/npc`. Deploys as: the `npc` binary in container `rd-npc` (`bin/sim run npc`).
Last updated: 2026-07-28 (first-pawns P4)._

An npc is an **automated player**: it drives the same `client/core` command/event API a browser
does — login via the gateway, anchors, `queue`d action programs — with no privileged
direct-to-shard path. One process runs ONE **brain**.

## The seam

- **`src/lib.rs`** — the harness: `Bot` (login, parameterized-radius `anchor_and_wait`, event
  pump, pause tracking, `resolve_thing_def` — def ids come from the server's `/content` corpus,
  never pinned constants) + the **`Brain` trait** (`async on_start` / `on_event` / `tick`) +
  `run_brain` (the drive loop).
- **`src/brains/`** — one module per behavior. `wolves` (the default): adopt-first (a restart
  re-uses an existing minted wolf) else `CREATE` one, then endless single A→B `MOVE_TO`s — the
  WORKER chains the hops — waiting on authoritative arrival or a deadline. `wildlife`: the
  legacy 4-wolf zone-0 pack (client-minted `place`/`move`), kept as the harness regression
  fixture.
- **`src/main.rs`** — a thin dispatcher: brain by argv[1] or `NPC_BRAIN` (default `wolves`).

A future villager-brain or region-steward-brain is a new `brains/` module + a dispatch arm —
never a new harness. Multiple brains = multiple processes/containers (`rd-npc-<brain>` when
more than one runs).

## Running

```
bin/sim build npc
bin/sim run npc NPC_BRAIN=wolves NPC_HOME=100,50 NPC_RADIUS=6 MOVE_MS=1000
```

Host network is required (the gateway resolves the edge as `ws://localhost:8473`). Cargo paths
are repo-relative; every runner mounts the whole repo at `/workspace`. NOTE: an edge restart
strands the session (no reconnect — `2026-07-27-sim-self-heal` territory); restart the
container after redeploys.
