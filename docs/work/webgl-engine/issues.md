# Issues — webgl-engine

_The sharp edges of owning the renderer. Most of the client is already ours; the risk is concentrated in the
render backend + keeping parity during the migration._

---

## I-1 · State management is the hard part Pixi did for us

Tracking blend / scissor / viewport / bound program / VAO / target / textures — and only issuing GL calls
when they change — is the non-trivial thing Pixi provides, and getting it subtly wrong is the classic
custom-renderer bug (a stale bind → wrong draw). BUT: owning the **whole** loop is what makes it tractable —
there's no foreign framework whose cache we corrupt (the exact failure that sank raw-GL-in-Pixi,
`caster-lut D-2`). Start explicit (set state per draw), then add a minimal
tracked cache ([F3](forks.md#f3)). Test with a state-torture scene (many targets/programs/blends per frame).

## I-2 · The texture/atlas seam

The atlas packer + LOD are ours, but they currently wrap Pixi `Texture`/`TextureSource` and read
`Texture.frame` for the sub-image UV. Port to the engine's `Texture`: upload (from the fetched bytes /
ImageBitmap) + hold the atlas-page GL texture + expose the frame UV rect (we already compute it). Keep the
resolver's tier logic (master→preview→geo) untouched — only the texture *object* changes.

## I-3 · Keep the pixijs client working throughout

`client/pixijs` must stay building + running until W6 parity. Develop `client/webgl` in parallel; don't
share mutable state or break the pixijs build. Only W6 flips primary + archives pixijs. (The docs-authority
hooks still gate both.)

## I-4 · Parity is the cutover bar — verify feature-for-feature

The new client must match pixijs on: login → world (bakes, display, zoom/LOD, pan) → panels
(chat/debug/settings/RT) → `/shadowcast` → `/overlayRT` per channel → `/showRT` → `/es300` → all debug
commands. Build a checklist and tick it in the browser (side-by-side with pixijs) before cutover — a missed
path is a regression shipped.

## I-5 · Shader GLSL copies; only the harness is rewritten

The shader *logic* (material reconstruction, the bakes, the shadow merge/display, OKLab) is already ES 3.00
and correct — **copy the GLSL bodies verbatim** into the engine's `Program`. Rewrite ONLY the harness that
`compileHighShaderGlProgramES300`/high-shader gave us: the vertex transform (uProjectionMatrix·world·local
→ our camera uniforms), the sampler/uniform binding, roundPixels. Diffing the GLSL against pixijs guards
against visual regressions.

## I-6 · The WASM client-core seam

The pixijs client drives the game via the Rust `client/core` compiled to WASM (login, commands, events,
zone subscribe). `client/webgl` reuses the **same** WASM module + its API — this is a copy of the loading +
bridge glue, renderer-agnostic. Confirm the vite wasm plumbing + the `/content` + `/textures` fetch seams
carry over unchanged.

## I-7 · Scope honesty — this is the rest of the engine

Owning the renderer means owning: context loss/restore, resize/DPR, the frame budget, texture memory, and
the draw pipeline — things Pixi handled quietly. Phase it (W1–W7) so each is a verifiable milestone, and
don't chase Pixi's full generality — build only what THIS client uses (the copy-vs-rewrite table in the
README is the scope boundary). "Bounded" ≠ "small".

## I-8 · Subscription race — `bridge.start()` outruns connection-ready (a latent bug OUR SPEED exposed)

**Symptom:** some loads come up with a blank viewport — the cold-tile snapshot never arrives (fresh users:
some deliver 1300+ tiles + render the world, some deliver 0). The renderer is fine — when tiles arrive they
draw correctly.

**Cause:** `WorldScene.onEnter` calls `bridge.start()` (→ `client.setAnchor` → the first zone subscription)
**synchronously the instant `login()` resolves**. `login()` resolves on `login_ok` (gateway resolve + connect
+ auth), but the world-server subscription channel isn't reliably ready that same tick — so the first
setAnchor/subscribe races it and, when it loses, the server never streams the cold snapshot.

**NOT introduced by the port** — verified: `client/WasmClient.ts`, `wasm.ts`, `environments.ts`, and
`LoginScene.ts` are **byte-identical** to pixijs (comments aside), and both call `bridge.start()` the same
way. The race was always latent. **PixiJS's heavier startup** (`Application.init`, WebGL context creation,
high-shader compilation) gave the connection a beat before `start()` ran, **masking** it. Our leaner no-Pixi
client mounts + subscribes sooner and loses the race — i.e. we _exposed_ a pre-existing bug, we didn't create
one. (A good argument for the migration: the client is measurably faster.)

**Refined diagnosis (2026-07-21).** Instrumented traces: on a stuck load the anchor subscription IS pushed
correctly (`SUB tile=(100,50)`), but **zero cold rows ever arrive for the whole session** — and re-issuing the
anchor does NOT recover it, **even when the anchor genuinely CHANGES** (nudged tile 100→103 → still nothing).
So it's not a dropped-then-deduped command: the world-server cold-tile **stream never starts** on the bad
connection, and no client-side re-subscribe fixes it (the [W4g](todo.md#w4) same-anchor retry is therefore
insufficient — kept only as a best-effort for the dropped-first-push variant). When a load DOES stream, the
render is perfect + fills the viewport. Intermittent (~half of fresh loads); refreshing gets a new connection
that may work. Render side is exonerated.

**Event trace (2026-07-21).** Logged every core `on_event` + every `setAnchor`. On EVERY load — good or
stuck — the client does its job correctly and deterministically: `loggedIn` fires, then ~90ms later
`SETANCHOR (100,50) world=true` (core connected, correct tile), no `disconnected`, no fire-and-forget. On a
GOOD load `coldTiles` stream ~50ms after the subscribe. On a STUCK load the identical-looking subscribe
produces NO stream ever, and a genuine anchor CHANGE doesn't recover it. So:
- RULED OUT: client dropping the subscribe (world=true), a missing poll (core is push-only via `on_event`),
  and the render (renders correctly whenever data arrives).
- REMAINING: the world-server / **edge** intermittently doesn't stream back for a valid, correctly-sent
  subscription. Our fast client subscribes ~90ms after `loggedIn`; likely the server↔edge subscription
  routing isn't live yet, and a subscribe that lands before edge-ready is DROPPED server-side (not queued).

**NO RETRIES** (per the user — the same-anchor retry was removed; it was the wrong fix and can't help a
correctly-sent subscribe). **Deterministic fix (needs the core↔server contract, the user's domain):** answer
whether a pre-edge-ready `setAnchor` is QUEUED (delivered when edge readies) or DROPPED. If dropped, either
(a) add a core event signalling "subscription channel / edge live" and gate `bridge.start()` on it (the
current events — loginStarted/serverResolved/loggedIn/coldState/coldTiles/coldThings/stateObject/zoneClosed/
callStats/subStats/clockSync — have none), or (b) make the server queue a subscribe that arrives pre-edge.
Separate open item: with the grid on, a GOOD load shows content bounded to ~9 zones — confirm world-bounds
vs under-subscription (needs a clean interactive test / the `/showRT` debug tools).

**ROOT CAUSE FOUND (2026-07-21) — it is a SERVER (edge) bug, NOT the client.** Traced the full path into
`server/edge/src/ws.rs::build_world`. On each client WS connect the edge eagerly connects its per-client
upstreams — players + event/data shards + the **cold-tile / cold-thing** shards — and for each does
`await_ready(ready).await.then_some(conn)` with a **5s `CONNECT_TIMEOUT`** (`connections.rs`). The cold-tile/
thing relay (`entity_state.on_insert/on_update → ColdTile/ColdThing frames`) is wired **only inside
`if let Some(t) = &tile`**. So if the cold-shard's SpacetimeDB `on_connect` doesn't fire within 5s,
`await_ready` returns false → `tile = None` → **the relay is never wired, silently, for the whole session.**
The client's `SubscribeZone` still runs `seed_zone` (generates + inserts the cold rows into the shard), but
nothing relays those inserts back → **no cold tiles ever.** A refresh = a new WS = a fresh `build_world` =
another chance for the cold connect to beat 5s → intermittent.

This explains EVERYTHING observed: subscribe sent correctly (`world=true`), zero rows for the whole session,
an anchor change never recovers it (the relay is wired ONCE at connect, not per-subscribe), refresh sometimes
works. **My earlier "our speed exposed a client race" was WRONG** — this is edge↔cold-shard connection
flakiness, agnostic to which client (pixijs hits it identically). **Fix is in the edge, not the client:** on a
cold-shard `await_ready` timeout, don't serve a silently-broken session — FAIL the login (client gets a clean
error / reconnects) or RETRY the shard connect, and/or investigate why the cold-shard `on_connect` intermittently
exceeds 5s. Client `client/webgl` is exonerated; W4g is really an edge fix.

**CLIENT-SIDE ROOT CAUSE FOUND + FIXED (2026-07-21).** There were TWO distinct failure modes, not one:
- **(A) No deliveries** — the edge cold-shard `await_ready` 5s timeout above (server-side, relay dead). Real
  but rarer.
- **(B) Deliveries arrive but don't render** — the one the user's testing actually hit, and the bigger one.
  Instrumented: on a stuck load core delivered **52 cold-tile rows to the bridge**, but the map held only
  **3 prims**. Cause: the webgl `WorldScene` **never wired `onContentReloaded → bridge.setContent`** (pixijs
  does). Boot loads the build-time **embed** corpus; login fires `reloadContent` (fetch the server's corpus,
  async). The bridge is created at scene-enter with whatever `ctx.content` is then — if `reloadContent` hasn't
  finished, it's the embed, whose biome defs don't cover the server's zones, so `content.zoneTilePrims` returns
  empty → the rows expand to nothing. And webgl **never recovered** (the reload→setContent hook was dropped in
  the W4d port), so it stayed blank — intermittent purely on whether the fetch beat scene-enter.
  **Fix:** wire `onContentReloaded(() => { bridge.setContent(getContent()); moverLayer.setContent(getContent()); })`
  in `WorldScene` (unsubscribe on exit) — `setContent` re-reads stems + **re-expands every stored cold row**
  through the new corpus. Verified: loads now render reliably (2962 prims every time; the re-expansion fires,
  visible as the cold-delivery counter climbing past 52). Mode (A) — the edge timeout — remains open as W4g.
