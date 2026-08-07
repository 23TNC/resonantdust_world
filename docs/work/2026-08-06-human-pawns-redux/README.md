# Human pawns, re-implemented — the two-prim pawn lives on the current stack

_Opened 2026-08-06 (user): "We are going to re-implement the human pawns. I am uncertain what
state human pawns are in currently. Essentially human pawns are multiple prims currently one
for the head and one for the body." Decisions in [`forks.md`](forks.md); the current-state
audit below is the ground truth this plan stands on._

## The audit (what exists today)

The original [2026-07-30-human-pawns](../2026-07-30-human-pawns/README.md) stream (done,
10/10) built the whole SKELETON, and most of it SURVIVED every rework since:

- **Content**: `human_female`/`human_male` author two `[[thing.part]]` blocks — body (slot 0,
  scale 0.8, anchored 0.5/1.0) and head (slot 1, `part = 1`, scale 0.5, `offset.z = 0.87`,
  `depth = 1.0` — the facing-flipped draw-order key) with per-facing `s`/`e`/`n` subframes.
  Since stat-model/input-rework they also bind `biological_lifeform` + `walks` level 1
  (24 tics/tile derived).
- **Art**: complete on disk — 9 body variants × 16 head variants per kind, per-facing masters
  (`albedo.e.1.png` shape), the manifest records `part_id [0, 1]`, `part_variants [9, 16]`.
- **Wire**: the payload `PART` opcode (= 1) SURVIVED the stat-model reshape; CREATE's payload
  words flow event → worker (which appends the minted TRAIT entries) → `spawn` → the fanned
  `Payload` frame → `PawnParts` → MoverLayer's per-slot join. The defs are allocated and
  pinned (`female 0x3002_00A0`, `male 0x3002_00B0`).
- **Rendering**: MoverLayer renders one warm prim per slot (`moverParts`), `carrierOf` attaches
  the head to the body's record, per-part facing stems resolve (`…/e.1`), `pawnAt` hit-tests
  every part.

And four things ROTTED or were never finished:

1. **No mint path exists.** `part_entry` has zero production callers; the P4 fixture was a
   one-off console call whose mover (`moveEntity`) died with input-rework. Nothing in the
   repo can spawn a human today.
2. **Mover subframes are dead data** ([subframe-ingest I11](../2026-08-02-subframe-ingest/issues.md#i11),
   explicitly still open): the authored body/head rects are never registered — `moverParts`'
   `subframes` field has NO TypeScript reader, and `registerThingSubframes` only covers cold
   things. Every human (and wolf) mover crops by default rects, not the authored ones.
3. **Drink would be offered and then refused**: humans author no `needs`, so the pie menu's
   gate passes (`metabolism > 0`) while the worker rejects (`target carries no row for the
   need`) — exactly the offered-but-refused drift the menu exists to prevent.
4. **Stale paper**: [2026-07-30-pawn-part-placement](../2026-07-30-pawn-part-placement/README.md)
   is open with 9 items written against `pawns.rd` VARIABLES THAT NO LONGER EXIST (the DSL is
   deleted; the TOML says `scale 0.5`/`offset.z 0.87`, not `0.625`/`−1.15`); the retired prim
   `layer` lane is still written by MoverLayer; a selected human would outline its BODY only.

## The model

- **Repair before build** ([F6](forks.md#f6)): the I11 subframe registration lands FIRST —
  per-slot, from `moverParts`' own rects, against the stems `moverSlotTexture` actually
  builds — because nothing visual can be judged before the crops are real.
- **The mint affordance is a CHAT command** ([F2](forks.md#f2)):
  `spawn <thing> [x y] [body N] [head N]` composes the CREATE + `PART` payload the console
  one-off used to be. Dev-facing, repeatable, through the existing allowlisted door.
- **Per-pawn appearance = PART variant nibbles** ([F3](forks.md#f3)): the def stays ONE def;
  a pawn's body/head choice rides its `PART` entries' def refs (variant `0..8` body,
  `0..15` head) — the u4-variants-in-spacetime intent of the original stream, realized
  through machinery that already decodes it (`variantOf(d) = d & 0xf`).
- **Humans are biological** ([F4](forks.md#f4)): they gain `needs = ["thirst"]`, closing the
  drink drift — a human at the pond drinks like a wolf does, through the same menu.
- **The stale paper is faced** ([F5](forks.md#f5)): pawn-part-placement CLOSES as superseded;
  what still matters from it (placement tuning as TOML edits, the lighting/caster lanes)
  is carried HERE or recorded as successors — never silently dropped.

## Exit

In the browser: `spawn human_female` beside the pond and `spawn human_male 103 64 body 2
head 7` mint two-prim humans whose crops match the authored subframes, whose heads turn and
depth-flip with facing, and whose VARIANTS visibly differ; right-click selects (both parts
outlined), the panel shows the derived 24 tics/tile + condition cards; the pie menu walks a
human to water and Drink actually satisfies; the wolf drills run unchanged; a cold boot
holds. Captures + logs in `completed.md`; **the user's eyes close the stream**.
