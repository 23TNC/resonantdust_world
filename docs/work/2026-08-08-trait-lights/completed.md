# Completed — trait lights

## 2026-08-08 — P3: the torch converts, the light block dies

**content** — the `emit_light` trait def APPENDED (order-is-law; the master
reseeded 234 → 235 with the guard quiet): two levels — warm `[1.0, 0.85,
0.55]`, blue `[0.55, 0.75, 1.0]`, both reach 16 / radius 0.35 / elevation 2.5
/ flicker, colors as FLOAT TRIPLES to hold the retired block's values exactly
(the loader's color accepts hex or `[r,g,b]`). torch binds level 1 constant,
torch_blue level 2; every `light = {}` block DELETED — `LightToml`,
`VisualParts::light` and `LightParts` died with it.

**`thing_light()` derives** — same stride-8 shape, first `object_lights`
tuple per kind (`elevation` rides the old `height` slot). Verified: unit
`thing_light_derives_bit_identically` pins BOTH torches' vectors to the old
block's exact values (`[1.0, 0.85, 0.55, 1.0, 16.0, 0.35, 2.5, 5.0]` /
`[0.55, 0.75, 1.0, …]`) — every downstream consumer untouched; 68 content
tests green; golden re-blessed (the appended trait).

**The consumer sweep + live** — wasm/webgl/worker/npc/master rebuilt (a stale
npc caught live: it refused the float-color corpus — the color enum predated
its build; I6 doing exactly its job), stack bounced onto the new corpus, seed
guard quiet, brains re-adopted. Live capture: both torch kinds GLOW from
their constant binds — warm pool at (99,51), cool pools at the three blue
torches. Evidence swap recorded on the todo item: the corpus serves live from
the working tree, so the pixel-diff acceptance became the unit pin + a
working-glow capture.

## 2026-08-08 — P2: the ONE merged-traits accessor

**`object_trait_rows` + `object_lights`** on the Bundle (F5/F4/F8): constant
binds derive as packed rows (constant wins a key collision — a forged payload
row is ignored), lights yield per merged trait in authored order, nothing
drops. Verified: unit `the_merged_accessor_derives_constants_and_lights`
(constant appears with payload absent; the forged row is shadowed; the torch
yields its bound tuple; five light traits yield five).

**The reader sweep (I1)** — every trait-row source now feeds the accessor:
worker (a `pawn_gameplay_rows` helper replaced the four inline payload
decodes; kind from the pawn's own state row), npc (wolves ×4 + bunnies
`rows_of`), wasm (`decode_payload` takes bundle+kind; `pawnConditions`,
`pawnEmotion`, `pawnNextCrossing`, `pawnGroundSpeed`, `needMax` and the three
menu filters gained the pawn-kind param), webgl callers updated (details
panel, menus, `speedFor`). New wasm export `objectLights(kind, payload)` —
stride 9 per light. Verified: the acceptance grep shows every remaining raw
`payload_traits` call is an INPUT to the accessor; 67 content tests green
(emotions goldens untouched — I7); webgl tsc clean.

**`mint_sidecars` skips constant binds** — the row joins the eval set for the
need-cap mint but never the stored payload. Verified: worker unit
`mint_sidecars_skips_constant_binds` (one stored row for the non-constant
bind; the constant one reaches eval + its light through the accessor); the
three worker test fixtures gained pawn taxonomies (the F6 refusal now guards
them too); 4 worker tests green.

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
