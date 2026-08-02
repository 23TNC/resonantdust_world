# Forks — one resolution

## F1 — the survivor's name: PARTITION LEVEL {#f1}

The user: "as we zoom in/out we re-size our tiles so that we do not have to re-allocate
textures. This was also called lod. I am uncertain what the new name should be."

Candidates weighed: **partition level** (chosen — `SquareCache` already speaks
"partition / re-partition" for exactly this operation, so the code names itself);
*zoom band* (accurate but implies the camera owns it, when the caches do); *grain /
density* (evocative, matches nothing existing). `partitionForZoom(zoom)` returns the
level, `slotPx = SQUARE >> level`, the lighting window carries `win.level`. One term,
already native to the file that owns the mechanism.

## F3 — the packing law: SQUARE POW2 STAYS {#f3}

The user (2026-08-02): a concurrent stream apparently broke away from pow2 packing —
"I am uncertain if that's good or bad… there are pros and cons to both. But we do need
to remain consistent."

**Pros of free-size**: crop-at-ingest could pack art at its exact aspect (a 2:3 tree
wastes ~44% of a square quadrant as margin); WebGL2/ES 3.0 supports NPOT textures
FULLY, including mipmaps — the old hardware reason for pow2 is gone.

**Pros of pow2-square** (chosen): every downstream consumer assumes it — the def
grid's 16-px addressing in the pool, the co-pack's `2N` quadrant model (one frame +
a fixed quadrant offset resolves any map), the SEED `ppu` lane's uniform scale, and
the mip chain halving cleanly to 1×1 with no odd-dimension rounding drift between the
four co-packed maps (a normal sampled one half-texel off its albedo is the exact class
of registration bug the subframe-ingest stream exists to kill). Free-size buys margin
bytes; pow2 buys every addressing invariant the format is built on.

**The law**: every packed frame and every co-pack page is a SQUARE POW2 ≥ 16. Enforced
at ingest as a REJECT, not a warn — consistency was the user's actual requirement, and
a warn is how the drift happened. If atlas pressure ever makes the margin bytes matter,
that future stream relitigates WITH the pool counter's numbers, against this fork.

## F4 — TWO atlases: graphics (filtered) and data (exact) {#f4}

The user, mid-P3 (verbatim): "you will very likely need to create two separate atlas.
We would have one graphics atlas for things like albedo and normal, but also a data
atlas for surface and likely layers... so that we can properly index into the data and
acquire data that hasn't been fiddled with."

Correct, and it caught a corruption my single-atlas mip plan would have shipped: a mip
chain + trilinear MIN on the whole co-packed page FILTERS the surface silhouette
(`silhouetteHit` thresholds `surface.b`) and the layer weights — data consumers must
read bytes nobody blended.

**The split that keeps every address intact**: ONE packer still allocates ONE `2N×2N`
rect per stem, but the rect is written to TWO same-size page textures — the GRAPHICS
page takes the albedo TL + normal TR quadrants (mips + filtered sampling), the DATA
page takes the SURFACE quadrant alone (NEAREST, no mips, exact). The NORMAL is classed
GRAPHICS by judgement (the user deferred it): its sole consumer — the bake —
renormalizes after decode, so filtering-then-renormalizing is the standard treatment,
and an UNfiltered normal is the worse choice under minification (speckle normals
shimmer in N·L). LAYERS is GRAPHICS by the user's follow-up call, and provably so: the
reconstruction is LINEAR in the weights (`out = residual + Σ wᵢ·tintᵢ`, tints constant
per channel), so linear filtering commutes with it exactly — filtered weights produce
precisely the filtered final colour. Surface is thresholded (`silhouetteHit` at
`surface.b > 0.35`) — the one read that must never see a blended byte. Frame
coordinates are identical on both pages, so the def format, the quadrant offsets, and
every shader's addressing stay byte-for-byte the same — only which TEXTURE a frame's
`source` points at changes, and `resolve()` hands each map its correct page. The
silhouette consumers (`uSurfaceAtlas`) bind the DATA page; the bake samples albedo and
normal from the GRAPHICS page. Cost: each page's unused half — accepted; the ladder
deletion bought back more than this spends, and the pool counter watches it.

"Always pass the maximum texture size" = each stem's largest served master (today
`BASE_LOD_PX = SQUARE = 128` for things; grid stems serve their atlas master whole).
The resolver asks for the stem's max from the manifest rather than hard-coding 128, so
a future art-resolution bump is a content change, not a client change — the same
posture as the SEED ppu lane (data-driven atlas scale, no hardcoded ×8).
