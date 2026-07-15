# Repository layout

The top level groups crates by side of the wire. Renamed + regrouped 2026-07-11
(was a flat tree of `server` / `gateway` / `spacetime` / `server_master` /
`server_simulation` / `client` / `pixijs` / `npc`).

```
server/                     everything server-side
  edge/       (was server)            the client↔world edge server (WS + content/textures)
  gateway/    (unchanged name)        client-facing routing over the index DB
  master/     (was server_master)     the tick metronome (bump loop)
  worker/     (was server_simulation) the resolution worker pool
  spacetime/  (unchanged name)        SpacetimeDB modules (shard/players/index/chat) + daemon
client/                     everything client-side
  core/       (was client)            headless client library + `headless` bin (login + zone sub)
  pixijs/     (unchanged name)         the browser renderer
  npc/        (unchanged name)         headless bot driver
shared/                     host-agnostic crates, bind-mounted into builds (NOT moved)
  codec/ tick/ dsl/ core/ wasm/
bin/  content/  textures/  docs/  laigter/  marigold/  vscode-rd-dsl/   (unchanged)
```

## Rename map (dirs · Cargo package · binary)

| Old | Dir | Cargo package | Binary |
|---|---|---|---|
| `server` | `server/edge` | `edge` | `edge` |
| `server_master` | `server/master` | `master` | `master` |
| `server_simulation` | `server/worker` | `worker` | `worker` |
| `gateway` | `server/gateway` | `gateway` | `gateway` |
| `spacetime` | `server/spacetime` | *(modules keep names)* | — |
| `client` | `client/core` | **`client`** (see caveat) | `headless` |
| `pixijs` | `client/pixijs` | `resonantdust-world-pixijs` | — |
| `npc` | `client/npc` | `npc` | `npc` |

**Caveat — the `client/core` crate keeps package name `client`.** A Cargo package
named `core` collides with Rust's built-in `core` crate (and with `shared/core` =
`resonantdust-core`). The *directory* carries the "core" identity; the crate stays
`client` (with `[lib] name = "client"`), so `shared/wasm`'s `use client::…` needs no
change. `bin/rd`'s CLI keyword for it is `core` (`rd build core`).

## The one non-obvious rule — Cargo path-deps are *container-relative*

Rust crates build **in docker** (no host cargo). `shared/` is bind-mounted into each
build container, so a crate's `path = "../shared/codec"` resolves against the
**in-container** layout, not the host tree. Consequences when moving a crate deeper:

- **Don't "fix" the Cargo `../shared/...` paths to match the new host depth.** For
  crates built via a *sibling-mount* compose (`edge`, `npc`, `gateway`), the path stays
  container-relative; only the compose **mount source** gains a `../`
  (`../shared` → `../../shared`).
- **Exceptions, host-relative:**
  - `client/core` builds *both* standalone (client/compose) *and* transitively inside
    the repo-root wasm build (`shared/compose`), so its codec dep is host-correct
    `../../shared/codec`; the standalone mount uses `../../shared:/shared` and the path
    resolves via root-`..` clamping. (`shared/wasm` deps it at `../../client/core`.)
  - `master` / `worker` have **no build compose**, so their `../../shared/tick` is plain
    host-relative.
- **`server/spacetime` internals unchanged:** the modules still mount at
  `/workspace/server/...` in-container, so their `../../shared`, `../../../shared` paths
  are untouched; only the compose mount source moved (`../shared` → `../../shared`, and
  the bindings mount `../server` → `../edge`).

## `bin/rd` component names

`rd build|up|down|deploy|logs` speak the new names: `edge`, `gateway`, `spacetime`,
`core`, `npc`, `pixijs`, `shared`. Compose services and container names for the edge are
`edge` / `edge-claude` / `edge-test` (were `server*`); log files are `edge-<env>.log`.

## Deferred (intentionally not renamed)

The edge binary's **runtime env-var contract** — `SERVER_ENV`, `SERVER_LISTEN`,
`SERVER_STDB_URI` (and gateway's `GATE_LISTEN`, `INDEX_URI`, `INDEX_DB`) — keeps its
names. Renaming them cascades into the Rust config code with no functional gain; treat
as a separate follow-up if desired. The `docs/` design-narrative still references old
crate names/paths in places (historical); a prose sweep is a low-priority follow-up.
```
