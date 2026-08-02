# Forks — render performance

_Decisions taken, with the reasoning. Cited by items and commits so a choice is never re-litigated
from memory._

## F1 — Flat geometry is the RESTING STATE, not a fallback {#f1}

> "We will load using flat geometry until we get the real texture." — user, 2026-08-02

**Chosen: geo first, always, and art upgrades into it.**

The distinction matters for how the code reads. A *fallback* is what you reach for when something
fails, and it invites "wait a bit longer, maybe it arrives" — which is precisely the barrier in
`ensureCoPack`. A *resting state* is where every stem starts unconditionally, with art as an
asynchronous improvement that may never come.

The renderer already implements the resting state (`Viewport.channels` returns `solid(...)` whenever
a frame is missing or geo). Nothing needs building; the barrier in front of it needs removing.

**Test it by denying the network.** If the world does not draw flat geometry with fetches blocked,
the resting state is not real and no amount of async fixes it.

## F2 — A placeholder tier is NOT the LOD ladder {#f2}

**Chosen: one small tier, fetched purely to have something on screen — and it never touches
geometry or zoom.**

The ladder was deleted deliberately, and it had a specific cost beyond complexity: the runtime opaque
bbox was computed from whichever tier decoded first, so the same art measured differently between
sessions ([subframe-ingest I7](../2026-08-02-subframe-ingest/issues.md#i7)). Placement registered
against a number that moves with streaming order is the bug that whole stream exists to end.

The placeholder avoids that by construction because it is **never consulted for anything but
pixels**:

| | LOD ladder (deleted) | placeholder (this stream) |
|---|---|---|
| chosen by | zoom / target px | nothing — always the same small size |
| feeds geometry | yes (bbox, `ppu`) | **never** |
| lifetime | the drawing tier | until the master lands, then dropped |

If an item here starts consulting zoom to pick a size, it has become the ladder and is wrong.

## F3 — Decode moves off-thread only if MEASURED to matter {#f3}

**Chosen: measure first, and the item says so.**

`createImageBitmap` on four full-size masters per stem is a plausible main-thread stall, and moving
it to a worker is a known pattern (`ImageBitmap` is transferable). It is also a real cost: a worker,
a transfer protocol, and a split where the atlas upload must stay on the GL thread regardless.

The stream's first day produced a wrong conclusion by reasoning about performance instead of
measuring it ([I1](issues.md#i1)). So P2 measures the decode share before P2 decides. If decode is a
small fraction of load latency, the bounded queue is enough and the worker is not written.

## F4 — The placeholder is DRAWN from the DSL, not fetched {#f4}

> "We don't need 32px placeholders. Instead we will draw … our DSL has an outline and tints … this
> should get us actual game assets in lieu of proper textures." — user, 2026-08-02

**Chosen: synthesize the co-pack on the GPU from data already in hand. [F2](#f2)'s fetched
placeholder tier is superseded and its P3 items are replaced.**

This is strictly better than a small fetched tier: it costs **zero network**, is available on the
first frame, and it cannot reintroduce
[subframe-ingest I7](../2026-08-02-subframe-ingest/issues.md#i7)'s instability because there is no
second tier for a bbox to be derived from.

**It works because of the art model.** `albedo-outlines-by-design`: an albedo master is a near-black
line drawing, and colour comes from `layers × tint`. So a drawn outline plus the authored tints is
not a crude stand-in for the asset — it is the same construction the real asset uses, at lower
fidelity.

All four quadrants synthesize from the same shape:

| map | drawn as | consumed by |
|---|---|---|
| **albedo** | the outline stroke, near-black — the master's own convention | the blit's residual |
| **surface** | the shape FILLED in B (coverage), G = 1 (no AO) | coverage, `silhouetteHit`, the receiver map |
| **normal** | flat `#8080FF` = `(0,0,1)` | the light pass, unchanged |
| **layers** | the shape filled in R | `packChannels` → the SAME authored tints as the real art |

**The consequence worth naming: surface.B is coverage, so lighting works on placeholders.** Shadows,
the receiver map and `silhouetteHit` all read that lane — so the world is fully lit, shadowed and
selectable before a single texture has downloaded, rather than being flat-lit until art lands.

**The shape source, which must be pinned before writing this** ([I6](issues.md#i6)): the authored
SUBFRAME gives the art's true rect, exactly and before any fetch. That yields an outlined, tinted
box at the art's real proportions — not a silhouette. Whether that is enough, or whether a coarse
per-kind silhouette should be authored too, is the open question; the rect alone is already a large
improvement over a flat geo box and needs nothing new.

**Swap is unchanged**: when the real co-pack lands it replaces the synthesized frame through the
existing pack path. The placeholder is never consulted for geometry ([F2](#f2)'s rule survives).

## F5 — The drawn placeholder uses TINTS first, polygons only if needed — and never at boot {#f5}

Resolves [I6](issues.md#i6) with a third answer: the shape is already generated and shipped as
`meta.json`'s `outline` — normalized image-space contours with holes and an earcut triangulation,
plus `color`, `bbox` and `channel_tints`. Not the subframe rect, and not something to author.

**Verified usable**: the contours are in normalized frame space (`0..1`), NOT projected, despite
`bin/lib/meta.py`'s "shadow-cast silhouette … projected" phrasing. So they describe the sprite's own
outline and are correct for albedo and coverage.

**Measured, which decides the staging:**

| | bytes |
|---|---|
| `outline`, median | 5,840 |
| `outline`, max | 28,017 |
| `outline`, all 80 stems | **531 KiB** |
| `channel_tints` + `bbox` + `color` | ~50/stem, **~4 KiB total** |

Neither reaches the client today: `ManifestEntry` carries `hash`/`maxSize`/`lods`/`maps`/`grid`/
`pad`/`span` only, though the edge reads meta.json already.

**Chosen:**

1. **Ship tints + bbox + outline colour in the manifest.** ~4 KiB corpus-wide, no measurable cost,
   no new render path — a solid quad in the real fill colour at the real proportions with the real
   outline colour as a border.
2. **Polygons only if flat shapes read badly**, and then in a **separate lazily-fetched bundle**,
   rasterized once into the atlas quadrant so nothing downstream changes.

**The failure mode this avoids — and it would be a bad one.** Putting 531 KiB of polygons in the
boot manifest bloats the file that GATES EVERY TEXTURE FETCH. That trades first-paint for total-load
and would plausibly be net negative: delaying the thing that unblocks all loading, in order to make
loading feel faster. If an item here starts adding polygons to the manifest, it has inverted the
stream's own goal.

**Two cautions carried forward:**

- **`outline` is effectively dead data.** The shadow system silhouettes via
  `silhouetteHit` → `texelFetch(uSurfaceAtlas)`, not polygons; the client's other "outline" symbols
  are the SELECTION overlay, unrelated. So nothing currently consumes it and nothing keeps it
  correct — a re-master could drift it silently.
- **26 of 106 stems have no outline at all**, so the flat-quad path is the floor, not an optional
  first step.

## F6 — 32 px PREVIEWS, not drawn placeholders — SUPERSEDES [F4](#f4) and [F5](#f5) {#f6}

> "It doesn't matter what is implemented today. We have not finished the game. We will have MANY more
> objects in the future we will need to handle … we need to generate 32px preview assets because that
> would be more reliable than drawing outlines from meta.json as we would need to load 5.8KB anyway
> so… may as well avoid manual drawing which could break us." — user, 2026-08-02

**Chosen: fetch a small REAL preview. The drawn placeholder is abandoned.**

F4 and F5 optimised for the corpus as it stands today — 106 stems, 80 with outlines. That is the
wrong thing to optimise for: the corpus grows, and a synthesized placeholder is a second rendering
of every asset that has to keep matching the first one forever. Real art at low resolution cannot
disagree with itself.

**The size argument, measured, and it is not close.** All four maps at 32 px, PNG-optimised:

| stem | albedo | normal | surface | layers | total |
|---|---|---|---|---|---|
| conifer | 113 | 753 | 900 | 443 | **2,209 B** |
| flora | 284 | 1,726 | 1,463 | 835 | **4,308 B** |

against **5,840 B median** for a single stem's `outline` polygons. The preview is *smaller than the
data the drawn version would have needed*, and it is the actual asset rather than an approximation
of it.

**And the serving already exists.** `server/edge/src/textures.rs`: *"sizes the client needs, so a
re-mastered asset needs no offline pyramid"* — the edge derives a requested size from the master and
caches the derivation. The cache is empty today only because nothing asks. So this needs no art
pipeline work and no offline generation; the client stopped requesting small sizes when the ladder
was deleted, and that request is what comes back.

**Still not the LOD ladder** ([F2](#f2)'s fence holds, and it is the reason this is safe): the
preview is one fixed small size, never chosen by zoom, never consulted for geometry, dropped when
the master lands. What made the ladder harmful was that the drawing tier varied and the bbox was
derived from whichever tier decoded first
([subframe-ingest I7](../2026-08-02-subframe-ingest/issues.md#i7)). A fixed preview that feeds only
pixels reintroduces none of that.

**What dies with F4/F5:** the synthesized co-pack, the polygon rasteriser, the manifest plumbing for
`outline`/`channel_tints`, and [I6](issues.md#i6)'s shape question — all moot. `meta.json`'s
`outline` stays dead data; if it is genuinely unused it should be deleted rather than half-revived.
