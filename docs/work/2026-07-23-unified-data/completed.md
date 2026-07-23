# Completed — unified data texture + scatter

_Done **and** verified (identity diff + zoom round-trip). Items move here from
[`todo.md`](todo.md). Append-only history; authoritative for what's done._

---

## P0 · Authoritative layouts — 2026-07-23 ✓

- `docs/VARIABLES.md` rewritten: the unified 1024×1024 RGBA32UI data texture (linear index
  `i → (i & 1023, i >> 10)`; bands defs 0–63 / prims 64–127 / lights 128–191; rows 192–1022
  reserved; row 1023 constants) + the command format. F1 ratified (1 px/prim — 64 bits headroom,
  the record is 128 and prims use 64); F2 decided by the rule of thumb (window mapping → constants
  row; per-draw debug toggles stay uniforms); F3 resolved (dense headers, flush-and-redraw,
  rotating cursor — the ring's readback half stays out under WebGL2's copy-on-upload + in-order
  semantics).

## P1 · Merge (CPU transport) — 2026-07-23 ✓

- Three textures + mirrors → ONE texture + ONE 16 MB mirror + one dirty row span. prim_data
  1 px/record (R position, G orient, B/A reserved) — half-texel select + scatter pair-wrinkle die.
  Gather binds `uData` (7 → 5 samplers); GLSL `fetchLin(t, i)` with band-base consts.
- Verified: identity 0 mismatches; zoom round-trip mints {lod0, lod5, lod6}; nonzero exactly 9,949;
  all bands decode via the debug readers.

## P2 · Scatter transport — 2026-07-23 ✓

- 64×64 command buffer (5-px groups, ~3,276 commands/fill), ONE row-span upload at a ROTATING
  cursor + ONE point-scatter draw per flush (vertex: slot → header lane → point at target texel;
  fragment: payload write; reads only the command buffer). Replay-idempotent absolute writes — no
  clearing. Oversized frames batch. Engine: draw-count override + RenderTarget `wrap`.
- Verified: same battery, entirely through scatter — 0 mismatches, 9,949 nonzero, three lod
  generations, idle/orbit at the 121 display cap (the lone "1 fps" probe was rAF
  occlusion-throttling during automation, disproven by instrumented flush = 0 ms + re-measure).

## P3 · Constants row — 2026-07-23 ✓

- Window mapping + slot/light-count in row 1023 (i32-in-u32 lanes), compare-written via the same
  command path; gather + overlay texelFetch them (window uniforms die; overlay binds uData).
- Verified: pan/zoom/resize correct (the toroidal windowing renders right at both zooms).

## P4 · Command format v2.1 (user) + perf close — 2026-07-23 ✓

- **v2.1 — self-addressing records** (user): every record carries its **u16 in-set id in R's high
  half**, so scatter commands are PURE PAYLOADS — fills are `1 header + Σn` px (≈1.06 px/command).
  Header: 15× u6 per-set counts (5 per RGB lane) + count₁₅ and the **u8 opcode** in A (0 =
  write-data; presence/other maps ride future opcodes). Record layouts re-cut: def R = id|offset_x,
  G = W|H|span|offset_y; prim R = id, G = position, B = orient; light R = id, G = position,
  B = colour, A = z|reach|emitter|cast (emitter folded from its own word); constants = ONE px
  (id|cols · rows|slot · i16 winCol|winRow · light_count).
- **Verified**: identity 0 mismatches; zoom round-trip {lod0, lod5, lod6}; nonzero exactly 9,949;
  **orbit 121 fps** (display cap); light/def/prim debug decodes correct under v2.1.
- **Perf numbers**: shadow tick ≈ 1 ms, flush ≈ 0 ms CPU; a fill is ≤18 rows of upload + one point
  draw; the all-prim lod swap (≈1,347) batches as ~22 fills of microseconds each.
