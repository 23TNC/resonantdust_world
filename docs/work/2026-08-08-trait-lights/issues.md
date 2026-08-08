# Issues — trait lights (anticipated; logged before they bite)

## I1 — the trait-level reader sweep must be EXHAUSTIVE {#i1}

Every `gameplay_row_data` call on a TRAIT row changes meaning under the split
(the u16 now carries `data:8 | level:8` — a reader that keeps the u16 sees
level + 256×data). Known readers to move to the new accessors: stat_eval's
trait levels, the emotions per-level lookup, inventory capacity (= trait
level), the food-chain HIGHEST-wins caps, `mint_sidecars` + `payload_traits`,
npc brain trait reads, and the webgl panels/tooltips that print levels. The
ONE-eval law concentrates most of these in shared/content — the sweep is
grep-driven and the acceptance is "no raw `gameplay_row_data` on a trait row
anywhere".

## I2 — the presentation residue moves to the trait DEF {#i2}

Today's light block carries r/g/b + intensity, reach, radius, height, flicker,
cast/hot flags. The trait bind carries only color-level + reach-data; the REST
is authored ONCE on the `emit_light` def (per level where it varies — both
torches share radius/height/flicker today). A kind that one day needs a
different flicker is a NEW LEVEL (append-only), not a per-bind override — the
bind stays two bytes. Stated now so nobody grows bind-side override syntax.

## I3 — cold baking is FREE — do not build a cold field for it {#i3}

Cold thing rows are already kind-keyed (`kind_pos_ref`); the client's
`lightFor(kindId)` already derives light per kind. Moving the SOURCE from
`visual.light` to the constant trait changes no wire format and no cold row —
"baking lights into cold" is the corpus lookup we already do. The drill is a
before/after screenshot, not a schema change.

## I4 — `constant` enforcement points, named while no write path exists {#i4}

No verb writes trait rows today (payload rows mint at spawn and are read-only
since). Enforcement is therefore: (1) the LOADER — a non-constant bind on a
thing refuses; (2) the ACCESSOR — constant assignments never come from
payload, so a forged payload row for a constant-bound trait is ignored (the
collision rule, F5); (3) the LAW IN PRINT — any future trait-writing verb must
check the def's constant binds and refuse. Written here so the check isn't
forgotten when that verb arrives.

## I5 — the carried torch must NOT glow {#i5}

Inventory F6's comment on the torch stands: picking a torch up takes its light
out of the world. Lights derive from RENDERED things — an inventory item has
no world prim, so the light vanishes naturally. But the pawn now CAN glow via
its own emit_light — verify a torch-carrying pawn without the trait stays
dark (no accidental "carried items contribute traits" merge).

## I6 — content schema changes ride the FULL consumer sweep {#i6}

shared/content is consumed by wasm + npc + worker (+ master's seed path).
TraitBindToml gaining `data`/`constant` and ThingDef's traits changing shape =
rebuild wasm bundle, webgl typecheck, npc, worker, master; the golden fixture
guards the tables (BLESS_GOLDEN to re-bless, deliberately). Miss one and the
failure is a silent stale-binary drift (the sim guard screams only on hash
mismatch).

## I7 — the emotions per-level arrays must survive the split untouched {#i7}

Emotions read trait LEVEL to index their 1..15 contribution arrays. Post-split
the level is the LOW byte — the accessor keeps their indexing identical, and
the emotions goldens (argmax, tie-break) are the regression net. A drift here
shows as wrong emotion cards, not a crash — check them explicitly.

## I8 — pawn hot lights are a NEW warm-class light source {#i8}

A glowing pawn attaches a light to a MOVER (hot light, follows the render
position) — the primitive graph carries light pieces and pawn-render's
warm/hot classing exists, but no mover-attached light has shipped. Watch: the
light must ride the CHASE position (rx/ry, not auth), zoom reprojection uses
the TEXTILE_SLOT rules, and per-light lod law ("coarsest-since-cast") applies
to a light that never stops moving. Budget one drill purely for this.

## I9 — trait binds on pawn kinds must not silently become constant {#i9}

Pawn kinds author starting traits today (walks, corpus, bite…) that mint
PAYLOAD rows — runtime-modifiable by design intent (leveling). The constant
flag is OPT-IN per bind; the migration marks NOTHING constant except the
torches' emit_light. A later pass may promote species invariants
(biological_lifeform) to constant for payload savings — successor, not now.
