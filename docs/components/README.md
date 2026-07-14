# Components — index / map

_Last mapped: 2026-07-14._

A **component** is a self-contained chunk we **deploy** or **share**. Shared components get their
own entry because *how they change dictates how their consumers operate*. Each component gets a
`docs/components/<group>/<name>/{design,intent,current,plan}` folder — created **lazily** as we
work it (per [../CONVENTIONS.md](../CONVENTIONS.md)), not up front. **Individual tables are not
components** — the module that holds them is.

This file is the map: what exists, where it lives, what it deploys/shares, and what's alive vs
dead. It's the antidote to "I forget… which is why we need this."

## `client/` — deployable
- **client/core** (`resonantdust-client`) — headless session core: `Command` in / `Event` out,
  the tokio engine (`engine.rs`) + its wasm mirror (`web.rs`). Shared by npc + pixijs(wasm).
- **client/npc** — automated player over client/core (wildlife/wolf pack). A bot process.
- **client/pixijs** — browser renderer (TS + pixi.js). The web client bundle.

## `server/` — deployable
- **server/edge** — the world server clients log into: per-session zone subs, appends events to
  the shard, serves `/content` + `/textures`. **Contains `worldgen`** (`edge/src/worldgen.rs`) —
  server-only biome generation that reads `content/` and seeds the shard's `cold` table when a
  client subscribes a zone (anchor/subscribe-driven). *Worldgen is a piece of edge, not its own
  component.*
- **server/gateway** — directory over the `index` DB; the client's first hop (`GET /server` →
  world-server URL).
- **server/master** — ticks a shard: `drop_timed_out → bump`, periodic GC. Respects `paused`.
- **server/worker** — resolves `event_log → state` (two-phase lifecycle, composition, cross-shard
  convergence, actor-reads).
- **server/spacetime** — the SpacetimeDB workspace; not itself deployed — it *holds* the modules
  + the pipeline macro + build scripts.

## `server/spacetime/server/modules/` — deployable ST modules
- **shard** — the unified tick pipeline: `event_log`/`state`/`state_log`/`holder`/`cold`/
  `tic_meta`/… + lifecycle reducers. The game's live + cold data. Its `cold` table (object model)
  **supersedes cold_tiles/cold_things.**
- **players** — auth/login (`claim_or_login`) + player records.
- **index** — the **directory + presence**: region→shard routing (`region_shards`), shard registry
  (`shards`), server registry + heartbeat/liveness (`servers`), **player→server assignment =
  presence** (`player_servers`), GC. gateway reads it; edge registers + heartbeats into it.
- **chat** — chat messages.
- **cold_tiles / cold_things — ☠ RETIRED / DEAD.** Superseded by shard's `cold` table (div #3,
  commits `2b2fc58` / `d47f152`), but the module **dirs + `redeploy.sh` fam-entries were never
  removed**. Should be deleted.
- **experiment — ☠ DEAD.** sync-experiment leftover; no build references. Delete.

## `server/spacetime/server/` (shared, ST-side)
- **pipeline** (`resonantdust-pipeline`) — the `decl_tick_pipeline!` macro that generates a
  module's tables + lifecycle reducers. Shared *by* modules (today: shard). A change here changes
  every module built on it — hence tracked as a component.

## `shared/` — shared crates (change here dictates how consumers operate)
- **shared/codec** (`resonantdust-codec`) — bit-packing: `event_word` (the DSL word frame),
  entity/zone `refs`, the `object` model. The wire shapes everything agrees on.
- **shared/tick** (`resonantdust-tick`) — domain effects + the **event-DSL VM** (interpreter).
- **shared/dsl** (`resonantdust-dsl`) — the **content DSL**: `content/*.rd` → runnable `Bundle`
  (data/visual defs).
- **shared/wasm** (`resonantdust-shared`) — the `#[wasm_bindgen]` layer (`WorldClient`, content
  helpers) exposing client/core + codec to JS.
- **shared/core** (`resonantdust-core`) — foundational pure-Rust shared logic. **Currently a
  hello-world placeholder** — barely used; decide if it earns its place.
- **shared/pkg, shared/target — build outputs, NOT authored components** (wasm-pack output /
  cargo target dir). No design/intent docs.

## The two DSLs (prefix them so they don't blur)
- **event-dsl** — the `Vec<u64>` action-program (event *definitions*). Not a deploy component; a
  cross-cutting concept defined by **shared/codec** (`event_word` frame) + **shared/tick** (the
  VM). Spec: `docs/spacetime-tables/event-dsl.md`.
- **content-dsl** — the `.rd` files (data/visual defs). Crate: **shared/dsl**. Authoring specs:
  `dev/dsl` (below).

## `dev/` — developer + gameplay (PROPOSED; today under `bin/` + `content/` + `textures/`)
Held as components because scripts are the foundation of how we develop.
- **dev/scripts/rd** — the dev CLI (build / deploy / run). Today: `bin/rd` + `bin/lib`.
- **dev/scripts/art** — asset generation (sprites, laigter normals). Today: `bin/art` (+
  `bin/laigter`, `bin/marigold`).
- **dev/scripts/dsl** — publishes the content DSL to R2 for server/client consumption. Today:
  `bin/dsl`.
- **dev/dsl** — the `.rd` content-DSL *specifications* (data + visual). Today: `content/`.
- **dev/textures/sprites**, **dev/textures/templates** — sprite art + generation templates.
  Today: `textures/` (gitignored) + templates.

## Open / to-resolve
- **Delete dead modules** cold_tiles, cold_things, experiment (+ their `redeploy.sh` fam entries
  and any config). Confirms the user's "not sure we should have cold_things/cold_tiles anymore."
- **shared/core** is a placeholder — keep, fill, or absorb?
- The **dev/ reorg** (`bin/` → `dev/scripts/`) is proposed, not done.
- Per lazy-create, per-component `{design,intent,current,plan}` folders come as we work each —
  first up is **shard** (during the `spacetime-tables` → `components/…` migration).
