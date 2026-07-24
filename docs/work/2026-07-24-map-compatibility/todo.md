# Todo — map compatibility (investigation order)

_Model + framing in [`README.md`](README.md); decisions in [`forks.md`](forks.md); findings in
[`issues.md`](issues.md). This is an INVESTIGATION — its deliverable is a compatibility rule + a decision,
not a shipped feature._

## P0 · Inventory every live map + its TRUE on-texture layout
- [ ] One row per map actually allocated in `game/viewport/` (shadow-cold, cold+hot lightmaps + their MRT
      attachments, the data texture, the four `SquareCache` composites × cold/warm, the command buffer,
      any preview/atlas pages). For each: family, textiles/tile, texture size (derived-from-window vs
      fixed reserve), pad/apron, contiguous vs slot-atlas, toroidal basis, and **how a reader addresses it**
      (world coord recompute vs cache-supplied UV vs texelFetch by index).
- [ ] VERIFY each row against the code (size at allocation, index math at every read site) — cite file:line.

## P1 · The compatibility matrix + the drift root cause
- [ ] From P0, fill an N×N matrix: for each (reader coordinate system) × (target map), mark SAFE / UNSAFE /
      SAFE-ONLY-VIA-SUPPLIED-UV, with the reason. The single UNSAFE cell that bit us: `textile_unit` shader
      → `textile_square` composite by world position.
- [ ] Reproduce the drift NUMERICALLY (not just "it flickered"): pick a world tile, compute the composite
      texel my reverted UV produced at two zooms (`slotPx` 48 vs 57), show it lands on different / wrong
      texels. Confirm the `floor` + apron + fixed-`fixedCW` are the cause ([issues.md#i1](issues.md#i1)).
- [ ] Note every OTHER place (if any) that already cross-references a composite by world coord — audit for
      latent copies of the same bug.

## P2 · Decide the rule ([forks F1](forks.md#f1))
- [ ] Choose: (a) conform the `SquareCache` composites to `map-model.md` (contiguous `cols·SQUARE ×
      rows·SQUARE`, no apron, no fixed atlas) so cross-family world-coord lookups become legal at any zoom;
      or (b) keep the atlas and make "no shader cross-references a composite by world coordinate" a RULE,
      with features deriving needed data within their own family. Weigh cost, risk to the working blit, and
      what future features (on-prim casting, others) actually need.
- [ ] Reconcile `map-model.md` with reality either way (conform the code to the doc, or amend the doc to
      admit the atlas + state the rule). One of them must stop lying ([issues.md#i2](issues.md#i2)).

## P3 · Re-plan on-prim casting on the chosen basis
- [ ] Rewrite [`2026-07-23-shadows-on-prims`](../2026-07-23-shadows-on-prims/README.md)'s P0 so the receiver
      is-thing + base-row comes from a SAFE source under the P2 rule (e.g. the caster buckets already in the
      data texture, or a `textile_unit`-family depth), never the `textile_square` zdepth composite by world
      coord. Everything downstream (the cone culls, corridor↔brute identity) was correct — only the input
      lookup was incompatible; it can be reused verbatim once the input is safe.
</content>
