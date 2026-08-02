# Completed — subframe at ingest

_The verification log: dated entries saying what landed and HOW it was checked. Append-only;
authoritative for what is done and why we believe it. Item text lives in `todo.md`._

## 2026-08-02 — P0, the baseline

Measured live at `?user=Claude&focus=100,48&zoom=4`, one torch at `(100,50)`, against the build at
`025fc1f2` (i.e. AFTER the subframe deletion — this stream starts from the current state, not from
the haloed one).

**The `100,48` bush's fringe** — `biome-thing/default/flora/e`, receiver map read back at 64
texels/tile:

| | value |
|---|---|
| covered texels | **1925** |
| topmost covered row | **8** (of 64) → world y **770.125** units |
| art's drawn top | **769.75** units |
| frame | **768 → 784** units |
| fringe | **−0.375 unit** — the receiver sits *inside* the art, no halo |

The halo is gone, as expected: the sign is negative (receiver smaller than art) where the bug had it
positive (receiver north of art). One receiver texel is 0.25 unit, so this residual is 1.5 texels on
a silhouette whose top row is a single bush spike — at the noise floor of the instrument, not a
finding. P4 compares against these two numbers.

**Opaque fraction of frame, and the plan-line error** — every stem the resolver had decoded (11):

| stem | span | opaque frac | plan-line error (units) |
|---|---|---|---|
| `pawn/animal/wolf/e` | 1 | 0.447 | **4.38** |
| `pawn/human/female/11/e.1` | 1 | 0.190 | 4.25 |
| `biome-thing/default/conifer/e` | 2 | 0.325 | 4.00 |
| `pawn/animal/wolf/s` | 1 | 0.333 | 1.63 |
| `biome-thing/default/flora/e` | 1 | 0.742 | 1.38 |
| `pawn/animal/wolf/n` | 1 | 0.255 | 1.13 |
| `pawn/human/female/7/s` | 1 | 0.528 | 1.00 |
| `pawn/human/female/7/n` | 1 | 0.547 | 1.00 |
| `pawn/human/female/7/e` | 1 | 0.302 | 0.63 |
| `pawn/human/female/11/s.1` | 1 | 0.690 | 0.50 |
| `pawn/human/female/11/n.1` | 1 | 0.707 | 0.25 |
| **average** | | **0.461** | **1.83** |

Two results worth stating plainly:

- **[I1](issues.md#i1) is answered, and it is not "correctness only".** More than **half** of every
  atlas frame is transparent margin on average. The crop is a real resolution and memory win.
- **[lighting-visual I1](../2026-07-31-lighting-visual/issues.md) is quantified at 1.83 units
  average, 4.38 worst** — the wolf's east frame stands nearly a third of a tile below its own feet.
  That is the number P3 closes.

**`packCoPack` shares one rect across all four maps** — confirmed by reading, `TextureResolver.ts`:

```ts
draws = srcs.map((s) => (s ? draw : null));   // ONE `draw`, four sources
```

Nothing enforces it, which is why the item exists; the design rests on this line.

**Found while measuring — [I7](issues.md#i7):** `flora/e`'s bbox read `fy = 0.125` in one session and
`0.109` in the next. The bbox is computed from whichever lod happened to decode first, so it is
**not lod-stable**. That is independent evidence for authoring the number rather than deriving it.

## 2026-08-02 — P1, the DSL carries the subframe

`shared/dsl` gains `DirFrame { sub: (x,y,w,h), anchor: (x,y) }`, all fractions, and
`VisualParts.dir_frames: [DirFrame; 3]` for `[e, s, n]`. Read from
`&thing.subframe.<dir>.{x,y,w,h}` / `&thing.sprite_anchor.<dir>.{x,y}`, each falling back to the
non-directional `&thing.subframe.*` / `&thing.sprite_anchor.*`, which default to the **whole frame**
`(0,0,1,1)` and centre pivot — so a kind that has not opted in cannot move.

Exposed to the client as `thing_subframe()`, **stride 18** (3 directions × 6 floats). Kept as a
sibling of `thing_layout()` rather than widening its stride-10, because every existing consumer
indexes into that array.

**Verified:** `cargo test --lib` in `shared/dsl` — **48 passed, 0 failed** (47 before). The new
`thing_subframe_is_per_direction_with_fallbacks` exercises the whole fallback chain in one corpus: a
kind authoring a non-directional rect *plus* an `n`-only `x` override *plus* an `e`-only pivot
override, beside a kind authoring nothing. It asserts the exact 36-float output and that an
unauthored kind's `dir_frames == [DirFrame::default(); 3]` with `sub == (0,0,1,1)`.

**West has no row** ([F4](forks.md#f4)) — the stride is 3, not 4, and the loader doc says why where
someone would be tempted to add it.

**Cost of the fixture, worth recording:** the test first failed with *every* row defaulted, including
the authored kind. Cause: `node_visual` opens with `store.read("prims.0.tint")?` — a corpus that
authors no tint returns `None` and every field silently defaults. Not a bug in this work, but it is a
trap for anyone writing a loader fixture, and it presents as "my new field does not parse".

**Ticked out of order:** the `sprite_anchor` re-base ([F7](forks.md#f7)) and the west-mirror items
have their DSL half done here but their client half in P2, so they stay open until the resolver
actually reads them. Ticking them now would claim wiring that does not exist.

## 2026-08-02 — P1 cont., the corpus is authored and VARIABLES.md carries the model

**Measured from the masters on disk** ([F8](forks.md#f8)), not from the client — [I7](issues.md#i7)
showed the runtime bbox moves with streaming order, and a permanent number must not. One pass over
`textures/**/surface.<dir>.<part>.png` at 128 px, thresholded at `B > 0.35` (the shader's own
constant), unioned across every variant folder so no variant is clipped:

| stem | dir | variants | x | y | w | h |
|---|---|---|---|---|---|---|
| `biome-thing/default/conifer` | e | 9 | 0.2188 | 0.0391 | 0.5625 | 0.9336 |
| `biome-thing/default/flora` | e | 13 | 0.0547 | 0.1094 | 0.8984 | 0.7969 |
| `pawn/animal/wolf` | e | 15 | 0.0 | 0.2578 | 1.0 | 0.4688 |
| `pawn/animal/wolf` | n | 9 | 0.3359 | 0.0859 | 0.3281 | 0.8438 |
| `pawn/animal/wolf` | s | 9 | 0.2969 | 0.0469 | 0.3906 | 0.8594 |
| `pawn/human/female` body | e/n/s | 9 | 0.1484 / 0.0781 / 0.0703 | 0.0469 | 0.7109 / 0.8516 / 0.8594 | 0.9141 |
| `pawn/human/female` head | e/n/s | 16 | 0.1172 / 0.1328 / 0.1484 | 0.0391 | 0.7656 / 0.7344 / 0.7109 | 0.9219 / 0.9297 / 0.9297 |
| `pawn/human/male` body | e/n/s | 9 | 0.1406 / 0.0625 / 0.0391 | 0.0391 / 0.0469 / 0.0469 | 0.7188 / 0.8828 / 0.9219 | 0.9219 / 0.9141 / 0.9062 |
| `pawn/human/male` head | e/n/s | 16 | 0.1016 / 0.125 / 0.1406 | 0.0391 / 0.0469 / 0.0312 | 0.8047 / 0.75 / 0.7266 | 0.9219 / 0.9141 / 0.9375 |

**The wolf is the case that justifies per-direction authoring**: its side view is long and flat
(`h 0.47`) and its front/back are tall and narrow (`w 0.33`). One rect could not have served both,
which is the user's point stated as data.

Authored into `content/visual/things.rd` (conifer, flora, wolf) and `content/visual/pawns.rd`
(female/male × body/head). Cold things author **non-directionally** — a cold thing has one mastered
facing, so e/s/n all inherit one rect — and pawns author per direction.

**That forced a plan extension, done here:** `VisualPart` needed its own `dir_frames`, because a
part is its own master with its own extent (female `s` body `h 0.914` vs head `h 0.930`, at different
`x`). Added to the loader, and exported on each `moverParts()` slot as `subframes` — the same
stride-18 shape `thingSubframe()` uses, so the host has one decoder for both.

**Every mastered stem in the corpus is covered.** The corpus references exactly five: conifer, flora,
wolf, human/female, human/male. `berry` exists under `textures/` but no corpus kind uses it (and it
ships no surface map). Every other kind draws the built-in `white`, which has no master and correctly
keeps the whole-frame default. The `biome-tile/*/l` grid stems are deliberately **not** authored here
— they need the per-cell path ([F6](forks.md#f6)), which is a P2 item.

**Verified:** `cargo test --lib` in `shared/dsl` — 48 passed, 0 failed, including
`the_real_repo_corpus_loads_and_the_humans_declare_their_parts`, which loads the actual repo corpus,
so the authored `.rd` parses. `rd build shared` builds the wasm bundle clean.

**`VARIABLES.md` updated on two counts.** It still documented the *deleted* `definition_data` GREEN
subframe lane — corrected to "free, reserved for the anchor" with the reason. And a new
**Sprite subframes** section documents the DSL variables, the stride-18 wire shape, and the four
rules that are easy to get wrong later: fractions not units, three directions not four, per-direction
because a facing is a different texture, and authored from masters not from the client.

## 2026-08-02 — F9 rework: ONE 0..15 rotation index space

The user's mid-flight correction: *"you are almost certainly going to need rotation/direction 0..15
because linked directions are 0..15."* Correct, and the evidence was already in the record —
`ROTATIONS_PER_DEF` has always been **16**, a sprite using `0..3` and a linked tile all sixteen. They
were never two index spaces; one is a prefix of the other. `[DirFrame; 3]` → `[DirFrame; 16]`, wire
stride 18 → **96**, in both `thingSubframe()` and each `moverParts()` slot's `subframes`.

Authored `&thing.subframe.r<0..15>.{x,y,w,h}` with `s`/`e`/`n`/`w` as aliases for `r0..r3`, so sprite
corpora stay readable and linked tiles address cells directly. The fallback chain is index → alias →
non-indexed → whole frame.

**This is also what makes [F6](forks.md#f6) land without a mechanism of its own** — a linked stem's
per-cell subframe is just indices `0..15` of the same array.

**Verified:** `shared/dsl` 48 tests pass. The rewritten test exercises the full chain in one corpus —
a non-indexed rect, an *alias* override (`n`), an *index* override (`r9`, a linked cell with no
alias), and an alias-only pivot (`e`) — asserting each of `f[0]`, `f[1]`, `f[2]`, `f[3] == f[0]`,
`f[9]`, `f[15] == f[0]`, and that an unauthored kind is `DirFrame::default()` at *every* rotation.

**Cost, recorded as [I8](issues.md#i8):** the first attempt used bare numerals (`subframe.9.x`) and
the write was **silently dropped**. A digit-only path segment parses as an array index, but
`subframe` is already a Map once `subframe.x` is authored, so the walk falls through with no write
and no error. Hence the `r` prefix. This trap is general to the DSL, not specific to this stream.

## 2026-08-02 — P2 in flight (NOT yet ticked)

`TextureResolver.setSubframe` + the crop branch in `packCoPack` are written and type-check, and the
wasm carries the data — but **nothing calls `setSubframe` yet**, so the client behaviour is
unchanged and no P2 item is ticked. What is written:

- ONE `AtlasDraw` computed from the subframe and applied to all four sources — the same
  `draws = srcs.map(...)` shape, now fed by an authored rect.
- Scale-to-**fit**, aspect preserved ([F2](forks.md#f2)); the quadrant stays square pow2, so `ppu`
  and the lod ladder are untouched ([I3](issues.md#i3)).
- `sprite_scale` **multiplies** the fitted rect instead of performing its own pivot re-centre
  ([F5](forks.md#f5)) — the old re-centre survives only on the legacy branch, for stems with no
  authored subframe.
- Placement puts the art's own anchor at the same fraction of the quadrant. **Worth noting for
  [F3](forks.md#f3):** with the corpus's `sprite_anchor.y = 1`, that lands the feet exactly on the
  frame's bottom edge — which may close [lighting-visual I1](../2026-07-31-lighting-visual/issues.md)
  *without* the anchor record lane. P3 should test that before adding the lane.

**`internal_padding` is deliberately still alive.** [F6](forks.md#f6) stands, but the substitution
belongs at `cellFrame` (resolve-time, per cell) rather than at `packCoPack` (ingest, per stem), and
`cellFrame` takes a symmetric pad where a subframe is an asymmetric rect. Removing it half-way would
silently shift every autotile cell, so it stays until that item is worked properly.
