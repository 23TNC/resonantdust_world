# Todo — spacetime-again

_**Empty — W1-W9 are all done.** The rebuild is complete and live end-to-end; see
[`completed.md`](completed.md). New work starts a fresh item here._

The deferred, non-blocking follow-ups noted along the way (not part of this stream):
- `CREATE`'s minted-id spawn claim (orchestrator + worker + a data_shard spawn-log).
- `MOVE_TO` multi-tile stepping + self-requeue (worker) — today it arrives in one step.
- A per-pawn `definition_reference` (a definition verb or `CREATE`) so pawns render their real
  sprite — today placed pawns carry def 0 and draw the fallback thing.
- Multi-data-shard `claim`/`write` routing (one shard today).
- Orchestrator liveness/takeover; live SDK reconnect on a shard redeploy (restart the clients today).
- The terrain (cold-zone) subscription path — `index.rs`/`worldgen.rs` return with it; `Event::ColdObjects`.
- A server-side pause/freeze verb (the `/pause` command is stubbed).
- Client-sync tic relay (the edge could push `master_clock` for interpolation).
