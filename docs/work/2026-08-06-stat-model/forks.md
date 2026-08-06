# Forks — stat-model

## F1 — the four-family pawn model on packed 16+16 rows {#f1}

_2026-08-06, **user**: "we add 'stats' to our list of stuff our pawns can 'have'. So… stats,
needs, traits, conditions… I believe that we can just hold vec<u32> traits, vec<u32> conditions,
vec<u32> needs on our pawns… all needs share u4 type u12 subType and so they can be inferred…
Traits can also hold u12 kind, u4 variant so we can hold u16 level. This makes our traits…
traits/skills."_

**Chosen (user)**: within a family every row shares `type = gameplay` + the category subtype, so
a stored row is `kind:12 | variant:4` + 16 bits of family payload — trait `level`, condition
`remaining_at_write`, need `value`. Stats are the fourth family but are DERIVED ([F8](#f8)),
so only three families store. **Supersedes** the interactions stream's full-u32-def-ref +
f32-word payload entries the day after they landed — the packed form halves the words and makes
"a pawn's gameplay state" three uniform `vec<u32>`s.

## F2 — the needs SUB-TABLE keeps `set_tic`; lazy depletion survives {#f2}

_2026-08-06, **user**: "We probably want to separate our needs into another sub table so we do
not need to fan out as much data every time we update them." On the lost timestamp: "Your
implementation sounds better, lets keep that model."_

**Chosen**: needs leave the payload for a new pawn-shard sub-table whose row is
`(entity, kind:12|variant:4|value:16, set_tic)` — the table has room for the timestamp even
though the packed u32 doesn't. Lazy depletion (value-as-of-a-moment, computed on read) survives,
AND need writes fan alone instead of dragging the whole payload. **Rejected**: value-only rows
(would force ticking values in spacetime — the exact cost the sub-table exists to avoid).
Conditions/traits STAY in the payload; only needs churn enough to earn a table.

## F3 — condition time is REMAINING-AT-WRITE, not a predicted expiry {#f3}

_2026-08-06, **user**: "I was just spooked by expiry tic because we have had lots of issues
trying to predict the future. If we instead held tics remaining, and we certainly have the tic
that was written on… we can work out where we are 'now'. Both get us the same result."_

**Chosen (user)**: the condition row's 16 data bits hold `remaining_at_write`; the entry keeps
the written tic beside it. Remaining-now = `remaining_at_write − (now − written_tic)` — derived
at read, zero writes while it runs, u16 wrap under the existing half-window guard
([I6](issues.md#i6)). Backward-looking (a fact about a past write), never a forecast.
**Rejected**: a stored countdown (someone must decrement it) and a stored expiry tic
(mathematically identical, but frames the record as a prediction).

## F4 — u16 FIXED-POINT over the authored domain; f32 on the wire; ONE quantization {#f4}

_2026-08-06, **user**: "Alright we can try it."_

**Chosen**: the stored need value is u16 fixed-point — `0..65535` maps linearly onto the need's
authored `min..max` (65k steps; thirst 0..100 resolves ~0.0015). TOML keeps authoring human
decimals (rates, "drink 3") and event inputs stay f32 BIT PATTERNS in their u32 lanes; the ONE
quantize/dequantize pair lives in codec and the quantization happens at WRITE (worker/reducer),
never per-consumer ([I2](issues.md#i2)). **Supersedes** F7 of the interactions stream (stored
f32): the f32's decimals survive in authoring and transport; only the at-rest width shrinks.

## F5 — affordances hang off INTERACTIONS and are stat PREDICATES; traits are leveled stat contributors {#f5}

_2026-08-06, **user**: "I think interactions should have affordances, not pawns. At which point,
we remove variance. All pawns with the 'Walks' trait will operate the same… The trait 'Walks'
will add N to the pawns 'ground_speed' stat. I believe the affordance is something like 'Can
Move Ground', which will check 'ground_speed > 0'. Then the interaction can have the affordance
'Can Move Ground'. This way we could 'Can Move Air' or 'Can Teleport' etc… I think it might make
the most sense to hold several 'levels' in our toml. So 1: 60 tics/tile 2: 50 tics/tile 3: 40
tics/tile etc."_

**Chosen (user)**: `[[affordance]]` = a named predicate over pawn stats, structured not
stringly ([F10](#f10)); `[[interaction]]` lists `affordances = [...]` that gate it;
`[[trait]]` = a per-level table of stat contributions (and need modifiers). Affordance
`variants` and trait-list `requires` are DELETED — capability is a stat threshold, so
`can_move_air`/`can_teleport` are TOML edits. Every existing affordance must re-express as a
predicate ([I9](issues.md#i9)): `drink` gains `can_drink` ⇔ `metabolism > 0`, fed by
`biological_lifeform`.

## F6 — the combiner: max-of-mins / min-of-maxes, authored winner, global bounds {#f6}

_2026-08-06, **user**: "we assign the maximum minimum, and the minimum maximum among the pawns
traits. So if we have 3 < var < 7, and 4 < var < 8. Then… 4 < var < 7. If we have 2 < var < 3
and 4 < var < 5… maximum minimum so 4, and minimum maximum so 3. So the question becomes does 3
or 4 win. So we will hold stats in our toml as well, and define global max/min (for safety) and
min/max win. I'm guessing needs will need the same treatment."_

**Chosen (user)**: modifier ranges intersect (max of mins, min of maxes); an EMPTY intersection
is decided by the def's authored `min_wins`/`max_wins`; authored global bounds cap everything.
ONE combiner in shared/content serves stats AND need domains/rates — a condition capping a
need's max is the same machinery as a trait flooring a stat.

## F7 — the re-stamp law: modifier mutations re-stamp affected need rows {#f7}

_2026-08-06, **user**: "Sounds about right."_

**Chosen**: lazy depletion is only correct while the rate is constant over the window, so every
mutation of the modifier set (grant/expiry of a condition, trait change) RE-STAMPS the affected
need rows — write the evaluated value + the mutation tic at that moment. Expiring conditions
need no write: the expiry tic is DERIVABLE from the row ([F3](#f3)), so the eval integrates
piecewise across it ([I4](issues.md#i4)). This is the invariant the whole design leans on;
VARIABLES.md states it as law.

## F8 — stats are DERIVED only: never stored, never fanned {#f8}

_2026-08-06, **user**: "never fanned, I see no reason why we couldn't cache them and use them
only on update… but also no reason we couldn't just re-derive when we need them."_

**Chosen (user)**: a stat is `clamp(sum of the pawn's per-level contributions, combined
bounds)` (base 0), recomputed from rows + corpus wherever needed — worker, npc, wasm. Caching
is an allowed optimization, never a source of truth. Consequence: the derivation must be
bit-identical everywhere — it ships ONCE in shared/content ([I2](issues.md#i2)).

## F9 — carrier bindings stay TOML: `interactions = [{ name, magnitude }]` {#f9}

_2026-08-06, **user** (on packing drink's magnitude into a u16 beside its id): "because we need
more data for our operations anyway… and therefore hold count… Then… I am uncertain if we
should bother."_

**Chosen**: don't bother — resolved against packing. Carriers are STATIC CORPUS, not spacetime
rows; the packed 16+16 trick only pays where rows live in tables. The carrier authors
`interactions = [{ name = "drink", magnitude = 3 }]` (replacing the `affordances = [...]`
spelling, per [F5](#f5)) and the magnitude rides the event as an f32 input exactly as today.
Drink 3 vs drink 5 per carrier — the original motivation — survives untouched.

## F10 — predicates are STRUCTURED, not expression strings {#f10}

_2026-08-06._ The affordance authors `check = { stat = "ground_speed", above = 0.0 }`
(`above`/`below`, exclusive) — a struct the loader validates against the stat registry, not a
parsed expression language. **Rejected**: `requires = "ground_speed > 0"` strings — an
expression grammar is a parser, an error surface, and an injection lane, for zero present
expressiveness. If predicates ever need composition, that's a schema extension, not string
parsing.

## F11 — trait rows mint at CREATE from the def; runtime grant/revoke is a successor {#f11}

_2026-08-06._ Thing defs bind starting traits (`traits = ["biological_lifeform",
{ name = "walks", level = 1 }]` — a bare string means level 1) and CREATE mints the rows.
Interactions adding/removing traits ("interactions can indirectly modify…", user) is REAL
future intent — the schema holds it — but the verbs (`GRANT_TRAIT`/`REVOKE_TRAIT`) are a
recorded successor, not this stream: nothing in the drink/move corpus needs them yet, and
conditions already prove the grant lane end to end.

## F12 — walks is AUTHORED and derived this stream; movement rewires in the input stream {#f12}

_2026-08-06._ This stream authors `walks` + `ground_speed` + `can_move_ground` and proves the
derivation (tests + npc log), but the worker's movement chain and MoverLayer keep reading the
`speed` field until the input stream replaces MOVE_TO with the `move_to` interaction. One
stream, one seam: rewiring movement here would drag the pie menu's front door in with it.
The two-source window is guarded ([I10](issues.md#i10)), and `speed` dies WITH the rewire
(delete-don't-deprecate), not before.

## F13 — a DERIVED condition may not modify needs (the circularity cut) {#f13}

_2026-08-06, resolved during P1._ A band (DERIVED) condition's liveness is computed FROM need
evaluation; letting it carry a `needs` modifier would feed the thing that decides it — a
fixpoint the lazy eval cannot host (and each band flip would be an unstamped rate change,
breaking [F7](#f7)). The loader REFUSES `duration = 0` + `needs = [...]`; stat contributions
on derived conditions remain legal (stats don't feed the needs eval). If a "Dehydrated slows
you" style rule ever needs the reverse — a band condition slowing a DIFFERENT need — that's a
design conversation about stamping band crossings, not a validation to delete.
