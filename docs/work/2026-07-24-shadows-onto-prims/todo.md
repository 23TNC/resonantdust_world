# Todo — cast shadows onto prims (execution order)

_Model + framing in [`README.md`](README.md); decisions in [`forks.md`](forks.md). VERIFY every phase at
**≥3 zoom levels + a live zoom transition** — the reverted attempt #2 passed at one zoom and failed on
zoom, so a single-zoom check is not acceptance. Use the zero-reach keep-alive light trick to hold a static
frame (the render loop is change-gated). Freeze the subject light, keep one zero-reach light dynamic._

## P0 · A zoom-safe receiver mask (the ONLY genuinely new work)
- [ ] Decide + build the in-family receiver source ([forks F1](forks.md#f1) — lean: bake a `textile_unit`
      receiver-depth map aligned texel-for-texel with `shadow-cold`, holding `is-thing` + `base-row` per
      texel). Populate it from the standing prims (their drawn billboard silhouette + base row).
- [ ] VERIFY (the acceptance test attempt #2 lacked): sample the mask by world coordinate in the gather and
      confirm `is-thing`/`base-row` read the SAME value for the same world position **across ≥3 zooms and
      through a zoom transition** — no drift, no flicker. Prove it with a debug read at two zooms, not by eye
      alone.

## P1 · Re-home the (verified) receiver math onto the new input
- [ ] Port attempt #2's gather changes verbatim EXCEPT the input: elevation `z = sin65·(base_y − P.y)`,
      per-light projection to `G`, and `casterOne()` with the two cone culls (seen-face: caster row >
      receiver row; light-side: `dot(Cb−Rbase, Rbase−L) < 0`). The receiver `is-thing`/`base-row` now come
      from the P0 mask, not a composite. Reuse `casterCover` + the corridor.
- [ ] VERIFY: corridor↔brute **bit-identical** with the prim path live (`__corridor` + `debugReadShadow`
      diff = 0) — re-earn P6, now at multiple zooms.

## P2 · Blit consumes the climbing prim shadow
- [ ] Re-enable the blit to take the (now-correct) shadowed irradiance on thing texels (replacing the
      current `inFront → isThing` placeholder that gives billboards full light). Keep a debug toggle to fall
      back to the placeholder for A/B.
- [ ] VERIFY: shadows climb receivers behind a caster (light south of the caster), no self-shadow, ground
      unchanged, transparent sprite gaps show ground shadow — at ≥3 zooms.

## P3 · Verify + close
- [ ] The full zoom sweep is the headline test: scrub zoom in/out over the forest, confirm shadows on prims
      are stable in shape + placement + no flicker (the exact failure that reverted attempt #2). Screenshot
      the same tile at 3 zooms.
- [ ] Cold/hot split inherited free (one gather per class); confirm cold pass-2 dirty = 0 steady-state.
      Perf holds at display cap. Docs + memory updated; retire the binary/placeholder path.
</content>
