# Forks — SQUARE 128

_Decision points, options, which we chose and why._

## F1 — Which dial do the art maps follow, and which the lightmap? {#f1}

The whole stream turns on this. `TEXTILE_SQUARE = SQUARE` today, so one constant sizes both.

- **(a) Art follows `SQUARE` (→128); lightmap gets a new pinned constant (64).**
- (b) Lightmap keeps `TEXTILE_SQUARE`; art gets the new constant.
- (c) Keep one constant, accept the 4096×2048 lightmap.

**Chosen: (a).** `SQUARE` is defined as "the grid unit — and the MAXIMUM art size: a slot cannot show
more than `SQUARE` px of a tile". Art resolution IS what `SQUARE` means, and `VARIABLES.md` already
documents it as the art dial in the frame-side and `ppu` formulas. Moving art off it would leave the
constant naming nothing.

(b) inverts the meaning of the better-established name for no gain. (c) is what the user explicitly ruled
out — "retain lighting at 64" — and it hands back the A/B's whole win.

## F2 — Name the new constant {#f2}

- **(a) `TEXTILE_LIGHT`.**
- (b) `LIGHT_TEXELS`, matching `SHADOW_TEXELS` in `shadowGather.ts`.
- (c) `LIGHTMAP_SQUARE`.

**Chosen: (a).** It is a per-tile texel resolution on the shared toroidal grid, which is exactly what the
`TEXTILE_*` family means, and `VARIABLES.md` presents those four families as one table. (b) reads well
locally but `SHADOW_TEXELS` is itself just an alias of `TEXTILE_UNIT` kept for a since-settled A/B — it
is not a pattern worth extending. (c) implies it is a variant of `TEXTILE_SQUARE`, which is the
confusion this stream exists to end.

## F3 — Split and raise together, or split first? {#f3}

- **(a) Split at unchanged values (P1), prove the no-op, then raise (P2).**
- (b) One commit: introduce `TEXTILE_LIGHT` and set `SQUARE = 128` together.

**Chosen: (a).** The split touches a slot address (`uLSlot`), and every lod bug this codebase has had was
a slot-address bug — `2026-07-24-map-compatibility` exists solely to police that class. Done at
unchanged values the split has a hard oracle: **every number must be identical**. Done together with the
raise, an identity failure has two candidate causes and the bisect is manual.

(b) is one fewer rebuild. That is not worth losing the oracle.

## F4 — Restore lod 3 while we are here? {#f4}

`LOD_LEVELS = 3` (lod 0..2), and the doc notes `frame_lod` is a `u2` "with room to restore lod 3". At
`SQUARE = 128` the ladder runs 128 → 32; lod 3 would extend it to 16 and a deeper `ZOOM_MIN`.

**Chosen: not this stream.** Nothing about raising `SQUARE` requires it, the record layout already has
the room so it costs nothing to defer, and adding a lod level would change the zoom-sweep acceptance
that every other item in this plan is verified against. Keep the oracle stable.

## F5 — What if P0 shows the wolf is soft for a different reason? {#f5}

P0 measures the wolf's atlas frame side before P3 re-masters anything, precisely so this is answerable.

**Decided in advance: the stream continues either way.** `SQUARE = 128` is a ratified `VARIABLES.md`
constant that the code currently contradicts, so conforming to it stands on its own. If the wolf turns
out to be soft because it was authored soft or loses detail in the channel-pack, that becomes an issue
and a separate stream against `dev/art` — it does not block or redirect this one.

## F6 — Overlap with the `2026-07-28-art-128-tiles` stream {#f6}

Another session opened [`2026-07-28-art-128-tiles`](../2026-07-28-art-128-tiles/README.md) while this
stream was in P0. It owns `bin/art`, `frame_span`, per-cell `pad`, and sizing crops by declared tile
footprint. Its README says it "depends on but does not own `SQUARE = 128` — that is square-128".

- (a) Keep P3 as written and re-master here.
- **(b) Narrow P3 to a client-side verification and leave every art-pipeline change to that stream.**
- (c) Merge the two.

**Chosen: (b)** — and P0's audit makes it nearly free. All 27 manifest entries already carry ≥128
(14 at 512), so **there is nothing to re-master**: raising `SQUARE` doubles `BASE_LOD_PX`, every sprite
steps up one LOD, and the corpus is untouched. P3 therefore reduces to proving `targetPx` went 64 → 128
and that the frames resolve bigger.

(a) would duplicate their work and risk two sessions writing `bin/art` at once. (c) is wrong on
ownership — the two streams cut at exactly the right seam already (they parameterise the tile edge,
this one sets it), and merging would make either unable to land alone.

**Boundary:** if raising `SQUARE` exposes an art-pipeline defect (atlas page pressure from 4× frame
areas is the likely one), it is filed as an issue HERE and fixed THERE.
