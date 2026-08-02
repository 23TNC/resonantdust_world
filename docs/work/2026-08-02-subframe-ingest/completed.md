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
