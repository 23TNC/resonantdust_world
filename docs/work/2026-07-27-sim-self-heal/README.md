# sim-self-heal — make the sim processes survive shard outages

_Work stream, opened 2026-07-27. Components: `server/master`, `server/worker`,
`server/orchestrator`, `server/edge` (+ a new shared `server/uplink` crate)._

## What

The three sim binaries `.expect()`/panic on **every** upstream SpacetimeDB connection at standup
and on the 5-second subscription-applied wait, and they never rebuild a connection that dies
mid-run — one briefly-unavailable shard kills the whole process, and a mid-run disconnect leaves a
zombie loop calling reducers on a dead connection:

- `server/master/src/main.rs` — 4 `.expect("build … connection")` + `rx_i.recv_timeout(…).expect`.
- `server/worker/src/main.rs` — 5 connections + a recv/panic ladder over all five subscriptions.
- `server/orchestrator/src/main.rs` — 4 connections + 2 recv `.expect`s.

The gateway already solved this exact problem: `server/gateway/src/directory.rs` shares an
`alive: AtomicBool` the SDK clears on disconnect/connect-error, and `conn()` rebuilds a dead
connection on next use, gated on subscribe-and-wait (the SDK does not auto-reconnect).

Adjacent hazard, same theme (a transient fault becoming permanent): `server/edge`'s asset-serving
paths (`content.rs`, `connections.rs`, `tex_manifest.rs`, …) call `.read().unwrap()` /
`.write().unwrap()` on `RwLock`s — one panic while a lock is held poisons it and every subsequent
content/texture request panics until the edge is restarted.

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
