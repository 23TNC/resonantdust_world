# Primitive graph — completed

_Done + verified. Chronological append; items move here from [`todo.md`](todo.md)._

## P0 — Layouts landed in VARIABLES (2026-07-25)
[`VARIABLES.md`](../../VARIABLES.md) now carries the primitive-graph design as authoritative truth —
code conforms to it from here, not the reverse. Landed:

- **Set table** — `definition_data` (0) / `prim_data` (1) / `light_data` (2) / `light_presence_lo` (3) /
  `billboard_presence` (4, renamed from `caster_buckets`) / `light_presence_hi` (5) / **`billboard_data`
  (6, NEW)**. Set 0 doubles as a prim's "no data carried" sentinel; **in-set id 0 is the global
  sentinel** so commands pad with zeros (every allocator becomes 1-based).
- **`prim_data`** — the composition node: ≤4 carried pieces (`set_a..d` + `id_a..d`), `child` bit
  reinterpreting RED as `parent_id | tile_offset | unit_offset` (root keeps `region|zone|tile|unit`),
  downward-forcing inheritance, one object per layer, **bias-8 signed** offsets.
- **`billboard_data` / `light_data`** — RED = `parent_id` + the **CPU-resolved** position; GREEN keeps
  the **authored** offsets. Light props inline (no light-def band); `emitter_radius` retained.
- **`resolved_zone` on `light_data` only** — `light_presence` is a **reach** relation, so a light sits up
  to `reach` tiles from the fragment reading it and `resolved_tile` pins it only mod 16 tiles (ambiguous
  past ±8; `LIGHT_REACH` = 12). `billboard_presence` is a **containment** relation, so a billboard needs
  no zone. Both documented inline. ([I13](issues.md#i13) — flagged for the user, applied pending reply.)
- **`definition_data`** — `+ u4 type`, `+ inherit_rotation` (per-**kind**, with a `prim_data` override,
  inheriting **one step**), `−` the self-address.
- **Rotation as a reconciliation signal** — the shader always uses the *definition's* rotation; a
  mismatch prompts a CPU definition swap. Written down because it is not inferable from the bits.
- **Presence** — the in-set id leaves the px → **8 slots/tile**: 16 lights + 8 billboards (was 14/7);
  empty standardises on `0`.
- **Command format v3** — fixed **8-px commands** (`R: opcode|set|id₀`, `G/B/A: id₁..id₆`, `px1–7` =
  payloads) = **56 writes/row**; the opcode defines the command so future 8-px operations get their own;
  the trivial index map lets the scatter vertex drop its per-set count scan.

Commit `d0b21e9`. `rd docs-check` green (235 files).

## P1 — Command buffer v3 (transport only) (2026-07-25)
Swapped the transport **without touching a single record layout**, so the change is behaviour-preserving
by construction and independently provable.

- **`SCATTER_VERT` rewritten**: the 16-iteration per-set count scan is **gone**. A point is now
  `p → command p/7, slot p%7`, header at `uCmdBase + cmd·8`, payload at `base + 1 + slot`, target
  `(set << 16) | id[slot]` — read straight from the command header instead of the payload's self-address.
- **`flush()` rewritten**: buckets dirty texels by set, cuts each set into chunks of ≤7, emits fixed
  **8-px commands** (`R = u8 operation | u5 set | u3 count | u16 id₀`, `G/B/A = id₁..id₆`, then 7
  payload px). Batches fill the buffer from the rotating row cursor; the draw issues `batch·7` points and
  the vertex sends `slot ≥ count` off-clip.
- **No sentinel, nothing burned** — `id = 0` stays writable, which the tile-keyed sets require.
- Constants: `CMD_PX`/`IDS_PER_CMD`/`CMDS_PER_ROW`/`MAX_CMDS`/`OP_WRITE_DATA` replace `MAX_PER_SET`;
  the scatter geometry's point buffer sized to the whole buffer (512 commands × 7).

**Verified**: `tsc --noEmit` clean; fresh load renders the scene correctly (trees, soft shadows,
lighting); data confirmed flowing through the new path — 525 billboards live, 3 lights, last flush → set
2 (the orbiting light). Records still self-address in the mirror; the scatter simply ignores it now, and
P2 removes it. Gotcha hit + recorded: [I15](issues.md#i15) (HMR false failure).

## P2a — `definition_data` on the v3 layout (2026-07-25)
Both offsets move into RED, the u16 self-address is dropped, and `layer`/`rotation`/`inherit_rotation`/
`type` are reserved-as-0 until the DSL supplies them (P5). One writer + three decoders
(`receiverCover`/`casterCover`/`billboardNormal`) changed in lockstep. Verified on a fresh load —
pixel-matches the reference. Commit `f83d084`.

## P2b — Presence at 8 slots, 16 lights/tile, `shadow-cold` on u7|u1 (2026-07-25)
The self-address leaves the tile-keyed px, which is what buys back the 8th slot:

- **`tileSlot`** (GLSL, 2 copies) + **`writeTileSet`** (TS) → **8 slots/px**: `R = s0|s1`, `G = s2|s3`,
  `B = s4|s5`, `A = s6|s7`. Presence `_hi` offset 7 → 8; `PRES_SLOTS` 14 → **16**; caster buckets
  7 → **8**; every light loop `slot < 14` → `< 16`; every bucket loop `c < 7` → `c < 8`.
- **`shadow-cold` repacked to u7 | u1 per slot** (user's call — [I16](issues.md#i16)): 16 × 8 = 128 bits
  exactly, so each slot's byte carries its own on-billboard flag and the separate A-lane flag field is
  retired. Decode is `float(b8 >> 1) / 127.0` — the stored `<< 1` **is** the ×2 restore, so full range
  costs nothing.
- Bug found + fixed during verification: a desynced slot-stride literal ([I17](issues.md#i17)).

**Verified** on a fresh load: relief and shadows restored, matches the reference. `tsc` clean.

## P2c — `light_data` on the v3 layout (2026-07-25)
- **Writer**: `R = parent_id | resolved_tile | resolved_unit`, `G = layer|rotation|hot|cast|z_offset|
  tile_offset|unit_offset` (authored offsets at the **bias-8 zero**, `0x88`), `B = rgb|intensity`,
  `A = reach | emitter_radius | resolved_zone`. Today's debug lights have no carrier, so `parent_id = 0`
  and the resolved position is simply the light's own absolute position — P3 replaces that with the real
  resolve walk.
- **New shared GLSL `resolvedPos(zone, tile, unit, ref)`**: a leaf stores zone|tile|unit (period 256
  tiles), and the region is recovered by taking the congruent representative **nearest the reading
  point** — exact while the separation is under half a period (128 tiles), which any light reach
  satisfies. This is the mechanism the `resolved_zone` argument was about, now implemented.
- Both light decoders (`LIGHT_FRAG`, `GATHER_FRAG`) moved to the new field positions.
- Two stale reads found during verification ([I18](issues.md#i18)).

**Verified** on a fresh load: relief, soft penumbra and shadow direction all correct. `tsc` clean.

## P2d/P2e — `billboard_data` leaf (set 6) + `prim_data` node (set 1) (2026-07-25)
The composition structure now exists in the data texture, in its **degenerate one-child form**: every
placed `Primitive` becomes a ROOT prim carrying exactly one billboard.

- **`billboardDataFor` rewritten** to allocate/patch BOTH records from one index (they are 1:1 here):
  `prim_data[idx]` = absolute `region|zone|tile|unit`, `child = 0`, `set_a = 6`, `id_a = idx`;
  `billboard_data[idx]` = `parent_id = idx`, resolved `tile|unit`, `definition_id`, authored offsets at
  the bias-8 zero. New `writeRecord()` compare-write helper — an unchanged billboard still emits no
  command, and a MOVED one now updates its position (the old code wrote position once at alloc).
- **GLSL**: `BILLBOARD_BASE` moves to set 6 (the leaf) and `PRIM_BASE` names set 1; the four decoders
  (`casterCover`, `receiverCover`, `billboardNormal`, `casterOne`) read position via the new
  **`resolvedTilePos`** (16-tile period) and take `definition_id`/`rotation` from the leaf's new lanes.
  Dead `casterRowOf` deleted.
- **The resolve reference is threaded correctly** (the finding from the plan): `casterOne`/`casterCover`
  take a `ref` and the walk passes **the visited bucket tile** — `(vec2(o) + 0.5) * UPT` in the corridor
  branch, `lc + (dx,dy)` in the brute branch — never the sample point, which can be many tiles away.
  `receiverCover` gets its bucket tile; `billboardNormal` uses `P` (the billboard is drawn there).

**Verified** on a fresh load: scene renders with relief + soft shadows, and the records were read back
from the mirror — prim #3 has `child=0, set_a=6, id_a=3`; leaf #3 has `parent_id=3`, resolved tile/unit,
`def=34`, offsets `0x88/0x88`. The link is intact in both directions. Two compile breaks hit on the way
([I19](issues.md#i19) backtick; three missed `receiverCover` call sites in the 4-corner straddle test).

## P2f — Naming reconciled to VARIABLES + the stride given a constant (2026-07-25)
- `BILLBOARD_DEF_BASE` → **`DEF_BASE`** (the def band serves more than billboards — it carries a
  `u4 type`), `CASTER_BASE` → **`BILLBOARD_PRESENCE_BASE`**, `writeCasters` →
  **`writeBillboardPresence`** — code now matches the authoritative band names ([I8](issues.md#i8)).
- **[I17](issues.md#i17) closed**: `TILE_SLOTS = 8` (slots in one tile-keyed px) now derives both
  `PRES_SLOTS` (× lo+hi) and `BILLBOARD_SLOTS`, and every fill / allocation / write / GLSL bucket loop
  reads from it instead of a bare literal. This is the exact desync that built buckets from misaligned
  memory during P2b; it can no longer happen by re-typing a number in one place.
- First change made with **Edit/Write instead of a bash heredoc**, so the project's PostToolUse guards
  actually ran ([I20](issues.md#i20)).

**Verified** on a fresh load: renders correctly; `tsc` clean; GLSL backtick guard clean on all four
shader-bearing files.

## P3a — Lights are CARRIED; prim ids get their own space; subtree free (2026-07-25)
The graph now holds **both** presentation types, and a carried leaf can no longer outlive its carrier.

- **`prim_data` ids are their own space** (`primNext`/`primFreeList`/`allocPrim`). The P2e shortcut —
  one index serving as prim id *and* billboard leaf id — breaks the moment lights also need carriers,
  since both would allocate from the same counter and collide. Billboards keep `billboardIndex` (leaf)
  plus a new `primOfBillboard`; lights get `primOfLight`.
- **A light is carried, never placed** ([F1](forks.md#f1) realised): each light allocates a ROOT prim
  holding the absolute position with `set_a = 2` (`light_data`) + `id_a = k`, and the leaf's
  `parent_id` points back at it. Offsets stay at the bias-8 zero until real nesting places a light
  relative to its carrier (a torch's flame above the sprite's base).
- **Subtree lifetime** ([I9](issues.md#i9)): freeing a billboard now also zeroes and releases its
  carrier prim; a removed light (`k >= n`) releases its prim and clears the node. No stale carrier stays
  reachable, and prim ids return to the free-list.

**Verified** on a fresh load — read back from the mirror: light 0 → prim 1 (`set_a=2, id_a=0`), light 1
→ prim 2 (`set_a=2, id_a=1`), billboard 3 → prim 6 (`set_a=6, id_a=3`); all prim ids distinct across the
two kinds, links round-trip both directions, scene renders correctly. `tsc` clean.

## P3b — The resolve walk + inheritance (2026-07-25)
The mechanism the whole model rests on now exists, and both leaf writers go through it.

- **`resolveCarried(prim, tileOff, unitOff, hot, cast)`** climbs `carrier → … → root`, summing
  **bias-8 signed** offsets and folding the inherited flags: `hot_cold` by **OR** (any hot ancestor
  forces the subtree hot — "topmost hot wins") and `cast_shadows` by **AND** (any non-casting ancestor
  silences it — "topmost !cast wins"). The offset sum is applied in world px and re-encoded, so
  unit→tile→zone carries come free. Bounded by **`MAX_PRIM_DEPTH = 8`** ([I12](issues.md#i12)) — a
  malformed cycle costs a fixed walk, never a hang. This is the **single resolve authority**: the CPU
  stamps what the GPU reads, and the GPU never walks the graph in its hot loop.
- Both `billboardDataFor` and `buildLights` now derive the leaf's resolved position + effective flags
  from the walk instead of assuming absolute placement. A no-op while every carrier is a root with
  zero offsets — and correct the moment either stops being true, which is what nesting needs.
- Ordering fix: the light's carrier is written **before** the resolve reads it (it used to be written
  after the leaf, which would have resolved against an empty record on the first frame).

**Bug caught by mirror readback** (a screenshot could not have shown it): billboard leaves came back
`cast = 0`. Their carrier prim never set `cast_shadows`, and the AND-down-the-chain silenced them.
Harmless *today* only because the gather reads that flag off the **light** record — it would have bitten
the moment the billboard's own flag was consumed. Fixed at the carrier.

**Verified** on a fresh load: the resolve reproduces each light's own absolute position exactly
(`tile 101, unit 136` = independently recomputed expectation); billboards `cast=1, hot=0`; lights all
`cast=1` with **only the dynamic green light `hot=1`** — inheritance flowing carrier→leaf. Renders
correctly; `tsc` clean.

## P3c — Real nesting: child prims, offset accumulation, height summing (2026-07-25)
- **`childPrimUnder(parent, dTileX, dTileY, dUnitX, dUnitY, set, carriedId, opts)`** authors a CHILD
  prim — `child = 1` makes RED's top half the `parent_id`, and placement is by **bias-8 signed** tile +
  unit offsets, so a piece can sit in any direction from its carrier. This is what P5 authoring calls to
  hang a hand off a pawn or a torch off a hand.
- **`z` now accumulates down the chain** and **saturates at u8** ([I9](issues.md#i9)) — heights add, so a
  torch's flame sits above the hand that holds it. Clamping (not wrapping) matters: a clamped light sits
  too low, a wrapped one teleports to the ground. Fixed a double-count found while wiring it: the light
  wrote its height into *both* the carrier and the leaf; the carrier now holds the **authored** height
  and the leaf the **resolved** sum, the same split position already uses.
- **`debugResolveChain()`** self-test builds `root → child → leaf` with known offsets (including
  negative ones), resolves, compares against hand-computed world arithmetic, then frees its scratch
  prims — safe to run against the live scene.

**Verified** on a fresh load: resolved `(2552, 1932)` == hand-computed `(2552, 1932)` — root
`40·64+3·4`, child `−2` tiles + leaf `+1` = `−64`, child `+5` units + leaf `+6` = `+44`. The root was
made hot + non-casting and the leaf came back `hot = true` (OR) and `cast = false` (AND), so both
inheritance directions fold across two levels. Light heights read `40` (not `80`) — no double-count.

## P3d — Reachability free replaces the flat sweep ([I9](issues.md#i9)) (2026-07-25)
`freeSubtree(prim)` releases a prim **and everything it carries, depth-first**: it walks `set_a..d`,
recurses into carried PRIMS, and returns carried billboards/lights to their own lists.
`MAX_PRIM_DEPTH`-bounded and cycle-safe via a `seen` set, so malformed data costs a bounded walk.

**Why the old sweep had to go:** "was this billboard seen this frame" only ever knew about *top-level*
billboards. The moment a prim can carry another prim, dropping the root strands its whole subtree — leaf
records the buckets no longer reference and ids the free-list never reclaims. Invisible in rendering,
and it only bites after enough churn.

**Verified** by `debugFreeSubtree()`, which builds `root{ child{ billboard }, light }` and frees the
root: the **nested billboard** (reachable only *through* the child prim — the case the flat sweep could
not see), the carried light, and both prims all come back zeroed, with **2 prims + 1 billboard**
reclaimed. Live scene unaffected; renders correctly.

## P4a — Two named dirty entry points; the force-alls retired (2026-07-25)
The user's original design ([light-prims](../2026-07-24-light-prims/README.md) P3), now built on the graph.

- **`markPrimDirty(x,y,w,h)`** is the shared cascade — dirty the prim's tiles, then every light reaching
  them, then those lights' cast regions. **Contract:** the extent must cover the prim *and everything it
  carries*, since moving a carrier moves its subtree. **`markBillboardDirty`** delegates to it;
  **`markLightDirty(L, from?)`** is the light-side door.
- **Class is derived at the door, not threaded by callers.** `markLightDirty` reads `L.dynamic` itself,
  so the three call sites that each passed their own `cls` (and could drift out of step with the light's
  actual class) no longer decide it.
- **Caster-removal force-all retired**: each resident billboard records its tight box (`lastBox`) and a
  departing one queues that box before its record goes. A removed caster has already left `standing`, so
  its extent is unrecoverable afterwards — which is exactly why removal used to recompute every tile.
- **Light-removal force-all retired** the same way via `lastLightBox`.
- **`rebakeAll()`** now names the one legitimate force-all — a GLOBAL constant moved (`__tilt`,
  `__pitchnormal`, `__worldlight`, …) so every baked texel is wrong. All 14 hand-set
  `forceColdDirty = forceHotDirty = true` sites route through it, so "recompute everything" can no
  longer be reached for as a shrug when the scoped path is inconvenient ([issues.md#i4](issues.md#i4)).

**Verified** on a fresh load at `?focus=100,50&zoom=0.25` (window 7440 tiles): steady state dirties
**216/7440 = 3.1%** — one moving light's cast region, not the world. Removing a light live kept it
scoped (**840/7440**) instead of force-alling. An earlier 240/240 reading at `zoom=2` was a small-window
artifact: the window was 20×12 and a single light's reach box is ~25×25, so it legitimately covered
everything — worth knowing before reading that number as a regression.

## P4b — The hand-set dirty flags derive from the entry points (2026-07-25)
`coldDirty` and `lightsVer` are no longer set by hand anywhere. They hang off the two places a change
can actually enter:
- **`markLightDirty`** flags both — a light change may alter its record (`coldDirty`) and the per-tile
  light lists (`lightsVer`). Previously each call site set whichever it remembered; forgetting one is a
  silent stale bake, which is precisely the failure mode a single door removes.
- **`rebakeAll()`** flags both plus the force-all, since a global constant change invalidates everything.
- Deleted the hand-set pairs in `seed()`, `__manylights`, `setEmitter`, and the per-frame `if (moved)`
  block in `tick` — every mover there already passes through `markLightDirty`.

**Verified** at `?focus=100,50&zoom=0.25` (7440-tile window, 1632 billboards, **120.7 fps**): steady
state dirties **2838/7440** (the moving light's region), and `setEmitter()` — a property on *every*
light, with no region to scope from — spikes to **7440 for exactly two frames** (the cold then hot
pass) before returning to scoped. The resolve-chain self-test still passes.

**Verification note:** the first attempt sampled a single frame after `setEmitter` and read `2838`,
which looked like the rebake had failed. It had already happened. A one-frame sample of a
multi-frame effect is not evidence — sample a window and take the peak.

## P5a — A placed object carries a light: the world lit with `lights.length === 0` (2026-07-25)
The two-presentation case now runs end to end through the real placement path.

- **`Primitive.light`** (`PrimitiveLight`: colour/intensity/reach/emitterRadius/height/cast/hot) — the
  light presentation of a placed object. Absent ⇒ every record byte-identical to before.
- **`carriedLightFor(billboard, light)`** writes the light leaf under the **same carrier prim** as the
  billboard (`set_a` = billboard, `set_b` = light), ensuring the carrier itself if the billboard path
  did not create one. Carried-light ids start at `N_LIGHTS` so they can never collide with the debug
  array's slots.
- **`buildPresence` now culls the debug array PLUS every carried light**, so a torch lights the world
  exactly as a seeded light does. When `this.lights` goes, the first list simply becomes empty.
- **`__torch(id?)`** turns an already-placed billboard into a torch — the stand-in for content until
  the DSL supplies a torch kind.

**Verified**: with `__gather.lights.length === 0`, a single carried light is listed by **131 of 240**
in-window tiles and the scene renders lit by it, trees casting shadows away from it. Three blockers
found and fixed on the way ([I25](issues.md#i25)) — the tick guard that made deleting `this.lights`
impossible, the presence/caster ordering, and the missing carrier for non-caster prims.
