# sim-self-heal — make the sim processes survive shard outages

_Work stream, opened 2026-07-27. Components: `server/master`, `server/worker`,
`server/orchestrator`, `server/edge` (+ a new shared `server/uplink` crate)._

## What

The three sim binaries `.expect()`/panic on **every** upstream SpacetimeDB connection at standup
and on the 5-second subscription-applied wait, and they never rebuild a connection that dies
mid-run — one briefly-unavailable shard kills the whole process, and a mid-run disconnect leaves a
zombie loop calling reducers on a dead connection:

- `server/master/src/main.rs` — 5 `.expect("build … connection")` + `rx_i.recv_timeout(…).expect`
  (the pawn shard added one everywhere, first-pawns 2026-07-28).
- `server/worker/src/main.rs` — 6 connections + a recv/panic ladder over all six subscriptions.
- `server/orchestrator/src/main.rs` — 5 connections + 2 recv `.expect`s.
- `client/npc` — the same disease on the CLIENT side: the engine never reconnects, and the brain
  keeps issuing commands into a dead socket forever (observed live, below).

The gateway already solved this exact problem: `server/gateway/src/directory.rs` shares an
`alive: AtomicBool` the SDK clears on disconnect/connect-error, and `conn()` rebuilds a dead
connection on next use, gated on subscribe-and-wait (the SDK does not auto-reconnect).

Adjacent hazard, same theme (a transient fault becoming permanent): `server/edge`'s asset-serving
paths (`content.rs`, `connections.rs`, `tex_manifest.rs`, …) call `.read().unwrap()` /
`.write().unwrap()` on `RwLock`s — one panic while a lock is held poisons it and every subsequent
content/texture request panics until the edge is restarted.

## Live evidence (first-pawns, 2026-07-28 — every failure mode observed in one session)

- **The void-writing worker** (first-pawns [I1](../2026-07-28-first-pawns/issues.md)):
  `rd redeploy` republished the `pawn` module while the trio was connected; the worker then
  logged "composed component" CLEANLY while writing into the wiped DB's dead session — nothing
  errored, the data simply never landed. Restarting the trio fixed it. An uplink that clears
  `alive` on the republish disconnect is exactly the cure.
- **The zombied npc**: an edge redeploy killed the npc's WS session; the brain kept logging
  "trip issued" into the dead socket indefinitely (deadline re-issues going nowhere). No error,
  no exit, no progress — the perfect zombie.
- **A mere BUILD triggered a data-wiping republish**: the module input-hash includes the
  in-tree `target/` dir, so building `pawn` re-triggered "deploy module pawn — publish, wipe
  data" on the next redeploy. Dev-redeploy hardening is P5 below.
- **`bin/sim run` happily runs a stale binary** after a failed build — twice in one session the
  "fix" that ran was the previous binary (compounded by the docker mtime miss).

## Design stance

- **One shared helper, not three copies.** A new `server/uplink` crate (`resonantdust-uplink`)
  generalises the gateway pattern: `Uplink<C>` = a build closure + `alive` flag + an optional
  subscribe closure; `get()` returns the live connection or rebuilds it (subscribe-and-wait,
  capped backoff). Closure-based because each module's generated `DbConnection` is a distinct
  type with no shared builder trait ([F3](forks.md#f3)).
- **Startup = mid-run recovery.** No special standup phase: build best-effort, then every pass
  acquires lazily and skips only the work that needs a dead uplink ([F2](forks.md#f2)). A sim
  process started before SpacetimeDB is up simply begins working when it arrives.
- **Correctness rides existing idempotence** — no new protocol. The worker's writes are absolute
  + replay-safe, the orchestrator is stateless (assign/claim idempotent, `assigned` is only a
  dedup), and the master's tic authority is the durable `index.master_clock` row. Re-processing
  after a re-subscribe is therefore safe by design; this stream only has to avoid *reading an
  un-applied cache* (hence `get()` gates on subscription applied).
- **Edge locks recover from poison** rather than propagate it (`unwrap_or_else(|e| e.into_inner())`).

Out of scope (noted follow-up): migrating the gateway's own `directory.rs` onto `uplink` — it
already self-heals; unifying it is cleanup, not resilience.

## Acceptance (stream-level)

Live chaos, not just green builds: restart the SpacetimeDB container mid-run → master, worker,
and orchestrator all reconnect **without process restarts** and wolves resume moving in the
browser; start the sim binaries with SpacetimeDB down → they retry and come alive when it does.
