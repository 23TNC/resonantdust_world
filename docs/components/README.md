# Components — index / map

_Last mapped: 2026-07-15._

A **component** is a self-contained chunk we **deploy** or **share**. Shared components get their
own entry because *how they change dictates how their consumers operate*. Each component gets a
`docs/components/<group>/<name>/{design,intent,current,plan}` folder — created **lazily** as we
work it (per [../CONVENTIONS.md](../CONVENTIONS.md)), not up front. **Individual tables are not
components** — the module that holds them is.

This file is the map: what exists, where it lives, what it deploys/shares, and what's alive vs
dead. It's the antidote to "I forget… which is why we need this."

> **Physical layout** — where each crate sits on disk, plus the 2026-07-11 old→new rename key that
> lets you read pre-restructure history — is [`docs/repo-layout.md`](../repo-layout.md). Kept
> separate on purpose: this map is *conceptual* and shouldn't carry a historical rename table.

## `client/` — deployable
- **client/core** (`resonantdust-client`) — headless session core: `Command` in / `Event` out,
  the tokio engine (`engine.rs`) + its wasm mirror (`web.rs`). Shared by npc + pixijs(wasm).
- **client/npc** — automated player over client/core (wildlife/wolf pack). A bot process.
- **client/pixijs** — browser renderer (TS + pixi.js). The web client bundle.

## `server/` — deployable
- **server/edge** — the world server clients log into. Today: login + clock sync, and it serves
  `/content` + `/textures`. **Contains `worldgen`** (`edge/src/worldgen.rs`) — server-only biome
  generation that reads `content/`; it has nothing to seed until a shard exists. *Worldgen is a
  piece of edge, not its own component.* Its zone-subscription surface went with the shard.
- **server/gateway** — directory over the `index` DB; the client's first hop (`GET /server` →
  world-server URL).
- **server/spacetime** — the SpacetimeDB workspace; not itself deployed — it *holds* the modules
  + build scripts.
- **server/master, server/worker, server/orchestrator — the rebuild's SDK-client processes. NOT
  BUILT.** master = metronome (bumps the tic in lockstep, sweeps, assigns the per-tic orchestrator);
  orchestrator = groups each tic's events into components and assigns a worker each; worker = composes
  a component. Flow: [`intent/spacetime-again/`](../intent/spacetime-again/README.md); items W4–W6 in
  [`work/spacetime-again/`](../work/spacetime-again/README.md). (An earlier master/worker existed to
  drive the deleted pipeline and were removed 2026-07-15; these are fresh.)

## `server/spacetime/server/modules/` — deployable ST modules
_Map + conventions: [`server/spacetime/`](server/spacetime/README.md)._
- **[players](server/spacetime/modules/players/)** — auth/login + player→shard routing. Live.
- **[index](server/spacetime/modules/index/)** — directory + presence (`servers`,
  `player_servers`); the gateway routes on it, the edge heartbeats into it. Its region→shard tier
  is vestigial. Live.
- **[chat](server/spacetime/modules/chat/)** — the message feed. Live; client link missing.
- **[event_shard](server/spacetime/modules/event_shard/)** — the event queue + log. **Not built.**
- **[data_shard](server/spacetime/modules/data_shard/)** — composition slots + client-visible
  state. **Not built.**
- **shard, pipeline — ☠ GONE (2026-07-15).** The unified tick pipeline, deleted for a ground-up
  rebuild; nothing legacy kept. Replaced by `event_shard` + `data_shard` above, whose flow is
  [`intent/spacetime-again/`](../intent/spacetime-again/README.md).
- **cold_tiles / cold_things / experiment — ☠ GONE (2026-07-14).**

## `shared/` — shared crates (change here dictates how consumers operate)
- **shared/codec** (`resonantdust-codec`) — bit-packing: the `refs` / `object` reference layouts,
  the `tic` ring, and the legacy `packed` zone_id. The wire shapes everything agrees on —
  authoritative in [`../VARIABLES.md`](../VARIABLES.md).
- **shared/tick — ☠ GONE (2026-07-15)** with the pipeline it was the SDK-free core of.
- **shared/dsl** (`resonantdust-dsl`) — the **content DSL**: `content/*.rd` → runnable `Bundle`
  (data/visual defs).
- **shared/wasm** (`resonantdust-shared`) — the `#[wasm_bindgen]` layer (`WorldClient`, content
  helpers) exposing client/core + codec to JS.
- **shared/core** (`resonantdust-core`) — foundational pure-Rust shared logic. **Currently a
  hello-world placeholder** — barely used; decide if it earns its place.
- **shared/pkg, shared/target — build outputs, NOT authored components** (wasm-pack output /
  cargo target dir). No design/intent docs.

## The two DSLs (prefix them so they don't blur)
- **event-dsl** — the action-program an event carries (`actions : Vec<u32>`). Not a deploy
  component; defined in [`../ACTIONS.md`](../ACTIONS.md) (encoding + palette) and realized by
  `shared/codec` (the words) + the worker's VM (running them) + the edge (composing them).
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
- **shared/core** is a placeholder — keep, fill, or absorb?
- The **dev/ reorg** (`bin/` → `dev/scripts/`) is proposed, not done.
- Per lazy-create, per-component `{design,intent,current,plan}` folders come as we work each — so a
  component **listed above with no folder is normal**, not a gap; this map is its home until it
  earns one. **Folders so far:** the five `server/spacetime` modules (intent + current/plan);
  partial, as work touched them — `client/core`, `client/pixijs`, `server/gateway`,
  `dev/{scripts,textures}`, `shared/codec` (a pointer only; its shapes live in `VARIABLES.md`).
- **Cross-component shapes do not get component folders.** Variables live in
  [`../VARIABLES.md`](../VARIABLES.md), tables in [`../TABLES.md`](../TABLES.md), the reasoning in
  [`../notes/`](../notes/tables.md). A component doc that restates a layout is drift waiting to
  happen.
