# Completed — live-edit

## 2026-08-09 — P0/P1/P2: the panel, and the two cheap tabs

**P0.** `design/live-edit.md` written and linked: the panel's shape, the four tabs' data sources,
the ONE-snapshot rule, the read-only stance, and 5 invariants. It records *why* two tabs were
nearly free and two were not — emotions author `color` and conditions borrow theirs through the
emotion pie, while traits and needs authored no colour at all.

**P1.** `LiveEditPanel` + `/edit`. The panel takes `setBody` with a preview region above and its
**own** tab strip below (styled from `TAB_BTN_CSS` / `TABS_CSS` so it matches the built-in one):
`DomPanel.addTab` is mutually exclusive with `setBody`, and the preview has to persist across tab
switches rather than be duplicated into every tab's content. It is the first panel with a real
reason to raise the 1×1 floor — `minCols: 12, minRows: 10`, below which the preview and the strip
stop being simultaneously usable.

`/edit` is one `registerCommand` call, as the survey promised. It focuses the panel and returns a
feedback line only when the selection isn't a pawn.

**P2.** Emotions and conditions, both as cheap as predicted. Conditions **reuses `ConditionCards`
outright** — which only became possible because `2026-08-09-selection-panels` F3 cut that class's
host contract from eight members to one. A second pie implementation would have drifted from the
conditions bar, and the user's requirement is precisely that the two look the same.

**The one-snapshot rule survived contact.** The first draft of the provider called `pawnEmotion`
again inside the live-edit snapshot — a second evaluation of the identical thing at a possibly
different tic, which is exactly what F5 forbids, and which I wrote anyway because the surrounding
code reads that way. It also added an 8th instance of the pre-existing `Uint32Array`/`Float64Array`
type error. Fixed at the root: the **shared** pawn provider now carries `emotionMagnitudes` (the
full `[sum₀..sum₁₅]` vector the argmax was already taken over) and live-edit *slices* it. No
second eval, no new call site, no growth in the error baseline.

**Verified live** (58 pawns in the world, clean profile):

- `/edit` opens the panel at its authored cells `16,4,24,22`, with a 769×274 preview region above
  a four-tab strip.
- Tabs switch and render: **Emotions** shows `Scared ◂9  Uncomfortable 6` — the argmax winner
  marked, live magnitudes beside each label. **Conditions** shows real 34×34 emotion-pie cards.
  Traits and Needs show `—`, awaiting P3/P4.
- It **re-binds**: sampling five pawns gave three distinct emotion readings (one carried an extra
  `Happy 2`), and selecting a *tile* empties every tab rather than leaving a stale pawn on screen.
- `pawnEmotion` was called 3× in 1200ms with details, conditions and live-edit all polling at
  500ms — i.e. live-edit adds **no** eval of its own.

Typecheck holds at the 7-error baseline; build green.

## 2026-08-09 — P3: trait colour and the traits tab

The first of the two expensive tabs, and it went exactly as the survey predicted: a loader field,
a wasm accessor, a corpus pass.

`TraitParams` gains `color: Option<u32>`, parsed from `#rrggbb` by the **existing** `color()`
helper that emotions already use — so the authoring convention is identical rather than parallel.
Unauthored is not an error: it resolves to `None` and renders neutral, because a half-coloured
corpus must still show you what a pawn carries.

Two accessors, deliberately mirroring `pawn_conditions` so the two grids read alike in the code as
well as on screen: **`pawnTraits(kind, payload)`** returning stride-2 `[reference, color, …]`
(colour `-1` when unauthored, so the caller renders neutral rather than guessing), and
**`traitLabelOf(reference)`** — labels resolve BY REF, never by position.

All **11** traits coloured, grouped so the grid reads at a glance rather than being eleven
arbitrary hues: locomotion blue, biology green, combat red, craft amber, light gold, meta grey.

**One thing the audit caught.** My insertion script skipped `emit_light` — it saw the
`color = [1.0, 0.85, 0.55]` *inside* that trait's light tuples and concluded the trait was already
coloured. A per-trait coverage check found it; without that check it would have shipped as the one
grey square in the grid and looked like an authoring oversight rather than a script bug.

**Verified live** after a wasm rebuild + client reload: the traits tab shows **5 squares** at
35×35 for a bunny, coloured `rgb(74,138,90)` / `rgb(58,110,232)` / `rgb(63,122,82)` /
`rgb(107,122,138)` / `rgb(184,163,74)` — the authored palette plus one neutral fallback — and
hovering them names them: **Biological Lifeform, Walks, Corpus, Herbivore, Forager**. Typecheck at
the 7-error baseline; `rd content-check` clean.

## 2026-08-09 — P4: need colour, the effective clamp, and the live rate

`NeedParams` gains `color`, parsed by the same helper. Four needs coloured so a glance at the bars
reads: water blue, food amber, life red, capacity grey.

**`needs_eval::need_state`** returns `(value, min, max, rate)` from ONE evaluation — the whole
point of F3/I3. The bounds are `need_bounds`' EFFECTIVE clamp (narrowed by traits and conditions,
not the authored domain) and the rate is `segment_rate` sampled at the current elapsed: the same
instantaneous rate the integrator averages over, rather than a second implementation of it. The
sign is flipped at the source so `rate` means *change in satisfaction* — negative is losing, for
every need including inverted domains — which lets the panel colour it blindly (I4).

Exposed as `needState(kind, payload, needs, need, now)` → `[value, min, max, rate, color]`, plus
`kindNeedNames` and `needLabel` so the tab iterates what a kind actually carries rather than
guessing.

**The rate needed a unit.** Per-tic is the honest number and unreadable: thirst drains at
0.0023/tic, which any sane rounding shows as `-0.0`. The panel formats **per hour**, because that
is what the corpus authors in — `deplete = 21600` means "the whole domain in one hour" — so the
displayed figure is directly comparable to the TOML. Thirst reads `-100/h` and hunger `-50.0/h`,
which are exactly their authored `21600` and `43200`.

### The bug this phase uncovered

See [I9](issues.md#i9). The bars rendered perfectly and every value was **0**, on a living pawn.
`pawnNeeds` was flattening 48-bit rows into a `Uint32Array`, cutting off the `data` half — the
value. It fed `pawnConditions` and `pawnEmotion` too, so **band-derived conditions had never fired
on the client**: the same pawn that showed `Scared`/`Uncomfortable` before the fix shows
`Dehydrated`/`Quenched` after it.

It was also the root of all **7 pre-existing type errors**. `Float64Array` took the client
typecheck to **zero errors** — green for the first time in this session, and the standing task
chip for those errors is now redundant.

**Verified live**: the needs tab shows `Thirst [100%] -100/h`, `Hunger [100%] -50.0/h`,
`Corpus [0%] -12.0/h` — bars in the authored blue / amber / red, rates red for losing, tooltips
carrying the raw `value / min..max`. Traits, conditions and emotions all still render.

## 2026-08-09 — P5 spike: the preview's verdict

Measured rather than guessed, per [F1](forks.md#f1) — and the measurement was worth it, because
the answer is a constraint rather than a yes/no.

Requesting extra WebGL2 contexts one at a time and checking the world viewport's context after
each: it survived 15 and was **evicted at the 16th**. Every request was granted; the browser never
refused. That is I1's predicted failure exactly — the world goes blank and nothing throws. A first,
unbounded pass that created 20 at once killed the world's context outright and needed a reload.

**Decision: candidate A (one extra `Viewport`), constrained to exactly ONE for the app's
lifetime** — created lazily on first `/edit`, reused forever, never re-created per open. One extra
context against a cap of 16 is comfortably safe; a per-open leak blanks the game on the sixteenth
`/edit`. A wins on merit under that constraint: zoom and pan come free from `Camera`, the world
renders correctly by construction, and it cannot drift from the real renderer the way a second
partial implementation would.

Texture residency is the cost this spike did **not** quantify — it answered the question that can
kill the app, not the one that merely costs RAM. Recorded as the open risk, with B as the fallback
if the preview proves heavy.

Also this pass: the needs bar and rate gained hover tooltips spelling the unit out in words
("units per wall-clock hour") — `/h` is doing a lot of work for two characters, and "is that per
tic?" was the first question asked of it.
