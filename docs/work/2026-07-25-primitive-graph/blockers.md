# Primitive graph — blockers

_Needs your input before P1 writes code. Open → resolved (archive resolved with a date)._

## B1 — Four layout semantics ✅ RESOLVED by the user (2026-07-25)
1. **Child offset signedness** → **bias-8** per nibble (−8..+7).
2. **CPU-resolved vs GPU-walked** → **both possible** via `u16 parent_id` on leaves + a `child` bit on
   `prim_data`; resolved further in B2 ([F1](forks.md#f1)).
3. **`rotation`/`layer` precedence** → dissolved: the shader uses the **definition's** rotation; stored
   rotation is a CPU **reconciliation signal** driving definition swaps ([I4](issues.md#i4)).
4. **Header count width** → dissolved by **fixed 8-px commands** ([I5](issues.md#i5)).

## B2 — The revision's two items ✅ RESOLVED by the user (2026-07-25)
1. **`emitter_radius` restored** — `light_data ALPHA: u12 reach | u8 radius | …` ([I10](issues.md#i10)).
2. **Resolved position: the CPU stamps it.** The leaf spends its whole RED lane on
   `u16 parent_id | u8 resolved_tile | u8 resolved_unit`; resolution walks root → child offsets → leaf.
   The authored offsets stay in GREEN. The GPU never walks the graph in the hot loop
   ([I11](issues.md#i11)). Also settled: **`id = 0` is the global sentinel**, so partial commands pad
   with zeros ([I7](issues.md#i7)), and `inherit_rotation` moves to `definition_data` + `prim_data` with
   **one-step** inheritance.

## B3 — `resolved_zone` alongside resolved tile/unit ✅ RESOLVED by the user (2026-07-25)
**Resolved.** The user supplied the deciding reasoning: `light_presence` is a **reach** relation (a light
knows exactly which tiles it affects, so it registers itself on all of them) while `billboard_presence`
is a **containment** relation (shadows are combinatorial — every billboard × every light — so a
billboard can only register the tiles it *occupies*, and the gather projects the shadow from there).
That is precisely why a light needs `resolved_zone` and a billboard does not. `u8 resolved_zone` is in
`light_data` ALPHA, landed in VARIABLES at P0, and **in use since P2c** — `resolvedPos()` reconstructs a
light's absolute position from `zone|tile|unit` by nearest-congruent (256-tile period), and
`resolvedTilePos()` is its 16-tile-period sibling for the containment case.

_(Left marked OPEN long after it was settled, which silently released the continuation hook — see
[I21](issues.md#i21). Close a blocker in the commit that closes it.)_

<details><summary>original analysis</summary>

`resolved_tile` is `x:4|y:4` = the **in-zone** tile, so reconstructing a light's
absolute position from a fragment has a **16-tile period** — unambiguous only within **8 tiles**. But
`LIGHT_REACH` is **12 tiles** today (and the `u12 reach` field allows far more), so a light 12 tiles north
would reconstruct as 4 tiles south. The gather genuinely needs the absolute position: it computes
`toL = Lxy - P` for falloff/N·L and marches the corridor from the light. This is the same
position-ambiguity class as the two zoom regressions, so I don't want to build on it.

**Proposed fix (already applied in the README, needs your yes):** also stamp **`u8 resolved_zone`**
(in-region zone, `x:4|y:4`). Zone+tile+unit → a **256-tile** period, unambiguous for any reach under 128
tiles; region stays inferred by nearest-congruent, exactly as the region-torus fold already does. It
costs nothing structurally — the room is in reserved space:
- `light_data`  ALPHA: `u12 reach | u8 radius | u8 resolved_zone | u4 reserved`
- `billboard_data` ALPHA: `u8 resolved_zone | u24 reserved`

**Alternative if you'd rather not spend the bits:** cap effective light reach below 8 tiles (currently
12) so the 16-tile period is unambiguous — but that's a gameplay/visual constraint imposed by an
encoding, which seems like the wrong trade.

</details>

_No open blockers. The stream is executable._

**Suggested path:** confirm `resolved_zone`, and I'll write the full layouts into VARIABLES at P0 and
build P1 — the command transport alone, provable pixel-identical before any record layout moves.
