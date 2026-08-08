# Issues — surface channels

## I1 — `art ao` cannot see a folded leaf: AO generation has been dead since the fold {#i1}

_2026-08-08 — PROVEN._ `bin/lib/depth_ao.py`'s `find_diffuse()` globs `rglob("diffuse.png")`
and file-matches `p.name == "diffuse.png"` — the **pre-fold flat name**. The current layout
folds direction and part into the filename (`diffuse.e.0.png`), which `bin/art`'s
`_find_diffuse` handles and `depth_ao.py` does not. Reproduced directly:

```
python3 -c "import sys;sys.path.insert(0,'bin/lib');import depth_ao,pathlib;
print(depth_ao.find_diffuse([pathlib.Path('textures/biome-thing/default/conifer/1')]))"
-> []
```

So `art ao` resolves sources correctly in bash, hands them to a script that rejects every
one, and returns 1. `cmd_maps` propagates that (`cmd_ao ... || return 1`), so `art maps --ao`
fails rather than lying — but the practical effect is that **no AO has been regenerated
since the leaf fold**. It also writes `occlusion.png` / `surface.png` via `d.with_name(...)`,
which would emit the flat names even if discovery were fixed — the write path needs the same
treatment as the read path.

## I2 — the on-disk AO maps are stale artifacts at the wrong scale {#i2}

_2026-08-08 — PROVEN._ Consequence of [I1](#i1): the 40 `occlusion.*.png` files in the tree
predate the fold and the pow2-normalisation law, so they sit at their era's master scale
while `diffuse` (and therefore `surface.B`) has been re-normalised since. Bbox sweep of
every AO-bearing leaf (`convert <occ> -format '%@'` vs the diffuse alpha thresholded at 35%):

| leaf | AO bbox | coverage bbox | ratio |
|---|---|---|---|
| `conifer/0/e.0` | `156x256+53+0` | `86x151+86+54` | 1.81 × 1.70 |
| `conifer/1/e.0` | `153x256+52+0` | `105x183+76+37` | 1.46 × 1.40 |
| `conifer/8/e.0` | `175x256+41+0` | `142x216+57+21` | 1.23 × 1.19 |
| `flora/2/e.0` | `128x114+0+7` | `106x98+11+16` | 1.21 × 1.16 |
| `wolf/777/e.0` | `124x62+2+33` | `122x57+3+35` | 1.02 × 1.09 |

Every conifer AO clips the full 256 frame height. The wolf's ~2 px margin is a legitimate
blur halo; the conifer's is not. Two independent symptoms confirm staleness rather than a
tunable: `height_to_ao` returns **1.0 outside the mask** by contract, yet the shipped conifer
AO background reads `gray(16)` = 0.063 — the file was not produced by the current code.

Visual check that makes it unmissable (coverage boundary in red over the AO):

```bash
convert textures/biome-thing/default/conifer/1/diffuse.e.0.png -alpha extract -threshold 35% -edge 1 -fill red -opaque white /tmp/edge.png
```

## I3 — TWO surface writers with contradictory layouts (the "presence in alpha" claim) {#i3}

_2026-08-08._ `bin/art` `_surface_kind` writes `R=emissive, G=AO, B=diffuse-alpha`, **RGB, no
alpha**. `bin/lib/depth_ao.py --surface` writes `R=height, G=AO, B=zeros ("open"),
A=silhouette` — a different lane assignment *and* an alpha payload. The second is where the
"we write presence to alpha" belief comes from: it is real code, and it is wrong.

Its `B = zeros` is precisely the invisible-sprite failure `_surface_kind`'s guard was added
to catch ("measured 178 of 255 surface maps in the tree entirely black before this check
existed") — B is the client's only coverage source, so a zero B renders nothing while every
file exists and every step returns 0. Resolution: **delete** the `--surface` branch and its
flag ([F2](forks.md#f2)); one map, one writer.

## I4 — the leaf R and the composite R mean different things; neither is read {#i4}

_2026-08-08._ The leaf's R is the **emissive mask** (lighting-feel P3 reclaimed the
reserved-for-height lane). The bake does **not** relay it into attachment 1 — it overwrites
R with **presence** (`uTileDepth >= 0.0 ? 1.0 : 0.0`) and side-channels the emissive into
`oDepth.r` instead, because attachment alpha is a blend factor and had no spare lane.

Then lighting-strip deleted the emissive add from the blit. Net today: the leaf R is
computed, the relay runs, and **nothing anywhere reads either R** (`grep` for a `.r` read of
any surface sampler returns only the bake's own write). The composite's presence bit is also
unread — `zdepth.B` already distinguishes ground from things. Both lanes are free, which is
what makes [F1](forks.md#f1) cheap.

## I5 — three comment blocks describe a model the code does not implement {#i5}

_2026-08-08._ Doc-drift to delete alongside the fix, not after:

- `bin/art` `_surface_kind`'s docstring: "the client DERIVES presence from B (smoothstep)
  and premultiplies at bake, so the composite carries A = presence and recovers ao = G/A".
  That is the retired **pixijs** model. The webgl bake keeps A at 1.0 and premultiplies
  nothing (lighting-feel F3). It also still says `R = 0 height (reserved)` four lines above
  the code that writes the emissive mask.
- `bin/art` header + the `maps`/`surface` echo strings: "R=height, G=ao, B=coverage".
- `albedoBlitShader.ts:94-97`: "the ambient × AO term ... [is] gone with the system that fed
  them" — but line 101 reads `surf.g` through `lightAt` whenever `uLit == 1`, and
  `Viewport.litEnabled` defaults **true**. lighting-correctness restored the term and the
  strip's comment was never updated. AO is live; the comment says it is dead.

## I6 — shrinking B invalidates authored subframes and the shadow polygons {#i6}

_2026-08-08._ Two derivations depend on the exact extent of B:

- **`[thing.part.subframe]`** rects in `content/things.toml` (7 tables) are measured by
  `bin/lib/subframe.py` as the bbox of `surface.B > 89/255` — the shader's own constant.
  Eroding B moves every rect. They must be **regenerated, never hand-edited** (the
  subframe-ingest I6 rule), and the corpus republished.
- **`meta.json` `outline`** (the shadow-cast silhouette polygons + earcut) is cut by
  `bin/lib/outline.py` from the **diffuse alpha**, not from B. After the erosion those two
  silhouettes disagree — and the erosion exists precisely so the shadow silhouette shrinks.
  `outline.py` must re-source from `surface.B` so there is one silhouette, not two.

Also downstream of the erosion: placement (`sprite_anchor`, part pivots) is registered
against the subframe, so a re-measure shifts anchors slightly and wants a look at a human
(two parts) and a wolf (facing set) before it is called done.

## I7 — 175 of 216 surface maps have no AO at all {#i7}

_2026-08-08._ Only conifer (9), flora (13) and wolf (18) carry an `occlusion.*.png`; every
other leaf falls through `_surface_kind`'s `G = 1` branch. Measured across the tree: **175
flat-white G, 41 with real darks.** So "fix the scaling" is half the job — the other half is
deciding whether AO is authored per-kind or generated for everything ([F7](forks.md#f7)).
Both tiles and pawn parts currently render with no cavity darkening whatsoever.

## I8 — grid/linked tiles have no outline; the extractor must no-op, not band the frame {#i8}

_2026-08-08._ `biome-tile/**` masters are **opaque squares** — coverage is the whole frame, so
"the outline components touching the coverage boundary" degenerates to a band around the
image edge, which is not an outline and must not be written to R or removed from albedo. The
outer-outline pass needs an explicit guard: a leaf whose coverage is (near-)the full frame
produces an empty R and an untouched albedo. Same guard protects any future full-bleed art.

## I9 — R is a data lane and must stay on the exact page {#i9}

_2026-08-08._ `TextureResolver` line 348: `const graphics = map !== "surface"` — **surface
alone rides the DATA page** (nearest, no mips) while albedo/normal/layers re-source from the
filtered graphics twin. So an outline mask in R is read exactly, which is what a mask needs.
The constraint to respect: do not "helpfully" move surface onto the graphics twin, and do not
encode the outline as something that needs interpolation to be correct.

## I11 — the span reader went stale; every remastered leaf lost its span {#i11}

_2026-08-08 — PROVEN and FIXED (user report: "albedos are not obeying span, coming back as 128
instead of the proper 256")._ `bin/lib/def_span.py` keyed its table on a part's explicit
`texture = "<stem>"`. The corpus stopped authoring that key when defs became registry-numbered
with a **derived** taxonomy — the stem is now `<type>/<subType[0]>/<kind>`, and the only
`texture =` lines left in `content/things.toml` are `texture = "white"`, the flat placeholder
the scanner deliberately skips. So `scan()` returned `{}`:

```
def_span: content/things.toml yielded no texture-bearing defs — the corpus moved or this
reader is stale (see content-toml-only I4)   [exit 2]
```

`_span_side` then returned empty, `cmd_split` fell back to **1 tile = 128 px**, and every
freshly mastered leaf came out half-size. This is precisely the failure `_span_side`'s own
comment predicts ("that failure once hid behind this `2>/dev/null` for a whole migration and
every freshly mastered leaf lost its span") — the loud exit-2 worked exactly as designed; it
had simply not been read yet.

Confusing detail worth recording: `meta.json` still said `span 2 / square 256 / span_from
corpus`. Those keys are merged forward by `meta.py`, so they were **July's** values riding
through today's rewrite while the pixels went to 128. The sidecar and the art disagreed and
only the art was wrong.

**Fix:** `def_span._things` now also reads the def's `type` / `kind` / `subType[0]` and derives
the stem; an explicit `texture` still wins where one is authored. Verified: `def_span --all`
resolves the five stems that have real art (conifer 2/256, flora, wolf, human male+female) and
skips every `white` placeholder.

## I12 — a mismatched channel lane SPLITS the surface map into a numbered sequence {#i12}

_2026-08-08 — PROVEN and FIXED (user report: "we shouldn't have surface.e.0-1 and
surface.e.0-2")._ Downstream of [I11](#i11): today's remaster wrote the diffuse/normal/albedo at
128 while `occlusion.*.png` sat at its stale 256 ([I2](#i2)). `_surface_kind` forces the R lane
to the diffuse's size but **never resized G**, so `convert "$r" "$g" "$b" -combine` got a
128/256/128 sequence, gave up on combining, and wrote the three lanes out as

```
surface.e.0-0.png  128x128   (R)
surface.e.0-1.png  256x256   (G — the stale occlusion)
surface.e.0-2.png  128x128   (B)
```

leaving the real `surface.e.0.png` **untouched at its July bytes**. 27 such orphans were in the
tree. Silent, because every step returned 0 — and worse, the zero-coverage guard then inspected
the *stale* `surface.e.0.png`, found healthy coverage, and said nothing. That stale file is what
"I am still seeing incorrect surface" was: the client was loading July art.

**Fix:** G is resized to the diffuse's geometry exactly as R already is, plus a post-write check
that names the split and removes the orphans if a lane ever mismatches again. Verified: the
conifer reassembles as one 128×128 map with zero `-N` files.

Note the resize fixes the SPLIT, not the SCALE — a 256 AO squeezed into 128 keeps its wrong
subject extent. The [F3](forks.md#f3) bbox guard is still owed.

## I13 — `remaster` never ran AO, so a stale occlusion survived every rebuild {#i13}

_2026-08-08 — PROVEN and FIXED._ The thing that kept [I2](#i2) alive. `cmd_remaster` and
`cmd_maps` both declared `ao=0`, while their own help text says

    --ao  pack depth-derived AO into normal.a (default)

So a bare `art remaster <kind>` appended `--no-ao`, `cmd_maps` skipped `cmd_ao` entirely, and the
July occlusion maps were simply carried forward — rescaled to the new square by
`normalize_square` and recombined, forever. Fixing the [I1](#i1) generator was necessary and not
sufficient: the fixed generator was never called.

How it presented, and why it looked like nothing had changed: after the first fix only
`conifer/1` (regenerated by hand) had a correct AO. The other eight variants still had
`bg=gray(16)` and a 1.7–1.8× bbox, so their surface maps came out **black background, green halo
band, cyan core** — which is exactly the image the user reported as "still incorrect". A correct
leaf is *green* background (AO = 1 outside the mask, per `height_to_ao`'s contract).

**Fix:** `ao=1` in both, matching the documented contract. Verified: `art remaster
biome-thing/default/conifer --layers 2 --split-normalize` now prints the AO step, and all nine
variants come back with a white AO background and a bbox within ~4–6 px of coverage per axis.

## I14 — a broken line-continuation had four `local`s leaking to globals {#i14}

_2026-08-08 — FIXED in passing._ `cmd_maps`' declaration block had two lines inserted into the
middle of a continued `local`:

```bash
local kind="" … ao=0 \
local no_bevel=0; local -a nobevel_args=() engine_args=()
local engine_set=0
      sharpen_normal=0 ao_strength=1.0 sharpen_ao=0 pitch_normal=0
```

The `\` swallowed the next line, so bash read `local kind=… ao=0 local no_bevel=0` (declaring a
variable named `local`), and the trailing `sharpen_normal=0 …` line became a bare assignment
command — those four were set as **globals**, not locals. It worked by accident and `bash -n`
cannot see it. Restored to a well-formed block while the `ao` default was being changed in the
same statement.

## I10 — the client redraw must respect warm-over-cold and the depth key {#i10}

_2026-08-08._ The display blit mixes cold and warm composites by `wcov` and resolves
mover-vs-thing occlusion by the `zdepth.B` painter key before it picks a surface. An outline
drawn from R has to ride the **same** mixed `surf` value, or a mover's rim will draw over a
cold thing that occludes it (and vice versa). Draw it from `surf.r` after the mix, not from
either tier's map directly.
