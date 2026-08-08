# Issues — trait lights (anticipated; logged before they bite)

## I1 — the trait readers move to the merged accessor, exhaustively {#i1}

Every "what traits does this object carry" read must go through the ONE
accessor (F5) or constant binds are invisible to it. Known readers: stat_eval's
trait levels, the emotions per-level lookup, inventory capacity (= trait
level), the food-chain HIGHEST-wins caps, affordance Tag checks,
`mint_sidecars` + `payload_traits`, npc brain trait reads, and the webgl
panels/tooltips. The row FORMAT does not change (F3 — the split was
withdrawn), so this is a call-site sweep, not a decode migration.

## I2 — presentation lives on the DEF's per-level table, never the bind {#i2}

A bind is `(name, level, constant)` — two words of intent. Color, intensity,
reach, fall_off, elevation, radius, flicker all ride the def's `emit_light`
level entries. A kind that needs a different flicker is a NEW LEVEL
(append-only), not a per-bind override — stated now so nobody grows bind-side
override syntax.

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
its own trait — verify a torch-carrying pawn without a light trait stays dark
(no accidental "carried items contribute traits" merge).

## I6 — content schema changes ride the FULL consumer sweep {#i6}

shared/content is consumed by wasm + npc + worker (+ master's seed path).
TraitBindToml gaining `constant`, trait defs gaining `emit_light` tables, and
ThingDef's binds changing shape = rebuild wasm bundle, webgl typecheck, npc,
worker, master; the golden fixture guards the tables (BLESS_GOLDEN to
re-bless, deliberately). Miss one and the failure is a silent stale-binary
drift.

## I7 — the emotions per-level arrays must survive untouched {#i7}

Emotions read trait LEVEL to index their 1..15 contribution arrays. The level
encoding is unchanged (F3), but the READ moves to the merged accessor — the
emotions goldens (argmax, tie-break) are the regression net. A drift here
shows as wrong emotion cards, not a crash — check them explicitly.

## I8 — pawn hot lights are a NEW warm-class light source {#i8}

A glowing pawn attaches a light to a MOVER (hot light, follows the render
position at its authored ELEVATION) — the primitive graph carries light
pieces and pawn-render's warm/hot classing exists, but no mover-attached light
has shipped. Watch: the light must ride the CHASE position (rx/ry, not auth),
zoom reprojection uses the TEXTILE_SLOT rules, and the per-light lod law
("coarsest-since-cast") applies to a light that never stops moving. Budget one
drill purely for this.

## I9 — no existing bind becomes constant in the migration {#i9}

Pawn kinds author starting traits today (walks, corpus, bite…) that mint
PAYLOAD rows — runtime-modifiable by design intent (leveling). The constant
flag is OPT-IN per bind; the migration marks NOTHING constant except the
torches' light trait. A later pass may promote species invariants
(biological_lifeform) to constant for payload savings — successor, not now.

## I10 — `fall_off` is authored now, consumed by a successor {#i10}

The user names fall_off (and future direction) among the light variables. The
schema accepts and carries it from day one; the LIGHTING RENDER currently has
no falloff parameter per light (the fine lightmap bakes N·L; attenuation shape
is the shader's). Wiring fall_off into the bake/blit is a lighting-stream
change, deliberately out of scope here — the value rides the corpus and the
stride vector so the successor only touches the shader. Stated so an authored
fall_off silently doing nothing is a KNOWN state, not a bug report.
