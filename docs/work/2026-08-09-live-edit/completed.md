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
