# forks — login-without-spacetime

Decision points, the options, which we chose, why. Chronological append. A fork is mine to
resolve; a blocker needs the user. Everything below is resolved — none of it is waiting on input.

---

### F1 — Where does the player registry live now? · 2026-08-10 · resolved

SpacetimeDB's `players` module held the registry. With ST gone it needs a home.

| Option | Verdict |
|---|---|
| **Append-only JSONL ledger + in-memory index** | **chosen** |
| SQLite (rusqlite/sqlx) | rejected |
| Embedded KV (sled/redb) | rejected |
| In-memory only | rejected |

**Why.** The registry is written once per login and read once per login — the 0.2.3 module comment
said as much ("the auth DB is low-write"). At that volume a file is not a compromise, it is the
right shape: zero deps beyond serde, human-readable when you want to know who exists, resettable
with `rm`, and diffable. SQLite and sled both re-import the class of problem this iteration exists
to remove — a schema, a migration story, a binary blob you cannot read — for a table that will hold
tens of rows for a long time. In-memory alone fails the actual requirement: `claim_or_login` must be
idempotent *across restarts*, or logging in as `Claude` tomorrow mints a second Claude.

Behind a `PlayerStore` trait, so the swap is contained when write volume justifies it.

---

### F2 — What is the credential? · 2026-08-10 · resolved

The user asked for "another method to handle login" now that ST is gone. First establish what was
there: 0.2.3 took `{"t":"login","name":"Alice"}` and relayed it to `claim_or_login`. **There was no
password.** The ST identity/JWT was never a user credential — it was how the CLI proved database
ownership. So there is no user-facing auth being *lost* here.

| Option | Verdict |
|---|---|
| **Name-only TOFU + a server-issued session token** | **chosen** |
| Name + password | rejected for now |
| Name + a per-player secret issued at create time | **designed for, next step** |
| OAuth / OIDC | rejected |

**Why.** Keeping TOFU keeps the login flow the user asked for — type `Developer` or `Claude` and
you are in — and matches the trust model the game already had. Adding passwords now costs the
flow and buys nothing while the only accounts are two dev names on localhost.

What we *do* add is the **session token**, because that is the part ST's JWT was quietly providing:
a way for a service that is not the gateway (the future world server, the worker) to trust a
connection without calling the gateway on every frame and without a shared session table. Stateless
HMAC, so any service holding the key verifies alone.

**The honest limitation:** with TOFU, anyone who can reach `/login` can claim any unclaimed name,
and can claim `Developer`. That is acceptable on localhost and **not** acceptable the moment the
gateway is reachable from outside. The fix is option 3 — issue a per-player secret at create time,
require it on subsequent logins — and the wire shape leaves room for it (the request is an object,
not a bare string). This must be closed before any non-local deployment.

---

### F3 — Stateless token or server-side session table? · 2026-08-10 · resolved

**Chosen:** stateless HMAC token, `v1.<base64url(payload)>.<base64url(mac)>`.

**Why.** A session table needs to be shared between the gateway and every service that verifies a
connection. Shared mutable state between services is exactly the requirement that argues for a
database — and we just removed ours. A signed token pushes verification to the edge of each service
with no coordination. The cost is that revocation is not immediate (a token is valid until it
expires); at this stage, with short expiries and a restartable secret, that is the cheaper problem.

---

### F4 — Does the gateway still resolve a world server? · 2026-08-10 · resolved

0.2.3's gateway was a directory: `GET /server` consulted the `index` ST database and returned an
endpoint. The index database is gone, and there is no world server in 0.3.0 to return.

**Chosen:** the login reply carries a `world` field, sourced from `RD_WORLD_URL` static config.

**Why.** Deleting the field would be simplifying away known future intent — the two-hop flow
(acquire an endpoint, then connect to it) is the shape we know we want, and it is far cheaper to
carry a static field forward than to re-thread the concept through the client later. Backing it
with config rather than a directory keeps this stream honest: there is no routing policy here, and
the plan says so rather than implying one exists.

---

### F5 — One cargo workspace, or independent crates? · 2026-08-10 · resolved

**Chosen:** a single root workspace, `server/gateway` + `client/core` as members. No Docker for the
Rust side.

**Why.** 0.2.3 built every crate in its own container with its own `target/`, and the two recurring
costs were exactly that: the stale-mtime problem (a changed file skipped by the container's cargo,
needing a `touch` to force a rebuild) and root-owned build output the host could not delete — both
hit again during the 0.3.0 teardown, where 21G of it needed a container to remove. A workspace gives
one `target/`, one lockfile, path deps that just work, and `cargo test` across the tree.

The daemon needed a container because it was a pinned third-party binary. Our own Rust does not.

---

### F6 — Is `Cargo.lock` committed? · 2026-08-10 · resolved

**Chosen:** yes, committed. The 0.3.0 `.gitignore` inherited `Cargo.lock` from 0.2.3, where every
crate was a library-shaped build in a container.

**Why.** The workspace root now produces binaries (`gateway`, `headless`). For binaries the lockfile
is the reproducibility record — without it, a fresh clone resolves whatever is newest and a
dependency bump becomes an unattributable behaviour change. P1 removes the ignore rule.

---

### F7 — Reinstate the docs-authority tooling? · 2026-08-10 · resolved

The user restored `docs/CONVENTIONS.md` verbatim from 0.2.3, so the convention is authoritative
again. But the convention references `bin/rd docs-check` throughout — index-link enforcement,
checkbox-vs-bullet errors, oversized-item warnings, `current/` freshness stamps — and `bin/rd` was
deleted with the 0.2.3 tree.

**Chosen:** follow the convention by hand for this stream; port `docs_check.py` from
`0.2.3:bin/lib/docs_check.py` as its own small stream once there is enough tree to check.

**Why.** The convention's value is the discipline, which does not depend on the checker. The
checker's value is catching drift across hundreds of files, and today there is one work folder and
no component tree — it would be guarding nothing. Porting it now also drags `bin/rd`'s profile and
env machinery back in, which is 0.2.3 architecture we deliberately dropped.

**The risk, stated:** the convention notes the continuation hook once sat dead for six days because
nothing enforced that plan files contain checkboxes. Hand-following a convention whose enforcement
was built *because* hand-following failed is a known-weak choice. It is the right call only while
the tree is this small; the porting stream should open before the component docs multiply.

---

### F8 — Component docs now, or as each component is built? · 2026-08-10 · resolved

**Chosen:** as each is built — the `design/` and `intent/` files are items inside P2, P3, P4, and
P5 rather than a pre-created empty tree.

**Why.** `CONVENTIONS.md` says folders are created lazily, "as we actually work a component, not up
front", and an empty `design/` is worse than none: the convention makes `design/` outrank the
ticket, so a stub that says nothing becomes an authority that means nothing. Writing each design
file in the phase that builds it keeps it true at the moment it gains authority.
