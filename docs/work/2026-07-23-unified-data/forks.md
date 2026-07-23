# Forks — unified data texture + scatter

_Decision points + options + which we chose + why. Chronological._

---

## F1 · prim_data: retire 2-prims-per-px → 1 px per record {#f1}

**2026-07-23 — proposed with the layout, resolve at P0.**

The 64-row band at 1024 wide = 65,536 slots at ONE texel each — same capacity the 2/px packing
bought at 256×128. One record per texel: (a) scatter commands write exactly one whole record (the
pair-wrinkle — shipping the neighbour prim's words — disappears); (b) the shader fetch drops the
odd/even half-texel select; (c) 128 bits per prim (64 today) = headroom for z/facing/flags.
Cost: none (the band is provisioned either way). **Leaning: do it in P1.**

## F2 · Which uniforms move to the constants row {#f2}

**2026-07-23 — open until P0.**

Row 1023 holds runtime constants updatable through the command path. Candidates: the toroidal
window mapping (cols/rows/winCol/winRow/slot — changes on pan/resize, read by gather + overlay),
table counts. Stay uniforms: per-draw debug toggles (`uCorridor`), anything the CPU flips
per-frame anyway. Rule of thumb: state that CHANGES VIA COMMANDS belongs in the texture; state
that selects a code path per draw stays a uniform.

## F3 · Command-buffer overflow policy {#f3}

**2026-07-23 — open until P2.**

2,048 commands per 64×64 buffer-full. Options when a frame exceeds it: (a) flush-and-redraw
(upload span + draw, repeat — simple, unbounded, extra draw calls only on burst frames);
(b) bigger buffer (128×128 = 8k commands, 256 KB); (c) spill the remainder to next frame
(NO — delays visible state). **Leaning (a)**: bursts are rare (all-prim lod swap ≈ 1,347), and
two draws on a burst frame is nothing.
