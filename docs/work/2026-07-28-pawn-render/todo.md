# Todo — pawn-render

_Items tick in place; the box is the move. Design: [`README`](README.md) ·
[`intent/tiered-lighting`](../../components/client/webgl/intent/tiered-lighting.md) ·
[`forks`](forks.md). Every visual item verifies at ≥3 zoom levels + during a zoom transition
(the attempt-#2 failure mode) — a fixed-zoom screenshot proves nothing here._

---

## P0 · Baseline

- [x] Capture the current wrongness while the wolf soaks: (a) wolf painting over a tree it
      stands behind, (b) flat/unlit wolf vs lit terrain, (c) ground-shadow darkening the
      wolf wholesale, (d) an n/s-facing cold prim or wolf casting an e/w silhouette, (e)
      emissive working (the regression control). Acceptance: screenshots + one console dump
      of cold/hot dirty counts while the wolf wanders (the cold-never-rebakes baseline).

## P1 · Depth-correct composite (front/behind vs trees)

- [x] Blit: read BOTH zdepth B lanes (already bound as `uDepth`/`uDepthWarm`,
      `albedoBlitShader.ts`) and pick the winner per pixel: warm beats cold UNLESS the cold
      pixel is a thing (`0x80` bit) whose base row is serially SOUTH of (greater than, mod
      128, wrap-aware) the warm base row — then the cold pixel wins outright (albedo,
      surface, emissive lane). Ground (`depth 0`) never occludes. Acceptance: wolf walking
      a lap around a conifer is occluded by the trunk when behind, occludes it when in
      front, at 3 zooms + mid-transition; no regression on mover-free scenes.
- [x] Row-key audit: confirm the baked warm base row tracks the CHASED fractional position
      consistently with cold things' rows (`prim.y + prim.height` convention,
      `Viewport.ts:202-204`) so the flip line sits at the visually-correct row; fix either
      side if they disagree. Acceptance: the front/behind flip happens as the wolf's anchor
      crosses the tree's base row — no premature pop.

## P2 · Movers into the HOT lighting pass (lit + normal-mapped wolves)

- [ ] Warm billboard records: pack the wolf's live state (position box, facing/rotation,
      atlas frame for the CURRENT facing, flipX) into the SAME record format as
      `coldShadowData`'s billboard records, delivered per frame by UNIFORM (F2 — movers are
      few); a shared decode serves texture (cold) and uniform (warm) sources. Acceptance:
      unit-level — a warm record round-trips through the decode identically to an
      equivalent cold record.
- [ ] Hot pass takes warm receivers: warm prims join the HOT class pass's receiver maps
      (own dirty channel — the mover's rect re-marks per move, which MoverLayer's eps-gated
      re-bake already bounds); the hot pass evaluates ALL lights (cold + hot) on
      warm-receiver texels, wolf N·L from the wolf's atlas normal frame through
      `worldNormal` (the same pitched world-frame rotation cold billboards use), landing in
      `hotLightRT` ONLY. The cold pass is untouched. Acceptance: wolf shows sprite relief
      under a nearby cold torch (normals working); `__shownormal` sane on the wolf; cold
      dirty count stays 0 while the wolf wanders.
- [ ] Blit light-select (F3): warm-winning pixels take `ambient + hotLight`; cold pixels
      unchanged (`ambient + coldLight + hotLight`). Acceptance: no double-bright seam where
      the wolf overlaps lit ground; a wolf in a dark corner is dark; the tier matrix holds —
      cold light lights the wolf via the HOT map (verify by watching `hotLightRT` dirty
      rects follow the wolf while `coldLightRT` stays untouched).
- [ ] Perf guard: the per-frame hot cost with a wandering wolf + the standing light rig
      holds the frame budget (measure against the 63-light/120 fps baseline). Acceptance:
      fps numbers in `completed.md` at zoom 1 and zoom 0.25.

## P3 · Shadows cast BY and ONTO wolves (hot maps only)

- [ ] Wolf casts: warm records join the HOT pass's caster walk (per-frame re-bucket for the
      handful of movers — the "O(1) re-bucket lands with the hot tier" seam,
      `shadowGather.ts:1648`); the wolf's silhouette shadows ground AND cold billboards via
      the existing climb math, landing in the hot map. Self-shadow excluded by the existing
      `casterOne` culls. Acceptance: the wolf drags a soft silhouette shadow past a torch;
      a wall/tree south of the wolf catches its climbing shadow; all in `hotShadowRT`, cold
      maps untouched.
- [ ] Wolf receives: warm receiver texels get the climbing-shadow treatment (elevation from
      the wolf's own base row) from cold AND hot casters, attenuating the hot-map light —
      the "tree's shadow climbs the wolf" shot. Acceptance: wolf walking through a tree's
      shadow shows the shadow crossing its BODY at the right height (not just its ground
      tile), at 3 zooms; the P0 baseline's wholesale-darkening artifact is gone.
- [ ] Matrix drill: with one cold light, one hot (flicker) light, one cold tree, one wolf —
      verify each of the four cells lands in the right map (instrument dirty/bake counters
      per RT): cold×cold bakes once and never re-bakes; the other three re-render per frame
      in hot; toggling the wolf away restores a hot-map-empty steady state. Evidence in
      `completed.md`.

## P4 · N/S billboards (cast + receive + normal)

- [ ] Records carry true rotation (0=s 1=e 2=n 3=w) end to end: `coldShadowData` stops
      hard-coding the e/w regime; the frame referenced is the FACING'S OWN atlas frame
      (n/s art silhouette) — cold prims with n/s rotations fix for free alongside the wolf.
      Acceptance: record decode round-trips all four rotations.
- [ ] Shader arms for n/s: `sampleCard` (u-mapping per rotation), `casterCover` (silhouette
      of the n/s frame on the standard card plane — F5: geometry unchanged, art changes),
      `billboardNormal` (n/s normal frames; no x-mirror for s, mirror rules stated per
      rotation). Acceptance: a wolf walking NORTH casts its front/back silhouette (not the
      side profile) — the user's reported artifact gone; e/w behavior bit-unchanged
      (corridor↔brute identity still holds on a cold-only scene).
- [ ] Receiving parity n/s: shadows climb n/s-facing wolf sprites at the same heights as
      e/w (the receiver plane math is facing-independent; verify rather than assume).
      Acceptance: lap around a shadowing tree shows consistent shadow behavior through all
      four facings.

## P5 · Wrap

- [ ] Docs: update `intent/tiered-lighting.md` status (mover tier now participates: what of
      the warm/rt tiers this stream realized vs left) + a §mover note in
      `design/shadows.md`; reconcile the STALE
      [2026-07-24-shadows-onto-prims](../2026-07-24-shadows-onto-prims/README.md) folder
      against the now-live receiver machinery (verify what its 0/8 items still mean; close
      or re-scope with the user's model preserved). Acceptance: `bin/rd docs-check` green;
      no doc claims billboards receive no shadows.
- [ ] Full acceptance lap: wolf circles a lit, shadowed grove — depth flips correct,
      lit + normal-mapped through all four facings, casts and receives shadows, emissive
      intact, cold maps never re-bake, fps holds. Screenshots (≥3 zooms) + counters in
      `completed.md`; memory updated; work-index row → done.
