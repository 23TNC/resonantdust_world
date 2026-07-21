# Issues — cold-data-textures

_Problems hit + candidate solutions + which we chose + why._

---

## I-1 · The shader radius safety check over-culls (disabled) — 2026-07-21

**Problem:** P4's F5 shader radius check — `if (abs(A.x−L.x) > Lradius || abs(A.y−L.y) > Lradius) degenerate` —
culled **every** caster (blank shadows), not just out-of-range ones. `A` (caster) + `L` (light) decode
correctly (the fan renders in the right place with the check off) and `Lradius` reads back ≈64 units, so the
maths *should* pass in-range casters — but it degenerates all. Root cause not yet pinned (a subtle
unit/precision mismatch in the shader compare, distinct from the CPU cull which works).

**Chosen:** disable it for now. It's a **safety for STALE LUT entries** (a caster that moved out of range
before its light's run was patched), and we currently **fully rebuild** the LUT on every change — so entries
are always fresh + already CPU-culled, making the check redundant. Re-enable (with a fix) when incremental LUT
patching (F5) can actually leave a stale entry. Kept in the shader, commented, with `Lradius` referenced so it
doesn't drop out.
