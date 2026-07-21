# Todo — webgl-engine (execution order)

_Remaining work only. Done + verified phases move to [`completed.md`](completed.md) (they are not kept here as
ticked boxes). New component `client/webgl` — see [`README.md`](README.md), [`forks.md`](forks.md),
[`issues.md`](issues.md). The pixijs client stays working throughout ([I-3](issues.md#i-3))._

**Done (in [`completed.md`](completed.md)):** W1 scaffold · W2 engine core · W3 shell/substrate/login ·
W4a viewport+camera · W4b essential shaders · W4c SquareCache · W4d viewport pipeline + WorldBridge (the world
renders) · W4e warm layer + movers (wired) · input/URL fixes · chat console · reliable-render content-reload
fix · frame-cap fix.

**Current front: W4f / W4g** (plus the deferred texture + smooth-LOD work, W4h).

---

## W4f · Shadows + debug — DONE for the basic cut (completed.md)

Done: **`/overlayRT`** (G-buffer overlay, absorbs the W4b `overlayShader` deferral) + the **first lights +
billboard shadows** on the geo tier (`ShadowCaster`, analytic screen-space cast, `/coldlights` + `/shadows`).
Descoped per the user: `/showRT` (deferred — needs a GL→DOM readback), and the `/es300`/`/mrttest`/`/inttest`
engine spikes (the techniques are already proven in W2). Remaining, when the shadow work resumes past basic
(see [D-2](deviations.md#d-2), builds toward the `docs/work/shadows/` design):

- [ ] World-space **`shadow-cold`** bitfield (per-light bit packing in the toroidal cache layout) so shadows
      persist + become `/overlayRT shadow-cold`-inspectable (the `OVERLAY_BITS` decode is already ported).
- [ ] The **round-robin** (3 lights/frame → world-cold) + screen-hot generation, to lift the per-pixel
      analytic cost and scale toward the 24-light goal.
- [ ] Textured casters (the geo boxes → real sprite silhouettes) once the texture atlas lands (W4h).

## W4g · Fix the intermittent no-stream — deterministically, NO retries

Two failure modes were separated during diagnosis ([I-8](issues.md#i-8)):

- **Mode B (client) — FIXED** (content-reload wiring, [`completed.md`](completed.md)): data arrived but the
  bridge, stuck on the boot embed corpus, expanded it to nothing. This was the case the testing mostly hit.
- **Mode A (edge) — REMAINING:** on some connections **zero** cold rows ever arrive. Root-caused to
  `server/edge/src/ws.rs::build_world`: the cold-tile/thing relay is wired only `if let Some(t) = &tile`, and
  the per-client cold-shard connect has a **5s `CONNECT_TIMEOUT`** ([`connections.rs`](../../../server/edge/src/connections.rs)).
  If the shard's `on_connect` misses 5s, `await_ready` returns false → the relay is silently never wired for the
  whole session; `SubscribeZone` still seeds the rows but nothing relays them back. Refresh = new WS = another
  chance → intermittent. **Fix is in the edge, not the client** (client is exonerated): on an `await_ready`
  timeout, don't serve a silently-broken session — FAIL the login (clean error / client reconnects) or RETRY the
  shard connect, and/or investigate why the cold-shard `on_connect` intermittently exceeds 5s.
- [ ] Separately: with the grid on, a GOOD load shows content bounded to **~9 zones** and panning doesn't seem
      to acquire more — confirm world-bounds vs under-subscription (only the initial anchor's reach is handled).
      Needs a clean interactive test (ideally the `/showRT` tools from W4f).

## W4h · Textures — real resolver/atlas + smooth-LOD (deferred from W4c/W4d)

- [ ] Return the real `TextureResolver` + atlas (`LodPool`/`TextureAtlas`/`MaxRectsPacker`) sharing the viewport
      `Renderer`'s GL context (retire the W3 stub, [D-1](deviations.md#d-1)), so **things render as textured
      sprites** (master→preview→geo) instead of solid `geoColor` boxes.
- [ ] Restore the `Channel` **ping-pong / reproject** so zoom-LOD shifts don't flash a re-bake (W4c shipped a
      single buffer per channel as the floor).
- [ ] Live-verify the **mover visual** (W4e is wired but pawns render as geo boxes only; there are no pawns in
      the world until the npc driver runs server-side).

## W5 · Port the UI

- [ ] DOM panels already copied (W3c). The canvas-hosting panel hosts our `<canvas>`. **Collapse the
      `LayoutNode`/`PixiPanel` canvas chrome into CSS on `DomPanel`** (borders/outline/resize-grip via
      `::before`/box-shadow) — deferred here from W3c ([F6](forks.md#f6)); retire the `LayoutNode` stub
      ([D-1](deviations.md#d-1)). `Text` → DOM overlays; `Graphics`/cards → the engine's line/rect/circle
      helper or CSS ([F4](forks.md#f4)).

## W6 · Parity sweep + cutover

- [ ] Feature-for-feature vs `client/pixijs`: login, world render, zoom/LOD/pan, every panel, `/shadowcast`,
      `/overlayRT`, `/showRT`, `/es300`, all debug commands ([I-4](issues.md#i-4)). When it matches, make
      `client/webgl` the primary client and **retire `client/pixijs`** (archive out-of-repo).

## W7 · The payoff — `caster-lut` C5 on the owned engine

- [ ] Implement the GPU cast (instanced, VTF reading light/LUT/caster), the integer bitfield (`RGBA8UI`/
      `RGBA32UI`, real `uint`, retire float-mod), MRT where useful — all clean, no Pixi walls. Resume
      [`caster-lut`](../caster-lut/todo.md) C5 here.
