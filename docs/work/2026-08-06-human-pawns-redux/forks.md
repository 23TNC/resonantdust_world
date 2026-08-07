# Forks — human-pawns-redux

## F1 — re-implement the two-prim human on the CURRENT stack {#f1}

_2026-08-06, **user**: "We are going to re-implement the human pawns. I am uncertain what
state human pawns are in currently. Essentially human pawns are multiple prims currently one
for the head and one for the body."_

**Chosen (user)**: the head+body multi-prim model stands; "re-implement" means making it
LIVE again on everything that landed since the original stream (packed gameplay rows, the
interaction door, derived speed, the pie menu) — repairing the audit's four rot points, not
rebuilding the skeleton, which survived intact.

## F2 — the mint affordance is a CHAT command {#f2}

_2026-08-06._ `spawn <thing> [x y] [body N] [head N]` in the existing chat-command registry
(beside `grid`/`focus`/`zoom`), composing `PROMOTE CREATE def pos count PART(0, body_def)
PART(1, head_def)` through `queue` — the same allowlisted door everything else uses; x/y
default near the camera focus, variants default 0. **Rejected**: a URL param (one-shot,
unrepeatable mid-session), a build-panel entry (humans are not buildings), and reviving the
`Humans` npc brain (the original stream's user-F6 reversion stands — the fixture is placed
and player-driven, not brained).

## F3 — per-pawn appearance rides the PART entries' VARIANT nibbles {#f3}

_2026-08-06._ ONE `human_female` def; a pawn's body choice (`0..8`) and head choice
(`0..15`) are the variant nibbles of its two `PART` def refs — the original stream's
"u4 body/head variants in spacetime" intent, realized through machinery that already exists
end to end: `part_entry(slot, def)` stores it, the fan carries it, `variantOf(d) = d & 0xf`
decodes it, and `moverSlotTexture` already probes `<base>/<variant>/<facing>.<part>` stems.
The art's asymmetry (9 bodies, 16 heads) is validated at the COMPOSER (the spawn command
refuses an out-of-range variant); the def's applicability array stays `["0"]` — PART refs
are appearance, not identity, so no new registry rows mint.

## F4 — humans gain `needs = ["thirst"]` {#f4}

_2026-08-06._ Closes the audit's drink drift: the pie menu's gate (`can_drink` ⇔
`metabolism > 0`) passes for humans while the worker refuses the satisfy (no thirst row) —
an offered-but-refused option is the exact drift class the one-filter design exists to
prevent. Humans are biological lifeforms; they thirst. **Rejected**: teaching the menu
filter about need-row presence — a second, client-side availability rule when the real
inconsistency is the corpus authoring a metabolism without the need it feeds.

## F5 — pawn-part-placement CLOSES as superseded; its live intent carries here {#f5}

_2026-08-06._ [2026-07-30-pawn-part-placement](../2026-07-30-pawn-part-placement/README.md)
is open with 9 unchecked items written against `content/visual/pawns.rd` variables
(`head.scale 0.625`, `head.offset.y −1.15`) that were DELETED with the DSL — a resumed
session cannot execute a plan whose knobs don't exist. It closes as SUPERSEDED (not
delivered): placement tuning is now plain TOML edits on the `[[thing.part]]` blocks (this
stream's render drills tune them if the current `0.5`/`z 0.87` look wrong), and the
lighting/caster-capture lanes are recorded successors in this stream's issues, not silently
dropped.

## F6 — repair BEFORE build: the I11 subframe registration lands first {#f6}

_2026-08-06._ [subframe-ingest I11](../2026-08-02-subframe-ingest/issues.md#i11) (mover
subframes never registered — the authored rects are dead data) is the load-bearing rot: no
mint drill can be visually judged while every mover crops by defaults. The fix is the
issue's own prescription — MoverLayer registers per-SLOT subframes from `moverParts()`'
rects against the stems `moverSlotTexture` actually builds — and it benefits the WOLF too,
so the standing drills double as its regression check.
