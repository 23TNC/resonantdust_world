# Todo — unified data texture + scatter (execution order)

_Each phase verifiable on `/overlayRT shadow-cold` + the corridor↔brute identity diff
(`__corridor(false)` + `__gather.debugReadShadow()`, must stay **0 mismatches**) + the zoom
round-trip (silhouettes persist, nonzero deterministic). Items move to
[`completed.md`](completed.md) when done **and** verified._

## P0 · Authoritative layouts

- [ ] `docs/VARIABLES.md`: the unified data texture (bands, base indices, constants row) + the
      command-buffer format (2-px commands, u20 target). Resolve [F1](forks.md#f1) (prim_data
      1 px/record) — the layout change rides this rewrite.
- [ ] Decide which uniforms migrate to the constants row (window mapping yes; debug toggles like
      `uCorridor` stay uniforms) — [F2](forks.md#f2).

## P1 · Merge the tables (CPU uploads first — transport unchanged)

- [ ] One 1024×1024 RGBA32UI texture + one CPU mirror; band base constants shared TS ↔ GLSL.
- [ ] `ColdShadowData` writes band-offset linear indices; row-span uploads (`uploadRows`) still the
      transport. prim_data → 1 px/record (F1): encode + `casterCover` fetch + debugPrim.
- [ ] Gather binds the one texture (7 → 5); `fetchLin` takes a band base.
- [ ] VERIFY: identity 0 mismatches, zoom round-trip, def decode sane.

## P2 · Scatter transport

- [ ] Command buffer (64×64 RGBA32UI) + mirror + writer API (`enqueue(target, payload)`; flush =
      one `uploadRows` span + one `drawArrays(POINTS, count)`).
- [ ] Scatter Program: `gl_VertexID` → command fetch → point at target texel NDC → integer
      fragment writes the payload. FBO-attach the data texture; ordered BEFORE the gather.
- [ ] All `ColdShadowData` writes route through commands (appends included — uniform write path);
      `uploadRows` retired for the merged texture.
- [ ] Overflow policy: >2,048 commands per frame → flush + second draw ([F3](forks.md#f3)).
- [ ] VERIFY: identity + zoom round-trip + a lod-swap burst (all-prim def swap ≈ 1,347 commands ≈
      2 flushes worst case) — and confirm the no-clear property (stale buffer tail, correct render).

## P3 · Constants row

- [ ] Window mapping (cols/rows/winCol/winRow/slot…) written via commands to row 1023; gather
      reads them by `texelFetch` instead of per-draw uniforms.
- [ ] VERIFY: pan/zoom/resize all correct (window changes ride the same command path).

## P4 · Perf + close

- [ ] Measure: commands/frame + bytes/frame during streaming, orbit, and lod-swap (compare against
      the row-span baseline numbers recorded in def-frame-anchors).
- [ ] Update `docs/work/2026-07-23-def-frame-anchors/forks.md#f5` → delivered pointer.
