# todo — login-without-spacetime

The plan for the life of the stream. Items are checkboxes; `[x]` **is** the move — tick in place,
never cut the line out. One action + one acceptance criterion each. Newest phase last (this file
reads in execution order).

Env vars this stream introduces: `RD_GATEWAY_LISTEN` (default `0.0.0.0:9473`), `RD_STATE`
(default `./state`), `RD_TOKEN_SECRET`, `RD_WORLD_URL`.

## P0 — Remove SpacetimeDB

- [ ] Delete `server/spacetime/` and `bin/st`. Acceptance: `git ls-files | grep -ci spacetime` prints `0`.
- [ ] Drop the four `/server/spacetime/*` rules from `.gitignore`. Acceptance: no `spacetime` match in `.gitignore`.
- [ ] Remove the `spacetime-start-1` container, the `resonantdust` network, and the `cargo-cache` volume. Acceptance: `docker ps -a`, `docker network ls`, `docker volume ls` each list nothing matching.
- [ ] Rewrite `AGENTS.md` so the "one thing that exists" block names the gateway. Acceptance: no `spacetime` match in `AGENTS.md`; the quoted command is a `cargo run`.
- [ ] Delete the dead `rd-plan` / `rd-execute` skills, or repoint them off `bin/rd`. Acceptance: no `.claude/skills/**` reference to `bin/rd` survives.

## P1 — Repo spine

- [ ] Add a root `Cargo.toml` workspace with members `server/gateway` and `client/core`. Acceptance: `cargo metadata --no-deps --format-version 1` lists both package names.
- [ ] Un-ignore `Cargo.lock` and commit it. Acceptance: `git ls-files Cargo.lock` prints the path.
- [ ] Create `server/gateway` as a bin crate on axum that answers `GET /health` with `ok`. Acceptance: `curl -s localhost:9473/health` prints `ok`.
- [ ] Create `client/core` as a lib crate named `client` with `native` (default) and `web` features. Acceptance: `cargo build -p client` succeeds.
- [ ] Confirm the `web` feature builds for wasm. Acceptance: `cargo build -p client --no-default-features --features web --target wasm32-unknown-unknown` succeeds.

## P2 — The player registry

- [ ] Write `docs/components/server/gateway/design/players.md` fixing the row shape and id ranges. Acceptance: the file names every field and its width, and is linked from the components map.
- [ ] Define `Player { player_id, name, created_secs, last_login_secs, flags }` in `server/gateway/src/players.rs`. Acceptance: a serde round-trip test passes.
- [ ] Implement `PlayerStore` as a trait, so the ledger is swappable. Acceptance: `players.rs` names the trait and the JSONL impl separately; nothing outside the module names the impl.
- [ ] Back it with an append-only JSONL ledger at `$RD_STATE/players.jsonl` plus an in-memory name→id index. Acceptance: a test writes two players, reopens the store, reads both back.
- [ ] Reserve ids `0..1024` and mint real players from `1024` up. Acceptance: a test asserts the first `claim_or_login("Alice")` on an empty store returns `1024`.
- [ ] Provision `Developer` at reserved id `512` on first boot. Acceptance: `claim_or_login("Developer")` on an empty store returns `512`, not `1024`.
- [ ] Port `validate_player_name` — non-empty after trim, ≤64 bytes, no control chars. Acceptance: a test table rejects `""`, `"   "`, 65 bytes, and `"a\u{7}b"`.
- [ ] Make `claim_or_login` idempotent by name. Acceptance: a test calls it twice with `"Claude"` and asserts one ledger line and one id.
- [ ] Stamp `last_login_secs` on every successful login. Acceptance: a test logs in twice across a clock bump and sees the field advance.
- [ ] Fsync the ledger append before the reply returns. Acceptance: a test kills the process after a login and finds the line on reopen.

## P3 — The session token

- [ ] Write `docs/components/server/gateway/design/token.md` fixing the token format and claims. Acceptance: the file states the wire string, every claim, and what verifies it.
- [ ] Mint a `v1.<payload>.<mac>` token HMAC'd with a gateway secret. Acceptance: a test round-trips mint→verify and recovers the player id.
- [ ] Reject a tampered payload, a bad MAC, and a past expiry as three distinct errors. Acceptance: three tests, each asserting its own error variant.
- [ ] Read the secret from `RD_TOKEN_SECRET`, else generate and persist `$RD_STATE/token.key` on first boot. Acceptance: a token minted before a gateway restart still verifies after it.
- [ ] Refuse to boot on a secret shorter than 32 bytes. Acceptance: `RD_TOKEN_SECRET=short` exits non-zero with a message naming the minimum.

## P4 — The gateway login endpoint

- [ ] Write `docs/components/server/gateway/intent/login.md` — the round-trip, who trusts what, why TOFU. Acceptance: the file states the trust boundary and links F2.
- [ ] Add `POST /login` taking `{ name }` and returning `{ player_id, name, token, expires_secs, world }`. Acceptance: `curl -sX POST -H 'content-type: application/json' -d '{"name":"Claude"}' localhost:9473/login` returns a non-zero `player_id`.
- [ ] Source `world` from `RD_WORLD_URL` static config, not a directory lookup. Acceptance: `RD_WORLD_URL=ws://x:1/ws` appears verbatim in the login reply.
- [ ] Add `POST /verify` taking `{ token }` and returning `{ player_id }` or `401`. Acceptance: a `/login` token verifies; one mutated byte gives `401`.
- [ ] Return `400` with a readable `{ error }` on an invalid name. Acceptance: posting `{"name":""}` gives `400` and a message naming the rule it broke.
- [ ] Keep `CorsLayer::permissive()` so a browser on another origin can read the reply. Acceptance: a cross-origin `fetch` from the vite dev server resolves rather than throwing.

## P5 — client/core login

- [ ] Write `docs/components/client/core/design/session.md` — what the client holds after login. Acceptance: the file names every session field and its lifetime.
- [ ] Define `Command::Login { name }` and `Event::LoggedIn` / `Event::LoginFailed` in `client/core/src/api.rs`. Acceptance: `cargo build -p client` succeeds.
- [ ] Add the pure `login_url` + `parse_login` pair in `client/core/src/gateway.rs`. Acceptance: unit tests parse a real `200` body and a `400 {error}` body with no network involved.
- [ ] Implement the `native` round-trip over reqwest. Acceptance: `cargo test -p client --features native` passes against a gateway spawned by the test.
- [ ] Hold `player_id` + token in a `Session` and expose `Client::player_id()`. Acceptance: the headless driver prints the id read from the session, not from the raw reply.
- [ ] Add a `headless` bin that logs in and prints the reply. Acceptance: `cargo run -p client --bin headless -- login Claude` prints a player id and a token prefix.
- [ ] Implement the `web` round-trip over gloo-net. Acceptance: the wasm build resolves a browser-console login to the same player id the headless driver gets.

## P6 — Prove it end to end

- [ ] Log in as `Developer` then `Claude` against a fresh gateway. Acceptance: ids are `512` and `1024` respectively.
- [ ] Restart the gateway and log both in again. Acceptance: both ids are unchanged, and the ledger still has exactly two lines.
- [ ] Confirm the ledger is readable by eye. Acceptance: `cat $RD_STATE/players.jsonl` shows one JSON object per line naming both players.
- [ ] Delete the ledger and re-login both. Acceptance: `Developer` returns `512` and `Claude` returns `1024` again.
- [ ] Write `docs/components/README.md` — the map: every component, path, what it deploys, alive-vs-dead. Acceptance: `server/gateway` and `client/core` are alive; the other five folders are listed as empty placeholders.
- [ ] Record the round-trip in `completed.md` with the commands and their output. Acceptance: a fresh session can re-run the proof from that entry alone.
