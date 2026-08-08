# Forks — surface channels

## F1 — R is the OUTLINE lane; the emissive relay is deleted, not moved {#f1}

_2026-08-08._ The feature needs a lane for the outer outline and there is exactly one
candidate. The options:

| | |
|---|---|
| **R (chosen)** | Its payload (the emissive mask) has had **no consumer** since lighting-strip deleted the emissive add; the composite's R (presence) is unread too ([I4](issues.md#i4)). Free, byte-aligned, already on the exact data page ([I9](issues.md#i9)). |
| A: the composite's alpha | Refused outright. Attachment alpha is the **blend factor** — writing data there multiplied the whole write by it and collapsed world coverage to nothing (lighting-feel P3). This is the mistake the layout already learned. |
| A fifth map | A new map is a fifth atlas quadrant, a fifth fetch, a fifth thing to keep registered. Not for a 1-bit-ish mask when a whole lane is idle. |
| Pack into B's spare bits | B is compared against 0.35/0.5 thresholds in five places and is bilinear-sampled nowhere but must stay monotonic. Sub-bit packing a threshold lane is how the next confusion starts. |

**Consequences that are part of the choice.** The leaf's emissive write in `_surface_kind`
goes; `uEmissiveOn` and the `oDepth.r` relay in `mrtBakeShader` go with it; the
`t.width > 1` "real map" discriminator that gated the relay goes. If emissive returns it
returns as its own map with a live consumer, which is what the deleted code was pretending
to be. The bake stops writing presence into attachment 1's R and **relays the leaf's R**
instead, exactly as it already relays G and B.

## F2 — `depth_ao.py --surface` is DELETED {#f2}

_2026-08-08._ Two writers, two layouts, one of them wrong ([I3](issues.md#i3)). Options were
"fix it to match" or "delete it". **Delete.** Nothing calls it (`cmd_ao` never passes
`--surface`; `cmd_maps` assembles via `_surface_kind`), it writes the flat pre-fold filename
so it could not land in a folded leaf anyway, and its `B = zeros` is an active foot-gun that
already cost the tree once. Keeping a second assembler "in case" is exactly the deprecation
the tree bans — git holds it.

Deleted with it: the `--surface` flag, its help text, and the `--ao-floor` / `pack_normal`
premultiply rationale that only existed for the retired alpha-packing path (verify before
cutting — the floor may still guard the `--pack-only` branch).

## F3 — a GEOMETRY guard beside the zero-coverage guard {#f3}

_2026-08-08._ [I2](issues.md#i2) shipped for weeks because nothing compared the AO to the
silhouette it is supposed to occlude. `_surface_kind` already refuses to be quiet about a
zero B; it gets a sibling: after combining, compare the **opaque bbox of G** against the
**bbox of B** and scream when they disagree by more than a halo's worth.

Chosen tolerance: **max(4 px, 6% of the coverage extent) per axis**. The wolf's real halo is
2 px (1.02×) and passes; conifer at 1.2–1.8× fails loudly. Deliberately a *warning to stderr*
in the same voice as the coverage check rather than a hard failure — a legitimately blurred
AO on new art should not block a `maps` run, but it must never again pass unnoticed.

Rejected: masking G to the coverage silhouette at assembly time. That would *hide* the
mismatch instead of reporting it, and out-of-silhouette AO is already discarded by the bake.
The mask is the wrong fix for a stale input.

## F4 — outer vs inner outline = connectivity to the coverage boundary {#f4}

_2026-08-08._ `split_layers.py` `_outline_mask()` already produces the whole outline: the
dark core (`lum < --outline-v`, default 0.18) grown by **hysteresis** into adjacent still-dark
rim pixels (`< --outline-rim`, 0.35) so the anti-aliased halo comes along without eating a
material's interior shadow. That mask is the input; it needs no replacement.

**Outer outline = the connected components of that mask that are adjacent to the exterior**
(a pixel outside coverage), found by a single flood from the boundary. Everything else — the
conifer's tier lines, the trunk seams, the wolf's leg separations — is interior detail and is
**kept in albedo untouched**, which is the user's constraint.

Rejected: erode-the-silhouette-by-N (a fixed band ignores that the line thickens at silhouette
corners and thins on long edges, and it cuts interior lines that happen to sit near the edge);
a chroma/luma re-detection (a second detector that can disagree with the one the layer split
already used — the same two-derivations trap as [I6](issues.md#i6)).

Where it runs: the outline mask is computed inside `split_layers.py` from the **de-lit
albedo**, which is where the line is cleanest. So the outer/inner split lands there and the
outer mask is handed to `_surface_kind` as a co-located intermediate (`outer_outline.<dir>.<part>.png`),
the same way `occlusion` is. That keeps one detector, one source of truth, and lets a leaf
without a layer split fall through to an empty R.

## F5 — erode presence by HALF the measured band, and record the width {#f5}

_2026-08-08._ The user's spec is "about half of the outer outline". Half of *what number* is
the question — the band is not uniform.

**Chosen:** per leaf, take the distance transform inside the outer-outline band, read its
**median** thickness, erode B by `max(1, round(median / 2))` px, and write the measured band
width into the leaf's `meta.json`. Median, not mean, because a few thick corner blobs should
not widen the erosion along the whole edge; `max(1, …)` because a sub-2 px band still wants
its half-pixel of slack and a 0 px erosion silently does nothing.

Recording the width is what makes the client's redraw honest: the shader draws a rim of the
same thickness the pipeline removed, instead of both ends guessing a constant.

Rejected: a global constant px (art is authored at one master square but subjects letterbox
at wildly different scales — a conifer's line and a flora sprig's line are not the same
width); eroding by the full band (the user said half, and the whole point is that the drawn
line covers the difference — over-eroding starts eating the sprite).

## F6 — extraction, strip and redraw land TOGETHER {#f6}

_2026-08-08._ P3 alone (write R) is invisible and safe. P4 alone (strip albedo + erode B)
removes the black rim from every sprite in the game and shrinks its silhouette — a plain
visual regression until something draws the rim back. So P4 and P5 are one landing: the
client's outline draw is in place, behind a debug toggle, **before** the tree is regenerated
without rims.

Sequencing that respects it: build P5's shader path against a **hand-made** R (one leaf,
generated by P3) while the tree still carries its rims and the strip is off; only then run
the strip over the tree. The toggle stays for the A/B, and the stream ends with it defaulting
on.

## F7 — AO coverage: fix the 41, then decide the 175 {#f7}

_2026-08-08._ [I7](issues.md#i7) says most leaves have no AO at all. This stream's committed
scope is **the 41 that claim to have one** — fix the generator, regenerate, guard. Generating
AO for the whole tree is a separate call with a real cost (every leaf re-runs an FFT
integration + an HBAO march) and a real question behind it (does a flat tile want AO at all,
or is its relief the normal map's job?).

**Chosen:** regenerate the three AO-bearing kinds under the fixed generator, look at the
result in the client, and record the tree-wide decision in this stream's `completed.md` as an
explicit recommendation with the measured cost — a successor executes it. Not deferred
silently: the guard from [F3](forks.md#f3) makes a missing AO visible as `G = 1` rather than
as a wrong number, so the gap is honest while it lasts.
