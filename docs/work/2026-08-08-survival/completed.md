# Completed — survival

## 2026-08-08 — P2: the drain lane

**shared/content** — `deplete` on NeedModifier (conditions + the trait
per-level form); `RateWindow` gained the `add` lane (absolute units/tic,
SUMMING; multipliers scale the base only); the cross-need DERIVED drains
compose in `rate_windows` (a source need's band condition draining a target —
the in-band interval derived from the source's own trajectory at depth 1,
converted between row frames by the signed wrapped stamp offset);
`satisfaction_at`/`next_crossing_tic` learned that a deplete-0 need moves
under adds; NEW `floor_crossing_tic` (the scheduler's read). Stat-model F13's
guard REFINED: derived conditions still may not author rate/min/max, but a
`deplete` on ANOTHER need is the survival-F3 exception — with the ACYCLICITY
GUARD refusing self-drains and depth-2 chains at load. Verified: the pinned
trajectory test `deplete_modifiers_drain_and_predict_the_floor` (single drain
2.0→1.0 over the derived window; summed drains 0.4; floor crossing exactly
1200; the undrained fast path untouched); 70 lib tests green; golden
re-blessed (the new field + the drains).

**content** — `starving` and `dehydrated` each author
`{ need = "corpus", deplete = 3600 }` (~10 min full→dead; both = twice the
pace); corpus's comment updated to the new law. Load + content-check green.

**the sweep** — wasm + webgl + npc (wolves' rate_windows call gained the
need rows) + worker (both call sites) + master rebuilt; stack restarted on
the drain corpus; seed guard quiet at 237; wolf re-adopted.

## 2026-08-08 — P0/P1: the paper + geo glyphs

**P0 VARIABLES.md** — `geo_label` on the thing schema, the `deplete`
DEPLETION MODIFIER on condition need-modifiers (F3, the user's shape), and
THE CROSSING RE-STAMP LAW beside the re-stamp law it extends. docs-check
green.

**P1 loader** — `geo_label` on ThingToml/ThingDef, `thing_geo_labels()`
(authored wins; default = the name's first char uppercased) + the wasm
`thingGeoLabels` export. Verified: unit `geo_labels_default_and_override`
(bunny → "B", logs' authored "Lg" wins); 69 content tests green, golden
unchanged.

**P1 client** — the glyph substitutes the WHITE FILL in the albedo resolve
(`solid(glyph)` wherever `solid(wf)` served): dark-on-white because the bake
multiplies residual × geoColor — the ground takes the box color, the letter
survives on any tint (F2's white-on-dark lettering FLIPPED for the multiply
pipeline, recorded here). Cached per label on an offscreen canvas;
`geoLabel` rides Primitive/PrimitiveSpec through the explicit addPrim copy
(the gotcha comment added); set by WorldBridge's SEED path, WorldBridge's
OVERRIDE path (found live: spawned things — logs, meat, torches — were
blank until the second site got the field), and MoverLayer's CARRIER slot
only (no doubled letters on two-part pawns). A partially-loaded real albedo
still outranks the glyph, so letters never stamp real art; the pack
lifecycle (the geo-flash law) is untouched by construction — only the
residual SOURCE for geo resolutions changes. Verified live: S shrubs,
T torches (warm + blue), L logs (details panel confirms "logs" at the
selected box), B bunnies, R rocks/reeds; conifers/flora with real art show
no letters. Captures: glyphs2 (the clearing), bunnyb (the warren).
