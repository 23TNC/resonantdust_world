# Primitive graph — blockers

_Needs your input before P1 writes code. Open → resolved (archive resolved with a date)._

## B1 — Four layout semantics ✅ RESOLVED by the user (2026-07-25)
1. **Child offset signedness** → **bias-8** per nibble (−8..+7). Agreed.
2. **CPU-resolved vs GPU-walked** → **both**, via `u16 parent_id` on leaves + a `child` bit on
   `prim_data`; CPU resolves presence/bucketing, GPU can walk when required
   ([F1](forks.md#f1)). One sub-decision remains → B2.
3. **`rotation`/`layer` precedence** → dissolved: **the shader always uses the definition's rotation**;
   stored rotation is a CPU **reconciliation signal** driving definition swaps, with `parent_rotation`
   (1 bit) opting into carrier inheritance. `layer` needs no rule — one object per layer, and
   `prim_data` carries none ([I4](issues.md#i4)).
4. **Header count width** → dissolved by **fixed 8-px commands** (opcode + set + 7 ids + 7 payloads),
   56 record-writes per row ([I5](issues.md#i5)).

## B2 — Two items from the revision (2026-07-25, OPEN)
1. **⚠ `light_data` lost `emitter_radius`** ([I10](issues.md#i10)). The revised ALPHA reads
   `u12 reach | u20 reserved`; the first spec had `u8 radius`. It is **load-bearing** — it drives the
   16-tap area-light penumbra from the delivered [penumbra](../2026-07-23-penumbra/README.md) stream, and
   `casterCover` falls back to a **hard quad** when `emitter < 0.5`
   ([`shadowGather.ts:225,257`](../../../client/webgl/src/game/viewport/shadowGather.ts)). Without it every
   soft shadow goes hard. **Assumed an oversight — restored in the README as
   `A: u12 reach | u8 radius | u12 reserved`.** Confirm, or tell me penumbra is being retired.
2. **Where does a leaf's resolved position live?** ([I11](issues.md#i11)) The gather's light loop runs
   per-texel-per-light, so walking parent chains *there* multiplies the hottest loop in the renderer.
   The CPU already computes each light's world position to build presence. **Lean: the CPU stamps the
   resolved absolute position into the leaf** — which needs a u32 home. Options: use RED's reserved
   (`u15` after `parent_rotation`) for `region|zone` and treat `tile_offset`/`unit_offset` as *resolved*
   tile/unit post-resolve; or carve it from ALPHA's reserved. Your call on the bit-home, since it's a
   layout semantic. (Alternative: accept a GPU walk with a documented `MAX_DEPTH`.)

**Suggested path:** confirm the `radius` restore + pick the resolved-position home, and I'll write all
the layouts into VARIABLES at P0 and build P1 — the command transport alone, which is independently
provable pixel-identical before any record layout moves.
