# Lighting visual correctness — grounded shadows + real normal maps — 2026-07-31

_Component: [`client/webgl`](../../components/client/webgl/). Successor to
[`2026-07-31-lighting-correctness`](../2026-07-31-lighting-correctness/README.md) (done). The
user's directive, verbatim: **"I want our normal maps to be properly implemented. I want
shadows to begin at the base of our prims, this is done by properly handling the minimum bbox
and anchor to align the texture to the bottom of the subframe. Once the world visually
displays correctly we will then focus on performance improvements."**_

**Sequencing is part of the directive**: this stream is VISUAL correctness only. The
steady-state/gating performance work (the ~10 ms → ~0.1 ms idle collapse the slot design
already enables) is the DECLARED SUCCESSOR, opened after the user signs off the visuals —
deliberately not here, so neither goal contaminates the other's acceptance.

## S1 — shadows begin at the BASE of the prim

The current record model treats the frame bottom as the ground and lets art float inside the
frame: heights run `[fu − subY − subH, fu − subY]`, and the card's world anchor is the FRAME
box's bottom-centre. Both halves disagree with what is DRAWN — masters are square canvases
with the subject letterboxed/centred, and the draw pins art per `sprite_anchor` — so a shadow
can detach from the feet or start mid-air.

The user's prescribed model (matching the old def-frame-anchors contract "y bottom-aligned —
shadows anchor at the base"): **the minimum bbox is bottom-aligned to the anchor**. Two
decoupled mappings fall out:

- **Height window**: the caster occupies `[0, subH]` measured from the anchor — the shadow
  BEGINS at the base, always. (`subY` stays what it is: the art's ATLAS address for
  sampling. Where the art sits in the frame and where it stands in the world are different
  questions; conflating them was the bug.)
- **World anchor**: the record's `C` must be the DRAWN art's opaque bottom — the feet — not
  the frame box's bottom. The reconciler derives it from the prim box + the bbox fractions
  (`p.y + (bb.fy + bb.fh) × p.height` for an unscaled frame box), per kind, verified.

The receiver height mapping and the silhouette fracY shift to the same `[0, subH]` window,
so lit-point elevation, coverage and casting all share one base line.

## S2 — normal maps, properly

The G-buffer already BAKES real normals: `normal-cold` / `normal-warm` composites carry the
art's normal maps, silhouette-keyed with flat-up fallback, INCLUDING the per-material RNM
detail fields and mover facings. Lighting never reads them — P5-of-correctness sampled the
raw def quadrant per receiver (no detail, no tile normals, ground hardwired flat-up).

Proper = **the slot pass samples the baked normal composites** (warm-over-cold by warm
coverage, the same rule the blit resolves pixels with): one mapping, art resolution, every
consumer of a normal (tiles with authored normal maps, the analytic wall normals, material
detail, mover facings) inherited for free. `receiverNormalAt` (the def-quadrant sampler)
retires with it. Light direction stays plan+height; the wrap floor and ambient×AO interplay
are re-tuned against captures.

## Acceptance model

Every phase ends ON SCREEN at a cold page load, both zooms, at the standing fixtures (the
torch pools at ~(100,51), the placed human at (104,54), the wolf, the user's walls). The
final phase is explicitly **the user's eyes** — this stream exists because the world does
not look right, so looking right is the exit criterion, with captures beside the strip's
before-images as the reference.

## Out of scope

Performance/steady-state gating (the successor stream — opened at this stream's close);
emissive + decay/flicker (awaiting the user's call, unchanged); the brightness decode fix +
torch reach 16 land HERE as P0 items (they were in-tree, uncommitted, when this stream was
written).
