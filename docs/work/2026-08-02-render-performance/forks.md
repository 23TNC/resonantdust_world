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
