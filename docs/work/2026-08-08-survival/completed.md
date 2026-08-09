# Completed — survival

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
