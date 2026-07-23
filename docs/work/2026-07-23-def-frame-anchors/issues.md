# Issues — def frame/anchor rework

_Problems hit + candidate solutions + which we chose + why. Chronological._

---

## Grid (linked-atlas) stems are second-class in the ingest/def model — 2026-07-23

USER: deferred ("I'll deal with it later"). The scale facet plus the latent ones, recorded for then:

1. **Scale-at-ingest**: one blit scales the whole grid image about its centre → cells migrate off
   their grid slots while `cellFrame` keeps slicing the original grid lines. Per-cell blits (each
   cell scaled about its own centre into its own slot) fix the *mechanics* — but for LINKED
   (edge-matched autotile) art, scaling is semantically questionable regardless: the art must run
   edge-to-edge to tile seamlessly, and a scaled cell leaves gaps at every tile boundary. A scale
   only makes sense for independent-cell variant sheets. → **packed unscaled + one-shot warn**.
2. **Def-model facets (bite when walls become shadow casters):** a cell sub-frame's origin includes
   the manifest `pad` inset — generally NOT 16-px aligned, so `frame_x/y` (u10 16-px grid) can't
   address it exactly; and `spriteBBox` is whole-image — per-cell defs would derive their bbox from
   the union silhouette of ALL cells. Both need per-cell treatment (per-cell bboxes; pad-free or
   16-aligned cell geometry) before a gridded stem casts.

## Hot-swap sprite_scale change leaves stale defs — 2026-07-23

`setSpriteScale` change evicts the stem's packed LODs (repack under the new transform), but defs
are IMMUTABLE and keyed `(stem, cell, lod)` — a repack that relocates the frame leaves the old
def's frame origin stale until reload. Dev-time only (scale changes ride content hot-swap);
fix = def invalidation by stem when C5/eviction lands.

## shared/dsl unit test not run locally — 2026-07-23

`cargo` lives in docker; `rd build shared` compiles (green) but doesn't run the updated
`thing_stem_and_layout_tables` test. Run it with the next dockerised test pass.
