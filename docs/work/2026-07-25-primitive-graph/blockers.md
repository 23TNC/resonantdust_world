# Primitive graph — blockers

_Needs your input before P1 writes code. Open → resolved (archive resolved with a date)._

## B1 — Four layout semantics the records don't yet state (2026-07-25, OPEN)
The bit layouts are complete and self-consistent (every lane sums to 32 — verified). These are the
*semantics* the layouts imply but don't fix, and each one is a decision a reader and a writer must agree
on before either exists:

1. **Child offset signedness** ([I2](issues.md#i2)/[F3](forks.md#f3)) — **⚠ blocks everything.** As
   literally specified, `tile_offset`/`unit_offset` are unsigned, so a child can only be placed **+x/+y**
   of its carrier: no left hand, no torch held to the left, no light centred on a sprite. **Lean: bias-8**
   per nibble (−8..+7 tiles / units).
2. **[F1](forks.md#f1) — CPU-resolved vs GPU-walked graph** (the crux). **Lean: CPU-resolved**; the GPU
   keeps its flat one-hop read, the graph lives in `prim_data` for updates. Determines
   [I1](issues.md#i1) (how a carried record learns where it is) and [I3](issues.md#i3) (hot-loop cost).
3. **`rotation` / `layer` precedence** ([I4](issues.md#i4)) — both appear at 3 levels with no stated rule
   (unlike `hot_cold`/`cast_shadows`, which you defined). Plus: does a carrier's rotation **re-face**
   children (billboard frame select) or **orbit** them geometrically? If it orbits, left/right hands must
   swap on an E↔W flip.
4. **Header count width** ([I5](issues.md#i5)/[F4](forks.md#f4)) — `u3` can't express the 8th group, so a
   set caps at 56 records/fill and the last group of a full fill is unaddressable. **Lean: `u4`** (still
   one header px, and leaves room for the opcode, [I6](issues.md#i6)).

**Why these need you:** 1 and 3 change what the bits *mean* (VARIABLES is authoritative and outranks
code); 2 sets the architecture the rest of the stream is built on; 4 is mechanical but is a layout edit.

**Suggested path:** take the leans on 1 (bias-8), 2 (CPU-resolved), 4 (u4) and answer 3 (I'd guess
"rotation re-faces, doesn't orbit" — but that's yours), and I'll write all of it into VARIABLES at P0 and
build P1 (transport-only, independently verifiable) before touching a single record layout.
