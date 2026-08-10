# login-without-spacetime

**Status:** open · **Opened:** 2026-08-10 · **Components:** `server/gateway`, `client/core`

Remove SpacetimeDB from the repo entirely, and stand up the first two pieces of 0.3.0 in its
absence: **`server/gateway`** owns login, **`client/core`** owns talking to the server. The
round-trip we're building: `client/core` asks the gateway to create-or-login a username —
typically `Developer` or `Claude` — and gets back a player id and a session token.

## Why

SpacetimeDB has been a sustained pain point across 0.2.x: a pinned image whose `latest` never
initializes, in-container builds that miss changed files, module publishes that wipe data because
there are no migrations, database names that silently reject underscores, and a wire/ABI triple
(image tag, crate version, generated bindings) that has to move in lockstep. The cost landed on
every schema change.

## What SpacetimeDB was actually doing

Worth separating, because the login replacement is far smaller than "replace our database":

| ST provided | Replaced by | Where |
|---|---|---|
| player registry storage (`players` module) | a JSONL ledger + in-memory index | this stream, P2 |
| DB-ownership JWTs (`id_ecdsa`, `spacetimedb_token`) | an HMAC session token | this stream, P3 |
| the `index` routing directory | static config on the gateway | this stream, P4 (stub) |
| world-state storage (the shards) | **nothing yet** | out of scope — see below |
| real-time subscription fan (server → client push) | **nothing yet** | out of scope — see below |

**Flagging plainly:** removing ST leaves two holes this stream does not fill — the world-state
store and the push fan. Login does not need either, which is why login is the right first cut. But
"spacetime is gone" is not the same as "we have a server"; the state store and the fan are the
next real design problems, and they are bigger than this one. Nothing here should be read as
having solved them.

## The trust model does not change

0.2.3's login was already **trust-on-first-use by name** — the edge took
`{"t":"login","name":"Alice"}` and relayed it to `players.claim_or_login`, which created the
player if the name was new and returned it if not. There was no password, and the ST identity was
never a *user* credential; it was how the CLI proved DB ownership.

So the replacement is not "add auth we lost". It is: keep TOFU-by-name, and put the registry
somewhere that isn't a SpacetimeDB module. What we *add* is a **session token**, so a service that
isn't the gateway can verify a connection without asking the gateway and without a shared session
table. That is the piece ST's JWT was quietly doing.

Real credentials (a per-player secret issued at create time) are designed for but not built —
see [`forks.md`](forks.md) F2. The wire carries a place for them.

## Design stance

- **The registry is a file, not a database.** An append-only JSONL ledger read into memory at
  boot. It is low-write (login only), small, human-readable, `cat`-able when you want to know who
  exists, and resettable with `rm`. Reaching for SQLite or sled here would re-import the class of
  problem this iteration exists to remove. It sits behind a `PlayerStore` trait so it can be
  swapped when write volume justifies it.
- **The token is stateless.** `v1.<payload>.<mac>`, HMAC'd with a gateway secret. Any service
  holding the key verifies it alone. No session table — a shared session store is how you end up
  needing a database again.
- **One cargo workspace, no Docker for the Rust side.** 0.2.3 built each crate in its own
  container with its own target dir, which is where the stale-mtime misses and root-owned build
  output came from. Native Rust needs none of it.
- **`client/core` keeps the two-layer transport split** that worked in 0.2.3: a pure
  `parse_*` half that is host-agnostic and unit-testable with no network, and a thin per-host
  round-trip (`native` = reqwest, `web` = gloo-net). This is what lets one login path serve the
  headless driver and the browser.
- **Carry the two-hop flow forward.** The login reply names the world endpoint the client should
  connect to next, even though that value is static config today and there is no world server to
  connect to. The shape survives; the routing policy lands later.

## Reserved identities (carried from 0.2.3)

- Ids `0..1024` are reserved for system/pseudo-players; real players mint from `1024` up.
- `Developer` is the one reserved-range name a human may claim, at id `512`.
- `Claude` is a normal player, minted on first login. Names are case-sensitive and capped at 64
  bytes.

## Done when

`cargo run -p client --bin headless -- login Claude` prints a player id and token obtained from a
running `server/gateway`; the same command after a gateway restart prints the *same* id; and
`git ls-files | grep -i spacetime` is empty.

## Files

[`todo.md`](todo.md) — the plan · [`forks.md`](forks.md) — decisions made and why ·
[`completed.md`](completed.md) — verification log · [`issues.md`](issues.md) ·
[`blockers.md`](blockers.md) · [`deviations.md`](deviations.md)

Convention: [`../../CONVENTIONS.md`](../../CONVENTIONS.md). `docs/components/` entries for
`server/gateway` and `client/core` are written as those components get built (P2 and P5) rather
than pre-created empty, per the convention's lazy-creation rule. `docs-check` does not exist yet —
see [`forks.md`](forks.md) F7.
