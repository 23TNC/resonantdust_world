# Blockers — spacetime-again

_Needs your input. Open → resolved; resolved rows keep a date._

---

## B-1 · The word format for `actions : Vec<u32>` — OPEN

**Blocks:** W1 (partly), W2 phases 3–4, W4, W6. Everything that *interprets* a program. W1's uid /
status helpers, W2 phases 1–2 and all of W3 are unaffected — start there.

**Why it blocks.** `event_word` was deleted (it had zero call sites — it belonged to the old
pipeline), and nothing replaced it. A worker cannot extract targets from a program it can't decode,
and the edge can't compose one.

**Why it's yours.** Three other open questions collapse into this one, which is the argument for
deciding it deliberately rather than letting the first implementation settle it:

- **The read set.** Writes fall out of `actions` — every reference is a `u32` carrying its
  `server_id`. Reads don't: a read reference and a write reference are *identical* in the stream.
  Telling them apart is game semantics (`action_reads_actor`), unless the encoding separates them.
- **`promote_state` / `promote_event`.** Named as verbs; the palette that held `ACTION_MOVE` went
  with `event_word`. They need to exist somewhere in the encoding.
- **Target extraction.** W2/W4 both need "which references does this program write" as a structural
  scan, not an interpretation.

**The old shape, for reference** (`git show checkpoint/pre-shard-rebuild:shared/codec/src/event_word.rs`):
`u64 = op_code:4 | reserved:12 | server_reference:16 | payload:32`, with `OP_LITERAL/OBJECT/ACTION/
ALIAS` and an append-only `ACTION_*` palette.

**What's changed since:** a word carrying a reference now needs **32 bits, not 48** — an
`entity_reference` is a u32. `op_code:4 + entity_reference:32 = 36`, so a word is either a u64 with
28 spare, or the fields are narrower than the old ones. `actions : Vec<u32>` in `TABLES.md` says a
word is a **u32**, which forces a split encoding (a reference occupies a whole word; the op-tag lives
in a separate word or in bits stolen from the payload).

**Suggested path:** decide the frame first — u32 words, and how an op-tag coexists with a 32-bit
reference in one. Read/write distinction and the `promote_*` verbs then follow from it rather than
being bolted on.

---

## B-2 · Two events on one `(entity, tic)` — OPEN

**Blocks:** nothing yet. W3's `apply` will need an answer, but the schema doesn't change either way
if the answer is "can't happen" or "last write wins".

**The question.** Ordering *across* tics is settled: a row at T can't be written while an earlier tic
for that entity is dirty, and the worker sees that because it holds a slot on every row for its
targets. *Within* one tic it isn't. Two players hit the same door at tic T: both events carry
`event_tic = T`, both target the door, `dirty(door, T) = 2`, and both unblock the moment T-1 settles.
Whichever `apply` lands first wins, and the outcome depends on network timing.

**Three answers, any of which is fine — they just need choosing:**

| | |
|---|---|
| **can't happen** | the edge or `declare_pending` rejects a second event on an occupied `(entity, tic)`. `dirty` is then only ever 0 or 1 and nothing more is needed. |
| **last write wins** | accept the non-determinism. Cheapest; means the same inputs can produce different worlds. |
| **order by `event_reference`** | the events compose in ascending order — deterministic. Needs *something* per slot that says which is next; `dirty` counts, it doesn't order. |

**Why it's yours:** it's a gameplay-determinism call, not a schema one. The old design took the third
answer and called it "the one property that must not be broken" — but that was a different machine,
and I've been wrong once already assuming its reasoning carried over.
