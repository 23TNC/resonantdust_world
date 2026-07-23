# Forks — unified data texture + scatter

_Decision points + options + which we chose + why. Chronological._

---

## F1 · prim_data: retire 2-prims-per-px → 1 px per record {#f1}

**2026-07-23 — proposed with the layout, resolve at P0.**

The 64-row band at 1024 wide = 65,536 slots at ONE texel each — same capacity the 2/px packing
bought at 256×128. One record per texel: (a) scatter commands write exactly one whole record (the
pair-wrinkle — shipping the neighbour prim's words — disappears); (b) the shader fetch drops the
odd/even half-texel select; (c) the record is 128 bits, prims use 64 today → **64 bits of
headroom** for z/facing/flags. Cost: none (the band is provisioned either way).
**RATIFIED (user) — do it in P1.**

## F2 · Which uniforms move to the constants row {#f2}

**2026-07-23 — open until P0.**

Row 1023 holds runtime constants updatable through the command path. Candidates: the toroidal
window mapping (cols/rows/winCol/winRow/slot — changes on pan/resize, read by gather + overlay),
table counts. Stay uniforms: per-draw debug toggles (`uCorridor`), anything the CPU flips
per-frame anyway. Rule of thumb: state that CHANGES VIA COMMANDS belongs in the texture; state
that selects a code path per draw stays a uniform.

## F3 · Command format, capacity + the rotating cursor {#f3}

**2026-07-23 — RESOLVED (user discussion).**

- **Format (user's separated headers, denser than the 2-px interleave):** headers carry only the
  u20 target, so 4 targets pack one header px (u32 lanes) → groups of 1 header + 4 payload px =
  **5 px per 4 commands ≈ 3,276 commands** in a 64×64 (the 2-px interleave gave 2,048). Capacity is
  a free parameter; 64 KB is nothing — NO reason to shrink the buffer.
- **Overflow:** flush-and-redraw (upload span + draw, repeat). Bursts are rare (all-prim lod swap
  ≈ 1,347 fits in ONE buffer anyway).
- **Rotating cursor (the user's ring-buffer idea, WebGL2-adapted):** rotate the write region
  through the rows across flushes/frames so a second flush never overwrites just-consumed rows —
  the driver never ghosts the command texture. The OTHER half of the classic ring (GPU progress
  cursor + CPU stall) is unnecessary in WebGL2 and stays OUT: `texSubImage2D` COPIES at call time
  (no CPU/GPU aliasing exists to guard), execution is strictly in-order (the GPU cannot observably
  fall behind), and reading a progress index back would cost a readPixels stall or frame-late fence
  poll. In WebGPU the full ring (persistent-mapped buffer + fences) maps 1:1 — the right long-term
  model; WebGL2 just lets us skip the hard part.
