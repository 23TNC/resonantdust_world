# Issues — emotions (anticipated inventory)

## I1 — the mood retirement is a WIDE sweep with a load-bearing tie-break {#i1}

Mood touches needs_eval (the sort — its `|mood|` tie-break keeps the card order a
TOTAL order shared with the npc), the wasm stride (`[condition_id, mood, remaining,
priority]` — consumers index positionally!), pawnMood, the npc's log line, and five
client files. The stride's mood lane becomes the summed emotion magnitude IN PLACE
(same stride, new meaning) or the stride reshapes — either way EVERY consumer moves in
the same commit, and the sort must stay total (priority desc → Σ magnitude desc → id
asc) or cards swap places between evaluations.

## I2 — golden + six-consumer rebuild, again {#i2}

The condition schema loses `mood` and gains `emotions`; traits gain the array form;
the new `[[emotion]]` table lands. Golden re-blesses; worker/master/orch/npc/edge/wasm
all rebuild (the edge's hot-reload refuses unknown fields loudly — logs-drop I2's
family — and the wasm needs the cache-bust ritual).

## I3 — the u8 pack is the CANONICAL compact form; keep one packer {#i3}

`emotion:4 | magnitude:4` (max +15, loader-enforced) — pack/unpack lives ONCE in
shared/codec (or the loader) and the wasm accessor ships modifiers as those u8s.
Two packers drifting on nibble order would color pies wrong silently.

## I4 — conic-gradient slice math must match the eval's sums exactly {#i4}

The pie slices derive from the SAME modifier list the eval sums — one wasm accessor
returns both (per-condition packed modifiers + colors), the card never recomputes.
The +1 happy +2 sad example renders 120°/240° — drill it pixel-visibly with a
hand-authored test condition.

## I5 — existing corpus conditions need emotion authoring {#i5}

thirsty/dehydrated/quenched author `mood` today; the retirement strips it and they
need emotions authored (thirsty → uncomfortable, dehydrated → scared/uncomfortable
high, quenched → happy — my picks, one TOML line each to retune). Any left unauthored
reads `fine +0` gray, which is legal but reads as "no effect" — author all three.

## I6 — the active-emotion example is the acceptance oracle {#i6}

The user's worked example (3 playful + 5 uncomfortable + 2 focused + 6 happy → happy)
drills via a hand-authored test pawn (grant conditions carrying those magnitudes) and
the shared eval's unit test — both must agree with the argmax rule and the tie law.

## I7 — carried successors {#i7}

Emotion-driven BEHAVIOR (the npc reading the active emotion), emotion transitions/
decay, per-emotion panel art beyond the color wash, and mood-history graphs are
recorded, not built.
