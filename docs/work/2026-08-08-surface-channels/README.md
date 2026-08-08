# Surface channels — settle R/G/B, then move the outline into R

**What** (user, 2026-08-08): the surface map's channel contract has drifted and nobody
can say what it is. We write `R: heightmap, G: AO, B: alpha/presence` — but B looks like
presence *and* we read it downstream as presence, while a Sonnet chat claims presence is
written to **alpha**, which we don't think we use at all. R (heightmap) is not used any
more to the best of our knowledge. And **G (AO) is not picking up the same scaling B is**,
which is very likely incorrect.

> "So first we want to resolve our G channel. After that is fixed, we are going to
> implement a new feature."

**The feature.** Our albedo retains a **black outline**. We also extract an outline so we
can procedurally draw stuff in game. The feature writes the **OUTER outline from albedo
into the Red channel**, then **removes the outer outline from albedo** — keeping the
**inner lines**, which are the texture's detail. The outline is thick and is included in
presence, so we also **remove presence under about half of the outer outline**, shrinking
presence a bit. That lets us **cheat during the shadow passes**, since we will be drawing
a thick black line on top anyway.

Every claim in that report is correct. The investigation (2026-08-08) also found *why*.

## What is actually true today

Two different things are both called "the surface map", and they do **not** share a
layout. That alone accounts for most of the confusion:

| | R | G | B | A |
|---|---|---|---|---|
| **leaf `surface.<dir>.<part>.png`** (`bin/art` `_surface_kind`) | **emissive mask** | AO (occlusion.png, else white) | diffuse alpha = coverage | **no alpha channel** |
| **G-buffer attachment 1** (`mrtBakeShader`) | **presence** (`tileDepth >= 0`) | relayed leaf G | relayed leaf B | **must stay 1.0** |

- **B is coverage/presence and always was** — the user's read is right. It is the *one*
  silhouette source: the bake's shared `discard` (`cov < 0.5`), the blit's output alpha,
  the warm-over-cold key, the selection outline's silhouette test, and `subframe.py`'s
  authoring threshold (0.35) all key off it.
- **Presence is NOT in alpha.** The leaf map ships with **no alpha channel at all** (data
  in a source alpha fights the client's upload-premultiply), and the composite's alpha is
  a **blend factor** that must stay 1.0 — writing data there once collapsed world coverage
  to nothing (lighting-feel P3). The "presence in alpha" claim traces to a real but
  **stale second writer**: `depth_ao.py --surface` writes `R=height, G=AO, B=0, A=silhouette`
  — a layout that contradicts `bin/art`'s and whose `B=0` **is** the invisible-sprite class
  the zero-coverage guard was added to catch ([I3](issues.md#i3)).
- **R is not the heightmap** — that lane was reclaimed for the emissive mask, and the
  emissive *consumer* was then deleted by lighting-strip. R is written, relayed into
  `zdepth.R`, and **read by nothing** ([I4](issues.md#i4)). It is free.
- **G is genuinely broken, and not only by scaling** ([I1](issues.md#i1), [I2](issues.md#i2)):
  `depth_ao.py`'s discovery still looks for the pre-fold flat filename `diffuse.png`, so
  under the folded leaf layout (`diffuse.e.0.png`) it finds **nothing** — `art ao` cannot
  run. Every `occlusion.*.png` on disk is a **pre-fold artifact** left at the master scale
  of its era, while the diffuse (and therefore B) has been re-normalised since. Measured:

  ```
  biome-thing/default/conifer/0/e.0   occ=156x256+53+0   cov=86x151+86+54    (1.8x)
  biome-thing/default/conifer/4/e.0   occ=154x256+51+0   cov=129x224+64+17   (1.2x)
  biome-thing/default/flora/2/e.0     occ=128x114+0+7    cov=106x98+11+16    (1.2x)
  pawn/animal/wolf/777/e.0            occ=124x62+2+33    cov=122x57+3+35     (~1.0x, halo only)
  ```

  And the coverage is thin: **175 of 216** surface maps carry a flat-white G — no AO at
  all ([I7](issues.md#i7)). AO is consumed (`ambient × ao`, `mix(0.75, 1, ao)` on the
  diffuse) whenever `uLit` is on, and it defaults on.

## The stance

- **Write the contract down first.** The channel layouts are cross-component variables
  (`dev/art` → `client/webgl`), so they belong in **`VARIABLES.md`**, stated once, with the
  leaf and the composite side by side because they differ. Every comment that restates a
  layout gets deleted or pointed at it — three of them are already lying.
- **One writer per map.** `depth_ao.py --surface` is **deleted**, not deprecated
  ([F2](forks.md#f2)). `bin/art`'s `_surface_kind` is the only assembler.
- **Fix G at the source, then guard it.** Teach `depth_ao.py` the folded leaf name,
  regenerate, and add a **geometry guard** to `_surface_kind` beside the existing
  zero-coverage guard: an AO whose opaque bbox disagrees with the coverage bbox beyond a
  tolerance is a scream, not a silent ship ([F3](forks.md#f3)). The zero-coverage check
  exists because 178 of 255 maps once shipped black; the same failure mode is what let a
  1.8×-oversized AO ride for weeks.
- **R becomes the outline lane** ([F1](forks.md#f1)) — the emissive relay is deleted with
  its dead consumer rather than moved, and the bake **relays** the leaf's R instead of
  overwriting it with an unread presence bit.
- **Outer vs inner is connectivity, not a new detector** ([F4](forks.md#f4)):
  `split_layers.py` already computes the full outline mask (dark core + hysteresis into the
  AA halo). The **outer** outline is exactly the components of that mask touching the
  coverage boundary; everything else is the detail we keep.
- **Shrink presence by half the outer band, measured not guessed** ([F5](forks.md#f5)) —
  a distance transform inside the outer band gives a per-leaf thickness; erode B by half
  its median (floor 1 px) and record the band width in `meta.json` so the client knows how
  thick to redraw.
- **Nothing ships half.** Stripping the rim from albedo without the client drawing it back
  is a visible regression on every sprite, so the extraction, the strip and the client's
  redraw land together ([F6](forks.md#f6)).
- **Shrinking B moves authored geometry.** Every `[thing.part.subframe]` rect is measured
  from B at threshold 0.35, and `meta.json`'s shadow polygons are cut from the diffuse
  alpha — a second derivation that will now disagree. Subframes get **regenerated** (never
  hand-edited) and `outline.py` re-sources from B ([I6](issues.md#i6)).

Authoritative docs touched: **VARIABLES.md** (the new texture-map channel section; the
subframe section gains the erosion note).

Components: [`dev/scripts/art`](../../components/dev/scripts/art/README.md),
[`dev/textures`](../../components/dev/textures/README.md),
[`client/webgl`](../../components/client/webgl/README.md).
