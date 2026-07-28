# Forks — lighting feel

_Decision points, options, which we chose and why._

## F1 — decay map resolution + format {#f1}

- (a) FINE (lightmap res, 4096×2048): sharp particles, but a full-map decay is ~134 MB of
  read+write bandwidth per frame — the standing-tax class of cost just eliminated.
- (b) COARSE (`TEXTILE_UNIT`, 512×256) RGBA16F, bilinear upsample in the blit.
- (c) Coarse RGBA8: half the memory again, but dim glows quantise visibly (the map holds values
  well below one light's peak for most of a particle's life) and decay multiplies compound the
  banding.

**Chosen: (b).** Glow has no edges to keep sharp; 131 k texels make the decay draw ~free; f16 has
the headroom for stacked splats and smooth decay tails. blendability of RGBA16F (EXT_color_buffer_float
renderability is already relied on for the RGBA32F accumulators).

## F2 — how the fade runs {#f2}

- (a) Ping-pong: read A, write B = k·A, swap. Two RTs, a swap, a full copy's bandwidth.
- (b) Compute decay lazily in the blit from a per-texel timestamp. No decay pass at all, but
  splats must then MERGE with timestamped state (read-modify-write at emit) and the map stops
  being a plain accumulator.
- (c) **In-place blend**: `blendFunc(ZERO, CONSTANT_COLOR)` + `blendColor(k,k,k,1)` — any
  fullscreen quad computes `dst *= k` with no texture read, no second RT, no shader logic.

**Chosen: (c).** One trivial draw, one RT, splats stay dumb additive quads. `k = exp(−dt/τ)`.

## F3 — where the emissive map lives {#f3}

The co-pack frame has four quadrants (albedo TL / normal TR / surface BL / layers BR) — no free
quadrant. Options: (a) a lane of the LAYERS quadrant (it channel-packs tints; a spare channel may
exist per kind); (b) a separate emissive atlas page (own texture, resolver returns a second
frame); (c) fold emissive INTO albedo at bake (pre-lit pixels) — rejected outright: emissive must
survive the light multiply, that is its entire point.

**Unresolved — resolve at execution** when the P0 audit shows how many kinds need it. Lean: (a)
if a layers lane is genuinely spare (zero new memory), else (b) sized to emissive-carrying kinds
only (flames are few).

## F4 — particle shadowing {#f4}

- (a) None: splats ignore casters — flicker bleeds into tree shadows, visibly violating the
  zero-light feel.
- (b) Per-frame: the blit re-shadows the decay map — a per-texel-per-frame cost, the exact class
  this stream exists to avoid.
- (c) **Stamp at write**: the splat shader samples the parent light's coarse-shadow slot once at
  emit and multiplies the splat by `(1 − shadow)`.

**Chosen: (c).** One fetch per splat texel, once. The stamp is as-of-emit — acceptable for
sub-second particles; recorded as a known limit in the README.

## F5 — what emits particles {#f5}

- (a) A prim-graph particle leaf (content-authored emitters) — the eventual home, but it drags in
  record layout + VARIABLES work for a feature whose look is unproven.
- (b) **CPU emitter v1**: lights carrying a flicker flag emit jittered splats client-side.

**Chosen: (b), with (a) noted in the intent doc.** Prove the look first; the write-side API
(pos/radius/colour/intensity) is the same either way, so promotion later is additive.
