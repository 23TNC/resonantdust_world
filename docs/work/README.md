# Work — index

_The flowing execution state. Each `work/<w>/` turns component `plan`s into executable items and
spans components by nature. Convention: [`../CONVENTIONS.md`](../CONVENTIONS.md). Last updated:
2026-08-10._

**Status** = the stream's lifecycle, not the session's focus: `open` (in progress) · `done`
(delivered; may have a small hand-off) · `closed` (abandoned/refuted) · `blocked` (needs input).

| Stream | Status | What it is |
|---|---|---|
| [2026-08-10-teardown-housekeeping](2026-08-10-teardown-housekeeping/README.md) | open | **Close out what the 0.2.3 deletion left dangling.** Removing 4554 tracked files and ~42G of ignored payload left a tail: copies that are now the *only* copy of something, tooling pointing at deleted paths, ~24G of docker state nothing will use again, and ~60 memories describing code that is gone. Framing matters — the disk is at **10% (863G free)**, so this is about ambiguity and risk, not space. Two things justify it: one genuine irreversible-loss risk (`bin/keys/r2.env` + `anthropic.env` were gitignored on purpose and now exist *only* in `../resonantdust_world_old/`, the directory everyone thinks of as the throwaway backup — plus four local-only branches and 8 dirty files in `/home/wolf/resonantdust`), and the fact that nobody can currently answer "which copy is authoritative?" across two full checkouts, a registered worktree, five pre-monorepo split repos, and a 14G prior iteration all answering to "resonantdust". Structure is **salvage → retire → reclaim**, with P0 a hard gate on P2/P3 — the teardown's own duplicate-copy check was sound for tracked files and inverts the moment the backup is the target (see `issues.md` H-I1). Also owns porting `docs_check.py` (the login stream's F7), since a restored `CONVENTIONS.md` cites a checker that was deleted. One blocker: keep-or-tear-down on the Lightsail instance, the R2 bucket, and `gateway.resonantdust.com` — spending and DNS decisions, and the R2 call interacts with retiring the local texture corpus. |
| [2026-08-10-login-without-spacetime](2026-08-10-login-without-spacetime/README.md) | open | **Remove SpacetimeDB entirely; stand up login in its absence.** ST has been a sustained pain point across 0.2.x — a pinned image whose `latest` never initializes, in-container builds that miss changed files, publishes that wipe data because there are no migrations, and an image/crate/bindings triple that must move in lockstep. The load-bearing finding: 0.2.3's login was already **trust-on-first-use by name** (`{"t":"login","name":"Alice"}` → `claim_or_login` → `LoginOk{player_id}`), so ST was never providing user auth — it provided the *storage* for the player registry and its own DB-ownership JWTs. The replacement is correspondingly small: the registry becomes an append-only JSONL ledger behind a `PlayerStore` trait (low-write, `cat`-able, resettable with `rm`), and a stateless HMAC session token takes over what the JWT was quietly doing — letting a service that isn't the gateway trust a connection with no shared session table. `server/gateway` owns `POST /login` + `POST /verify`; `client/core` owns the round-trip as a Rust lib with `native` (reqwest) and `web` (gloo-net) hosts over one pure `parse_login`. Ids: `Developer` at reserved `512`, real players from `1024`. Also folds the Rust tree into one cargo workspace — no Docker for our own code, which is where the stale-mtime misses and root-owned build output came from. **Does not fill two holes ST leaves: the world-state store and the real-time push fan.** Login needs neither; both are larger design problems than this stream. |

## Note on the authority tree

0.3.0 restored [`CONVENTIONS.md`](../CONVENTIONS.md) but not the rest of the 0.2.3 docs system.
`docs/components/` is being rebuilt **lazily**, one component at a time as it gets built (see the
stream's `forks.md` F8), and `bin/rd docs-check` does not exist yet — the convention is followed by
hand for now, with the checker ported from `0.2.3:bin/lib/docs_check.py` as its own stream once
there is enough tree to check (F7). Prior art for anything deleted: `git show 0.2.3:<path>`.
