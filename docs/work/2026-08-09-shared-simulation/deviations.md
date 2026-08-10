# Deviations — shared-simulation

_Where the code departs from the plan (`components/<c>/{design,intent}`). Rows: date · what the
plan says · what the code does · why · fix/status. Log at the moment of deviating; also log one
you find._

## D3 — core OWNS the corpus but does not FETCH it
**2026-08-10 · introduced · OPEN, small.**

**What the plan says.** P2c item 3: *"Give core the corpus: fetch `/content` at login into an
`Arc<Bundle>` on the engine."*

**What the code does.** `ClientWorld` owns `Option<Arc<Bundle>>` and every derived answer reads it
from there — the item's acceptance (`derive_paces`/`pathable` no longer take a `&Bundle` from the
host) is met. But the HTTP GET still happens host-side and arrives via `Client::set_corpus`.

**Why.** The two hosts fetch on different transports — native reqwest against the world server's
`/content`, browser `fetch` through `contentBoot.ts` — and core has no cross-target HTTP today.
Writing one would mean a `#[cfg]` split inside core, i.e. two implementations of the fetch, which
is the defect this stream exists to delete. Handing core the bundle keeps ONE owner of the parsed
corpus, which is the part that actually mattered: nothing downstream can now be passed a different
one.

**Fix/status.** OPEN. Worth doing when core grows a transport abstraction it needs for other
reasons; not worth a `cfg` fork on its own. Recorded so "core fetches the corpus" is not later
assumed from the ticked box.

## D2 — `client/core`'s own intent doc already says webgl should be dumb
**2026-08-10 · found · OPEN → P2b.**

**What the plan says.** [`docs/components/client/core/intent/client.md`](../../components/client/core/intent/client.md):
*"The **client** is the game client as a headless Rust library — **all logic, no rendering**...
`pixijs` will later become a **dumb display** layered over it (via a wasm/FFI shim), and other Rust
programs can drive it the same way. **There is one contract for every host.**"*

**What the code does.** There is no one contract. `client/npc` builds a composed world view in
Rust; `client/webgl` builds a second one in TypeScript; `client/core` holds neither. Same for the
walk (D1). Both hosts re-implement the logic core was specified to own.

**Why this matters more than D1.** I went looking for a reason NOT to pull the world view into
core — a documented decision that core is a transport layer, some cost that made it wrong. There
is none, and the opposite is written down. Two supporting facts:

- **Core is not stateless and never was.** `ZoneManager` alone holds seven maps of subscription
  state, plus the tic estimate and the session. Retaining a composed world view is consistent with
  what core already is.
- **Moving it REMOVES a cost rather than adding one.** `findChords` currently takes a `cells` grid
  that webgl builds and copies across the JS↔wasm boundary on every call
  (`MoverLayer.ts:709-715`). With the view in core, wasm reads it directly and the copy disappears.

**One distinction to preserve, not collapse.** `WorldBridge` conflates two things: the composed
*model* (what kind is at this cell, is it pathable — simulation, the server must agree) and the
*prim cache* (what is drawn there — presentation, per F2's line). Core takes the model. Webgl keeps
the prim cache as a **derived view** of it, not a second source of truth.

**Fix/status.** OPEN — [P2b](todo.md), four items, ahead of P4.

## D1 — the client's simulation lives in the presentation layer
**2026-08-09 · found, not introduced this stream · OPEN → the whole stream is the fix.**

**What the plan says.** `client/webgl` renders; `client/core` is the client. The Rust core holds
the world model and every host — browser, npc, any future headless consumer — reads it. Simulation
rules the server and a client must both compute live in `shared/` and are called, not copied.

**What the code does.** The movement rule is implemented **twice** and shared **zero** times:

- `server/worker/src/main.rs` owns it privately — `REANCHOR_TICS` (:385), `CHORD_CAP_TILES` (:387),
  `hop_stride_tiles` (:391), `resolve_walk_position_for` (:412), `MOVE_TO` (:2953),
  `MOVE_STEP` (:2986).
- `client/webgl/src/game/world/MoverLayer.ts` re-implements it in **TypeScript** — `Spec` (:110),
  the walk annotated *"The server's stepping rule, mirrored EXACTLY (worker `apply` MOVE_TO)"*
  (:130), `speedFor` (:660), `computePath` (:699), the reseed (:818).
- `client/core/src/world.rs` is 158 lines of decode and holds no positions at all.
- `client/npc/src/lib.rs:266` keeps `pawns: HashMap<u32, (i32, i32)>` — the last authoritative
  **whole tile**, no subtile, no pace, no advancement between anchors.

**Why it happened.** Speculation was built where its first consumer was — the browser, which was
the only client that had to *look* right. Nothing in the tree objected: `shared/content` already
had `path_eval`/`stat_eval`/`needs_eval`, so the seam existed and was simply not used, and the
comment "mirrored EXACTLY" was accepted as a contract when it is only a hope. *"The browser needed
it first" is not a reason — it is the absence of one.*

**Consequences, both measured.** The two walks disagree by 2× ([I1](issues.md#i1)), which the
render-chase reports honestly as a teleport — 116 of 214 reseeds past `CHASE_SNAP_TILES` in 54 s.
And every headless consumer reasons about pawn positions that are tile-granular and up to one
chord stride (8 tiles) stale, because the code that closes the gap runs in a browser tab.

**Fix/status.** OPEN. [`todo.md`](todo.md) P1–P4: the rule moves to `shared/content::move_eval`,
`client/core` gains the mover track, npc reads it, webgl keeps only the render-chase
([F2](forks.md#f2)). P5 adds the guard that would have caught this, because nothing did.
