# Completed — trait lights

## 2026-08-08 — P0/P1: the paper + the loader

**P0 VARIABLES.md** — the CONSTANT assignment law (§ Pawn gameplay state), the
trait-def `emit_light` per-level schema block, the thing schema's bind form
with `constant` (the `light = {}` block's row replaced), and the summary-table
lines. Verified: docs-check green.

**P1 loader** — `TraitBindToml` gained `constant` (a bind resolves to
`TraitBind { name, level, constant }`; a bare string = level 1, non-constant);
trait defs gained the per-level `emit_light` table (`TraitLight`: color/
intensity/reach/fall_off/elevation/radius/flicker/cast/hot — it participates
in the array-agreement law and can BE the level-count source for a pure light
trait); load validation refuses a non-constant bind on a non-pawn thing with
an error that names the fix ("must be `constant = true`"). `ThingDef.traits`
became `Vec<TraitBind>`; `thing_sim_version` hashes the constant bit; the
three tuple consumers (worker `mint_sidecars`, npc lib pin, the loader test)
moved to the struct. Verified: units `constant_binds_and_emit_light_tables_load`
(2-level light def loads, the bound level selects the tuple, an over-level
bind refuses) + `a_non_constant_bind_on_a_cold_thing_refuses`; 66 lib tests
green; the golden's only diff was the new empty `emit_light` field —
re-blessed deliberately (I6).
