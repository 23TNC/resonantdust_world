# Issues — linked cell padding

## I1 — Step 3 of the request ("scale up to 128") already exists, and is free

The user described three steps: grab the 128 px frame, apply the padding to all four sides, scale the
result back up to 128.

**Only step 2 needs building.** `cellFrame` returns a `TexFrame` — a rect on the atlas page — not a
resampled image:

```ts
const t = new TexFrame(base.source, base.x + u0 * base.w, base.y + v0 * base.h, uw * base.w, vh * base.h);
```

The consumer samples that rect across the destination footprint, so a smaller source rect over the same
destination IS the scale-up. No blit, no second pass, no cost. Worth stating plainly because the
request reads like three pieces of work and it is one.

**The catch is what that free stretch costs elsewhere** — it is a non-integer magnification, which
every other map in this engine avoids by construction. See [F5](forks.md#f5). The scaling being free is
what makes the artifact possible; if it were a resample we would control the filter.

## I2 — Two live definitions of `pad` in the tree, meaning opposite things

Worth knowing before authoring anything, because the word appears in both places and they are not the
same quantity:

| where | meaning |
|---|---|
| `bin/lib/pad_maps.py --pad N` | a canvas-edge guard ring, in px, added AROUND a whole map |
| `atlas.json` `padU`/`padV` → manifest `pad` | a normalized per-cell sampling INSET the client trims |
| **this stream's `texture_pad`** | **px of guard per cell edge — the authored source of the inset** |

`pad_maps.py`'s own header records that the first is wrong for atlases: *"the boundaries a sampler
actually crosses are the INTERIOR ones, which a canvas ring never touches, and shrinking the content
pulls every cell off its pitch (measured on an 8×8 sheet: 1024 canvas preserved, but 1022 px of content
across 8 cells = 127.75 px/cell instead of 128)."*

That measurement is the same arithmetic as [F5](forks.md#f5), arrived at from the writer's side — the
pipeline already discovered that insetting content off a pow2 pitch produces fractional cells. This
stream is deciding whether the same fractional-cell effect is acceptable when it happens at *sampling*
time instead of at *bake* time. It costs no pixels, which is the argument for it; it is still not an
integer scale, which is the argument against.
