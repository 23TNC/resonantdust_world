# emotions — 16 emotions replace the mood scalar; conditions wear their colors

**What** (user, 2026-08-07): "add an emotion system. We will have 16 emotions, this
replaces our emotion stat… Fine, Happy, Sad, Angry, Embarrassed, Confident, Tense,
Bored, Confused, Scared, Motivated, Restless, Focused, Affectionate, Uncomfortable,
Playful. We will have one overall active emotion at a time… determined by the cumulative
affects of traits and conditions. So a pawn with 3 playful 5 uncomfortable 2 focused and
6 happy will hold the emotion happy. We will define our emotions in toml… how they are
visually represented… sad will carry the color blue, angry the color red, embarrassed
yellow. Conditions will be colored based on the emotions they alter. A condition will
typically alter one emotion… alters no emotion → Fine +0. Fine will be gray. We will not
display the text for the emotion in the condition — hover… a tool tip with all of the
properties of the condition. A magnitude of +0 will be omitted. We will include the +
symbol. All magnitudes will be positive. Because we hold 16 emotions… a condition can
hold a single u8… u4 emotion u4 magnitude… maximum magnitude +15. When we color our
condition we will divide our condition into a pie chart based on the magnitudes of each
emotion modifier… +1 happy +2 sad → 2/3 blue and 1/3 happy[-color]. Our conditions will
be drawn as squares. For our first pass we will color our details panel background color
the current active emotion on the pawn."

## Design stance

- **Emotion identity is the u4 INDEX, corpus-declaration-ordered, `fine` first**
  ([F1](forks.md#f1)): sixteen `[[emotion]]` blocks (name, label, color); index 0 =
  `fine` = the +0 default = gray. NOT registry-numbered — emotions never ride the wire
  (every consumer holds the corpus); the u4 is the whole identity, and the modifier
  packs to the user's u8 `emotion:4 | magnitude:4` (max +15).
- **Modifiers live on CONDITIONS and TRAITS** ([F2](forks.md#f2)):
  `emotions = [{ emotion = "happy", magnitude = 6 }]` — conditions author scalars,
  traits author per-LEVEL arrays (the stats pattern). Magnitudes are 1..15 (author
  nothing instead of +0); an unauthored condition reads `fine +0`.
- **The active emotion = argmax of summed magnitudes** over the pawn's traits + ACTIVE
  conditions, through ONE shared eval ([F3](forks.md#f3)) — the user's example verbatim
  (3 playful, 5 uncomfortable, 2 focused, 6 happy → happy). No contributions → `fine`;
  a tie → the LOWEST index wins (deterministic, and the fine-first ordering resolves
  ties toward calm).
- **MOOD RETIRES** ([F4](forks.md#f4) — the "replaces our emotion stat" reading): the
  conditions' `mood` offset field, `pawnMood`, the panel's `mood N%` row and the card
  sort's `|mood|` tie-break all go; the sort tie-break becomes SUMMED EMOTION MAGNITUDE
  desc. Delete-don't-deprecate; the sweep is its own phase.
- **Cards become pie-colored SQUARES with a tooltip** ([F5](forks.md#f5)/[F6](forks.md#f6)):
  no text on the card; a CSS conic-gradient slices the square by modifier magnitudes
  (slices in emotion-index order); solid color for one modifier; solid gray for none.
  Hover = the IntentStrip tooltip pattern showing ALL properties: label, `+N <Emotion>`
  per modifier (+0 omitted, `+` always), priority, remaining for a timed grant, and the
  need modifiers.
- **First pass panel wash** ([F7](forks.md#f7)): the details panel BODY background takes
  the active emotion's color (alpha-dimmed so the monospace rows stay readable — the
  literal ask, kept legible).

## What exists (audited 2026-08-07)

- Conditions: `mood` offset + `stats`/`needs` modifier lists; the sort
  `priority desc → |mood| desc → id asc` computed ONCE in
  `needs_eval::active_conditions` (F3 of conditions) — every consumer ranks identically
  (panel cards + npc decision order). `pawnConditions` (wasm) returns stride 4
  `[condition_id, mood, remaining, priority]`; `pawnMood` sums offsets.
- Mood consumers to sweep: needs_eval (sort + sum), wasm (`pawnMood`, the stride),
  npc wolves (logs mood; decisions use the sorted list), client (DetailsPanel mood row,
  ConditionCards' mood text, MoverLayer.pawnInfo, WasmClient types, WorldScene
  provider).
- ConditionCards renders text cards today; the strip machinery (tooltip pattern,
  panel-sibling positioning) is proven by intent-queue-ui.
- Six content consumers rebuild on schema change (worker/master/orch/npc/edge/wasm);
  golden guards condition params; the corpus is hot-served.
