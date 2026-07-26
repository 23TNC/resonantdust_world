# TEXTILE_SLOT — forks

_Decisions taken, with the option not taken and why. Chronological append._

### F1 — size maps in TILES, not screen resolution (user, 2026-07-26) — ADOPTED
Every textile map becomes a fixed **24×16 slot** grid rather than a screen-derived buffer.

**Rejected: keep screen-derived sizing.** It is what the G-buffer does today
(`cw = ceil(screenW + RESERVE_CSS)`) and it is *correct* for the G-buffer, but it makes the budget a
function of the player's monitor and leaves the lightmap — which is world-derived — free to grow to
**176 MB** at zoom 0.25. A fixed tile grid fixes the budget, equalises gameplay across monitors, and caps
the art pipeline at 128px in one move.

**Cost, accepted knowingly:** the reference target becomes 2560×1536 (2K native), so a 4K player is
magnified ~1.5× and a 1080p player minified ~1.33×. Per-channel memory rises 14 → 24 MiB because the
standard is larger than this machine's 1862×853 panel. That is a purchased uniformity standard, not waste.

### F2 — continuous zoom with up to 2× magnification, not downsample-only (2026-07-26) — ADOPTED
Within a lod band the slot texture is magnified by 1→2×, snapping back to 1:1 at each boundary.

**Rejected: downsample-only.** Attractive ("never upsample"), but it does not follow from a fixed grid. The
display-side sampling ratio is `1/σ` and the `2^k` **cancels**, so lod does not affect sharpness at all
([I1](issues.md#i1)). A genuine 2:1 minifying band at every lod requires the visible grid to hold the
zoomed-out end — 40×24 visible slots, ~77 MiB per RGBA8 surface and ~308 MiB for the accumulator. 4× the
memory for sharpness that only appears at the bottom of each band.

**Rejected: discrete (snapping) zoom.** `σ = s` always, so the ratio is exactly `1/s` everywhere and no
magnification ever occurs — the cheapest option and it preserves "never upsample" end to end. Declined
because smooth zooming is wanted. Worth revisiting if the magnification reads badly.

**Note:** the bake stage is unaffected and remains exactly 1:1 — the art mip at lod k is `128/2^k` px into a
`128/2^k` texel footprint. "Never upsample" survives where the art is concerned; only the display magnifies.

### F3 — NEAREST is the universal rescale rule (2026-07-26) — ADOPTED
Replication on upscale, decimation on downscale. Never averaging.

Not a preference — it is independently **mandatory** for three unrelated maps, which is what makes one
shared reproject path possible:
- the shadow bitfield cannot be filtered at all (interpolating packed bits is meaningless —
  [shadows I-1](../shadows/issues.md), [shadow-world I-2](../shadow-world/issues.md))
- `zdepth` encodes a discrete `0x80 | baseRow`; averaging two neighbours yields a row belonging to neither
  prim, a silent z-order corruption
- the additive lightmap must stay exactly-representable to remain invertible
  ([primitive-graph F11b](../2026-07-25-primitive-graph/forks.md#f11b))

**Considered: averaging for the lightmap.** Higher quality on zoom-out and still exactly invertible if the
fixed-point grid is respected (values become multiples of 1/4 per level, exact in FP32). Declined for the
first pass because it costs 4× headroom per lod step (65,793 → ~257 lights at full brightness after four
levels) and forfeits the single shared rescale path. Revisit if decimation aliases visibly.

**Linear stays available** for albedo/normal/surface as an optional quality choice — they are sampled, not
accumulated or bit-packed.

### F4 — how aspect ratio is handled — RESOLVED 2026-07-26: size for the 5:3 reference, cover trades area
**Decision:** the grid is sized for the **5:3 reference (2560×1536)** and cover fit handles everything else.
No per-aspect grid, no letterboxing, no widest-aspect padding. Taken as the executable default so P1 is not
blocked; it is cheap to revisit because it changes only the slot count, not the model.

The fairness property this commits to: **nobody exceeds 20×12 visible slots, and off-aspect viewports see
LESS area, never more** — the inverse of the usual ultrawide advantage. A 21:9 player sees 20×8.37 slots
against the reference player's 20×12. That is a deliberate property, not a bug to fix later by letting
ultrawide see wider.
Cover fit already guarantees nobody exceeds 20×12 visible slots, and off-aspect viewports see *less* area,
not more — the inverse of the usual ultrawide advantage:

| viewport | s | visible slots |
|---|---|---|
| 2560×1536 (5:3, reference) | 1.0 | 20 × 12 = 240 |
| 2560×1440 (16:9) | 1.0 | 20 × 11.25 = 225 |
| 3440×1440 (21:9) | 1.344 | 20 × 8.37 = 167 |
| 1080×1920 (portrait) | 1.25 | 6.75 × 12 = 81 |

So the reference aspect is the most advantageous and everything else trades area away, with no black bars.
**Decide and record**: is that the intended fairness property, and what is the widest aspect the grid is
sized for? It sets the slot count, so it must land before P1.

### F5 — 24 is not a power of two (OPEN, low stakes)
The wrap modulus is the slot count: 16 masks, 24 does not. A 32×16 grid would make both axes bitmasks at
3072→4096 wide (24 → 32 MiB per RGBA8 surface, ~+90 MiB total). Not worth it — integer modulo is cheap on
modern hardware — but it should be a recorded decision rather than an accident.
