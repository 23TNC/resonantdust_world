# Presence + buckets into the data texture (region-torus) — 2026-07-23

_Component: [`client/webgl`](../../components/client/) · `game/viewport/shadowGather.ts`,
`coldShadowData.ts`. Builds on [`2026-07-23-unified-data`](../2026-07-23-unified-data/README.md)
(the one data texture + command-buffer scatter). Phases in [`todo.md`](todo.md)._

Fold **light-presence** and **caster-buckets** into the unified data texture as two more sets, so
they ride the SAME scatter pass — killing the second-FBO problem their separate textures created.
Address them on a **fixed region-sized torus** (deterministic, thrash-resistant, pan/zoom moves
reads not writes), self-addressing like every other record.

## The load-bearing principle (do not re-forget)

**DATA holds PERSISTENT state only.** A map belongs in the data texture iff it survives across frames
and changes only on events (an object created/moved/destroyed, a zone streamed/evicted). It does
**not** belong if it's rebuilt every frame or produced on the GPU:

| in DATA (persistent) | NOT in DATA |
|---|---|
| defs, prims, lights (unified-data) | **`shadow_dirty`** — rebuilt per frame (owner-change + scoped) |
| **light-presence** (this stream) | **`shadow-cold` RT** — GPU-written render-target output |
| **caster-buckets** (this stream) | per-draw uniforms / debug toggles |

_(An earlier idea to fold `dirty` into DATA was **rejected** — it's transient; putting it in DATA
would mean re-scattering ephemeral state every frame, worse than the texSubImage it replaces. Kept
here as the cautionary record.)_

## The FBO problem this solves

A scatter draw targets ONE framebuffer, whose attachment size fixes the viewport + NDC divisor.
Presence as a separate `cols×rows` texture = a different pixel grid = a **second scatter FBO + pass**
(and its window-relative addressing needs constants + a dynamic divisor). Folding presence into the
1024² data texture makes it just more sets → ONE FBO, ONE pass, ONE program. The opcode selects
behaviour *within* a pass; the FBO selects the pass — so "presence" must not be a separate texture.

## Region-torus addressing (deterministic, no region bits)

The data set is **1024×64 = 65 536 = one region's tiles** (`REGION_DIM=16` zones × `ZONE_DIM=16`
tiles, squared). Address every tile by its **in-region** coordinates — `zone mod 16` + tile — with
**no region bits stored**, because the viewport window is ≤ one region, so two zones sharing a
residue (16 zones = 256 tiles apart) can never be visible together (proof: max visible tile-distance
< 256 ⟹ no mod-256 collision). The "park at a region corner" 4-region straddle stays unique
(zones 0,0 / 0,15 / 15,0 / 15,15 all distinct mod 16).

**Zone-strip fold** (keeps a zone's 256 tiles contiguous in one row — good for bulk zone ops):

```
world tile (wc, wr)
  in-region zone (zx, zy) = ((wc >> 4) & 15, (wr >> 4) & 15)     // ÷16 then mod 16
  in-zone tile   (tx, ty) = (wc & 15, wr & 15)
  row  = (zx >> 2) + zy * 4                       // 0..63  (4 zones/row × 64 rows = 256 zones)
  col  = (zx & 3) * 256 + ty * 16 + tx            // 0..1023
  slot = SET_BASE + row * 1024 + col              // bijective over the region
```

**Anchor-centered, closest-wins.** Zoom past a region and the materialized set clips to the 16×16
zone block **centred on the anchor** — "closest to anchor wins the slot" is just the residue-class
representative, i.e. the existing SquareCache toroidal owner-tracking at a FIXED region modulus. No
hard cap: further zoom-out degrades gracefully (loser tiles simply aren't materialised; at 65 536
tiles on screen each is ~4 px and the detail is invisible anyway). This makes `map-model.md`'s
shared TILE grid maximally rigid — one fixed modulus for prims, lights, presence, buckets — so the
toroidal-basis-mismatch bug class ([lighting-rebuild issues](../2026-07-22-lighting-rebuild/issues.md))
is impossible by construction.

## Set layout (functional requirement: two NEW sets)

Presence px is a full 128-bit texel today (8× u16 slots). Self-addressing spends ONE slot on the
**u16 in-set id** (R's high half, like every unified-data record) → **7 slots/tile** (7 lights /
7 casters), unifying presence into the 1px pure-payload command path (no 2px form, no opcode branch;
two more of the header's 16 u6 per-set counts go live — 10 sets stay reserved):

```
presence-light  set:  R  u16 id (16–31) | u16 slot0 (0–15)   G  slot1 | slot2   B  slot3 | slot4   A  slot5 | slot6
caster-buckets  set:  (identical shape — 7 prim indices/tile; 0xFFFF = empty)
```

_Set INDICES are arbitrary (addressing is by base constant), so keep unified-data's bases
(defs 0, prims 1, lights 2, constants 15) and add **presence-light = set 3, caster-buckets = set 4**
— minimal churn. The user's conceptual globals-first ordering (globals 0 … presence 4) is
equivalent and skipped._

## Eviction — two layers, separated by distance

- **Presence / buckets** clear on **region-window exit** (a tile's owner-zone leaves the anchor
  16×16 block — 8 zones from anchor).
- **Prims / lights** clear on **zone eviction** (deep — the object's zone is destroyed or drops from
  the subscription; ≫ 8 zones away, "the other side of the torus"). Retained until then.

The reach gap makes this self-safe with NO reach-lag machinery: reach is 12 tiles ≈ **0.75 zones**, so
a prim only affects tiles within ~1 zone; by the time its zone is evicted (≫ 8 zones) every tile it
could bucket into left the region long ago. A prim id is freed only when provably unreferenced.

**Thrash resistance is free**: the region-torus is a 16-zone-deep hysteresis band — a slot's owner
flips only when the anchor crosses a midpoint between same-residue representatives (16 zones apart),
so a jiggle re-writes at most a zone-column and re-thrashing costs 16 zones of travel.

**Allocation**: prims/lights need a **free-list** (freed u16 ids → a stack, reused on next alloc) so
the id space survives churn; the high-water mark stays bounded by peak concurrent objects
(subscribed-zones × per-zone, ≪ 65 536). Defrag is deferred ([`forks.md#f4`](forks.md#f4)).
