# Completed — survival

## 2026-08-08 — P4 control arc + P5: the truth

**P4 the control arc (I6)** — the fed cast soaked ≥30 minutes with 5-minute
population samples: 4 → 5 at t+5m (the DELIBERATE `/spawn human_female` for
the I7 assertion) then FLAT at 5 through t+30m — no unforced deaths; the
wolf and warren sustain themselves against the drains. Soak log:
`start 4 / t+5m..t+30m = 5 / DONE`.

**P5 the truth** — memory `survival-delivered` + the MEMORY.md line written;
docs-check green (the 4 standing warnings predate the stream). The stack
BOUNCE: `rd redeploy --run` (which RESETS the shards — the drill world,
meats included, was wiped by design), gateway re-deployed (its binary
did not ride `redeploy --run` back up — the npcs exhausted retries against
the dead gateway and needed a re-run), then master/orchestrator/worker/
wolves/bunnies restarted. Arcs green post-bounce: login→anchor→generate→
render in the browser (glyphs live on the fresh seed), the bunnies brain
adopts + moves, the wolves brain issues trips, and the crossing scheduler
RE-DERIVED slots for every fresh pawn from nothing (I4's bounce-durability
claim observed live: fires at the +20,000 horizon clamp for the full cast).
The user's eyes close the stream.

## 2026-08-08 — P3/P4: the crossing scheduler + the drills

**P3 scheduler (D1: RESTAMP_NEED verb 18)** — a queued literal SET_NEED
cannot re-validate, so the worker gained the worker-only re-stamp verb
(`[Write, Imm]`, never in CLIENT_VERBS; full new-verb ritual run). The
ensure pass re-derives every live pawn's floor crossing per pass
(`drain_target_needs()` from the corpus; mint coverage and bounce
durability fall out free — I4), keeps one slot per (pawn, need) with
earlier-wins supersession (I2), clamps the horizon at 20,000 tics (I8),
and floors the fire at master+4. The re-stamp arm evaluates the need FRESH
at processing and feeds the SAME death sweep a SET_NEED feeds. Verified
live: differentiated per-pawn crossings scheduled (horizon-clamp wraps
included, e.g. fire=791 = 46327+20000 mod 2^16).

**P3 death hookup** — drilled on a band-forced bunny (0x30800002, hunger +
thirst → 5.0): the crossing SUPERSEDED 55879→47750, the re-stamp wrote
value=0.0 at exactly the predicted tic, the need-write trigger fired death
ONCE, the pawn was removed, the meat SET composed (cold overlay), and the
extra queued death orders no-op'd ("target is not a live pawn" — I3).

**P4 the pens** — the wolf drill (0x30800000, both needs forced to 5.0):
crossing re-scheduled to fire=50313, the re-stamp landed value=0.0 AT
50313, death fired once, removal fanned, meat dropped — the death site
wears its M glyph on camera beside the bunny drill's M (capture:
bunnydeath). The bunny dehydration leg is the P3 drill above (forced band
= the pen equivalent, recorded honestly: no physical pen was built —
forced needs stand in for denial).

**P4 humans inherit (I7)** — a fresh `/spawn human_female` band-forced to
hunger 5.0: the Starving condition card appears in her details panel
(tooltip: +4 Scared, +3 Uncomfortable, priority 30, corpus modifier
listed) and the scheduler re-scheduled her corpus crossing from the far
horizon to fire=53772 the moment starving activated. Fed back to 90:
NO new schedule line — earlier-wins keeps the stale slot, which
re-validates harmlessly at fire (the exact D1 rationale, observed).

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
