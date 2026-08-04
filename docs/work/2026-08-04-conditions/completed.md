# Completed — conditions

_Dated entries: what landed and how it was checked. Append-only; authoritative for what's done.
The items themselves live in [`todo.md`](todo.md) with their boxes ticked._

## 2026-08-04 — P0: the corpus and `shared/content` speak "condition"

**What landed.** `content/needs.toml` renamed `[[moodlet]]` → `[[condition]]` and each band's
`moodlet =` key → `condition =`, ids 1/2/3 untouched. `shared/content` renamed throughout:
`MoodletParams` → `ConditionParams`, `NeedBand.moodlet` → `.condition`, the six `Bundle`
accessors → `condition_names` / `condition_id` / `condition_name` / `condition_params` /
`condition_params_all`, `MoodletToml`/`moodlet_slots` in the TOML loader, and
`active_moodlets`/`ActiveMoodlet`/`moodlet_id` → `active_conditions`/`ActiveCondition`/
`condition_id` in the eval. The "conditional moodlet" sub-vocabulary retired to **DERIVED** vs
**TIMED** per [F4](forks.md#f4), and the `m`-prefixed locals in `needs_eval` (`mid`, `mp`) went
with it. `ConditionParams` gained the [F6](forks.md#f6) note: `mood` is one effect, not the
definition.

**How it was verified.** `grep -in moodlet content/` empty; `grep -rin moodlet shared/content/src`
returns only historical `needs-moodlets` STREAM references (deliberately preserved — the rename
script sentinels that token, since `docs/work/2026-08-03-needs-moodlets/` is a real folder).
`cargo check -p resonantdust-content --all-targets` green. `cargo test -p resonantdust-content`:
**11 unit + 2 golden tests pass**, including all five `needs_eval` band/timed/wrap tests at
unchanged values.

**The golden re-bless.** `BLESS_GOLDEN=1 … --test golden`, then diffed against the pre-bless copy:
**8 changed lines, every one a rename** (`== moodlets ==` → `== conditions ==`, two `moodlet:` band
keys, `== moodlet_params ==`, three `MoodletParams {` headers). Zero numbers, zero ids, zero table
rows moved — the acceptance criterion exactly. The un-blessed test then passes.

**Fixed en route:** [I2](issues.md#i2) — the `BLESS_GOLDEN` branch had become unreachable when the
DSL died, so the fixture could not be regenerated at all. The dead `.rd` oracle half is deleted and
the bless path now lives in the surviving TOML test.

## 2026-08-04 — P1: the wire names follow, and the renamed chain runs live

**What landed.** `PAYLOAD_OP_MOODLET` → `PAYLOAD_OP_CONDITION` (**still 3**) with
`condition_word` / `payload_conditions` / `upsert_condition`; `GRANT_MOODLET` → `GRANT_CONDITION`
(**still opcode 11, arity 2**). The pawn module's reducer `grant_moodlet` → `grant_condition`,
republished. Both binding trees regenerated. The edge verb allowlist, the worker relay arm, the npc
brain and `client/core`'s `PawnParts` doc all follow. `shared/wasm` came with them (a P2 item pulled
forward — `rd redeploy` builds `shared`, so it could not wait): `pawnConditions` / `conditionLabels`.

**How it was verified.**
- `cargo test -p resonantdust-codec` — **63 tests green**; `PAYLOAD_OP_CONDITION == 3`,
  `GRANT_CONDITION == 11`, the arity table still yields `&[Write, Imm]` for 11.
- `bin/rd deploy module pawn` — published with no schema error.
- `bin/rd build spacetime pawn` — `grant_condition_reducer.rs` written, `grant_moodlet_reducer.rs`
  deleted; `git diff --stat` on the binding tree shows **only** those two files plus `mod.rs`, i.e.
  no formatting churn. `st-bindings` mirrored and byte-identical to the edge's copy.
- `bin/rd build edge` / `bin/sim build worker|master|orchestrator|npc` / `bin/rd build core --check`
  — all green. `bin/rd redeploy --run` clean, then `bin/rd redeploy` reports "nothing changed".
- Generated `shared/pkg/resonantdust_shared.d.ts` exports `pawnConditions` and `conditionLabels`;
  no `moodlet` symbol survives in it.

**The live drill** (the real proof — this is the whole renamed chain, npc → edge allowlist → worker
relay → republished reducer → payload op → shard fan → npc decode → `active_conditions`), run with
the new [F8](forks.md#f8) drills, `NPC_THIRST=20 NPC_GRANT=3`:

```
thirst initialised … satisfaction=20 drill=true
GRANT_CONDITION queued (drill) … condition=3
condition band change tic=1704 conditions=["dehydrated"]              mood=0.0999…  next=Some(5305)
condition band change tic=1710 conditions=["dehydrated", "quenched"]  mood=0.3      next=Some(5305)
```

Both arithmetic checks hold: `0.5 − 0.40 = 0.10` for the derived band alone, `0.5 − 0.40 + 0.20 =
0.30` once the timed grant stacks. The wolf CREATE-minted, adopted and ran authoritative trips
throughout (the republish wipes the module's data, so it re-minted).

**Fixed en route:** [I3](issues.md#i3) — `rd build core --check` had been red since 2026-08-03
(`headless.rs` never learned the `payload` field that needs-moodlets P4 added to `Event::PawnParts`).
[I4](issues.md#i4) — the two-week-old edge container predated the `content` bind mount in
`compose.yml`, so worldgen and content serving were both dead; `rd down edge && rd up edge` fixed
it. Neither was caused by this stream.

**Not verified:** no log line from the edge or worker names the accepted verb — they don't log verbs
at INFO. The npc evidence above is stronger (the grant could not have reached the payload otherwise),
but the literal wording of the item's criterion ("edge log shows the verb accepted") was not met.

## 2026-08-04 — P2: the client and the authoritative docs

**What landed.** `moodlets` → `conditions` through `WasmClient.ts`, `MoverLayer.ts`,
`WorldScene.ts` and `DetailsPanel.ts`'s `DetailsProviders`. `docs/VARIABLES.md` §"Needs &
conditions" rewritten (schema block, accessors, the DERIVED/TIMED pair, the TOML example),
`docs/TABLES.md`'s payload row `MOODLET` → `CONDITION` and the payload-entry verb prose,
`docs/ACTIONS.md` row 11 → `GRANT_CONDITION`. Both table/action rows carry an explicit "renamed;
the VALUE is unchanged" note so a reader hitting an old dump is not left guessing.
`VARIABLES.md` also now states the [F6](forks.md#f6) rule outright — a condition's effects are an
OPEN set, `mood` is the first, do not write code that assumes a condition *is* a mood offset — and
documents `priority` ahead of P3 building it.

**How it was verified.** `npm run typecheck` (tsc --noEmit) clean; `bin/rd build webgl` clean;
`grep -rin moodlet client/webgl/src` returns only historical `needs-moodlets` stream references.
`bin/rd docs-check` green. A repo-wide `grep -rin moodlet` over code + docs, excluding
`docs/work/` history and the `needs-moodlets` stream token, now returns **only** the two deliberate
"renamed from" notes.

**Live in the browser** (the whole point — the renamed client reading the renamed wire): selected
the drilled wolf `0x30800001` in the running client and read the details panel back out of the DOM —

```
pawn      0x30800001
kind      pawn/animal/wolf (#7)
mood      30%
  Dehydrated  −0.40
  Quenched  +0.20  2284t
```

That is the npc's evaluation, independently recomputed by the wasm eval from the same payload:
same two conditions, same offsets, same mood. The panel still renders them as text rows — cards are
P4.

## 2026-08-04 — P3: priority is authored corpus data, sorted in the ONE eval

**What landed.** `ConditionParams.priority: i32` (+ `ConditionToml`, `#[serde(default)]` so an
omitted key is `0` and never a derived guess), authored in the corpus as dehydrated 30 / thirsty 20
/ quenched 10 — **in tens**, so a future condition slots between two without renumbering.
`active_conditions` now returns its result sorted `priority` desc → `|mood|` desc → `condition_id`
asc, and `ActiveCondition` carries `priority` out. The wasm `pawnConditions` stride widened 3 → 4
(`[id, mood, remaining, priority]`); `WorldScene`'s decode reads fours and preserves the order.
`DetailsPanel`'s provider type carries `priority` and documents that the order is authoritative.
`VARIABLES.md` states the rule, the tens convention, and why derive-from-mood is wrong.

**How it was verified.**
- New unit test `the_order_is_priority_then_magnitude_then_id`, on a fixture built so **each** key
  decides something and the insertion order is deliberately wrong on all three: the highest-priority
  condition is authored LAST in the band list, two conditions tie on priority so `|mood|` splits
  them, and two tie on priority AND `|mood|` (0.20 both, opposite signs) so `condition_id` splits
  them. Asserts the exact sequence `[1, 3, 2, 4, 5]`. Also asserts `priority` rides out on both a
  derived row and a timed grant.
- Loader test extended: an authored `priority = 20` reads back verbatim; a condition that omits the
  key yields `0`.
- `cargo test -p resonantdust-content` — **12 unit + 2 golden green**.
- Golden re-blessed: the diff is **three added lines**, the three authored priorities. Nothing else
  moved — notably the `needs probes` section is byte-identical, which is the evidence that adding
  the sort did not reorder any existing result.
- `docker compose -f shared/compose.yml run --rm check` — the **2-pass** gate (native all-targets +
  wasm32 `--features js`) green, so the browser surface really compiles, not just the native lib.
- `npm run typecheck` clean; `bin/rd build shared` regenerated the bundle.

**Not yet observed live:** the panel is still text rows, so the *visible* effect of ordering lands
with the cards in P4. The order itself is proven by the unit test and the unchanged golden probes.

## 2026-08-04 — P4/P5: the cards, and the click that maximizes them all

**What landed.** `game/panels/details/ConditionCards.ts` — the strip, as a `position: fixed`
element appended to the panel's HOST (`#app`), i.e. a **sibling** of the panel root, not a child
([F7](forks.md#f7)). It re-anchors to the panel's bottom-left off `onRectChange` + `onFocus` +
`onMinimizeChange` + `onOpenChange` + `window.resize`, takes `panelZ + 1` so it draws over the
panel's own bottom edge without leapfrogging the next thing focused, and is removed in `destroy`.
Conditions left `DetailsPanel`'s text rows entirely; the body now reserves the strip's band as
bottom padding so its last text row cannot hide under the cards.

A card is label + signed mood + remaining-tics timer, hover-lit, `cursor: pointer`, with
`stopPropagation` so a card click never falls through to a world select. The top `MAXIMIZED = 4`
render at `CARD_W = 80`, the rest at `CARD_W_MIN = 28` (`MINIMIZED_FRACTION = 0.35`). Clicking a
minimized card expands them all; clicking any card while expanded collapses back
([F5](forks.md#f5) — a state with no exit is a trap). The flag is PANEL state, persisted as
`details.conditionsExpanded`, so selecting a different pawn does not silently re-collapse it.

**How it was verified** (measured in the running client, not eyeballed):

| check | result |
|---|---|
| padding from the panel's left / bottom edge | `8` / `8` — exactly `PAD_LEFT` / `PAD_BOTTOM` |
| gaps between cards | `6` px throughout (card origins 8, 94, 180, 266, 352, 386) |
| 4 maximized + rest minimized | widths `80,80,80,80,28,28`; flags `000011` |
| 4 maximized fit the default panel width | `8 + 4×80 + 3×6 = 346` ≤ the panel's `355` |
| **exceeds the panel** | strip `406` px against a `355` px panel — **51 px past the right edge, unclipped** |
| drag / resize / window-resize | left `8`, bottom `8` held across all three |
| minimize → restore | strip `none` → `flex`; close → reopen likewise, one node throughout |
| click a minimized card | `406 → 510` px, flags `000011 → 000000`, `localStorage '1'` |
| click any card while expanded | back to `406` / `000011`, `localStorage '0'` |
| reload while expanded | reopens expanded at `510` px |

**Deviation from the item's letter:** P4's first item said to give the remaining text rows "their
own `<pre>` child". No child was needed — the body element is already `white-space: pre`, and once
the conditions moved out to the strip there was nothing left to separate them from. The item's
intent (no condition rows in the body) is met and verified by grep.

**Added:** `window.__cards(n | rows | null)` — a debug pin on the strip. The corpus authors three
conditions and two of them are DERIVED bands on the same need, so no real pawn can carry more than
three at once; the 4-maximized rule and the viewport clamp are untestable without it.

## 2026-08-04 — P6: the screen edge, and the drill end to end

**What landed.** The clamp ([F9](forks.md#f9)) — two distinct cases, because conflating them is
what makes a right-docked panel feel broken. If the strip FITS the viewport, its origin slides left
until it does; only a strip wider than the SCREEN becomes a scroller. Plus the pointer model: the
container is `pointer-events: none` and only the cards are `auto`, so gaps and the strip's tail
pass clicks through to whatever is beneath.

**How it was verified.**

| check | result |
|---|---|
| panel docked bottom-right (`left 1504`) | natural origin would be `1512`; strip **slid 168 px** to `1344`, right edge `1854` inside a `1862` viewport — all 6 cards visible, no scroll |
| 30 expanded cards | natural `2574` > room `1846` → clamps to `1846`, `overflow-x: auto`, `pointer-events: auto` |
| …scrolled to the end | card 30 fully on screen at `x 1774–1854` |
| `document.body` horizontal scroll | never — `scrollWidth === clientWidth` throughout |
| `elementFromPoint` in a card gap | the panel beneath, **not** the strip |
| `elementFromPoint` past the strip's tail | the world `CANVAS` |
| `elementFromPoint` on a card | the card (its label child, parent `data-rd-condition`) |

**The end-to-end drill** — a wolf on the TOML corpus, `NPC_THIRST=44 NPC_GRANT=3`, with the cards
live in the client:

```
tic= 395  conditions=["thirsty",    "quenched"]  mood=0.55  next_crossing=Some(1958)
tic=1962  conditions=["dehydrated", "quenched"]  mood=0.30  next_crossing=Some(3990)
```

The crossing was **predicted at 1958 and observed at 1962** — the brain evaluates once per second,
i.e. every 6 tics, so that is exact to the sampling grain. Arithmetic: `0.5 − 0.15 + 0.20 = 0.55`,
then `0.5 − 0.40 + 0.20 = 0.30`. The panel followed independently, recomputing through the wasm
eval: `mood 55%` / `Thirsty −0.15` + `Quenched +0.20 3263t` → `mood 30%` / `Dehydrated −0.40` +
`Quenched +0.20 1828t`, the Quenched timer counting down on screen throughout. Card order tracked
priority (thirsty 20 before quenched 10; dehydrated 30 first once it landed) with no TS sort
anywhere.

**Two environment fixes the drill forced.** Adding `priority` to the corpus broke every binary that
links `shared/content` and was built before it — the npc failed with `unknown field 'priority'` and
the edge would have too. Rebuilt and redeployed both (`bin/rd build edge`, `bin/sim build npc`,
`redeploy --run --force`), and hit the `docker-cargo-mtime-miss` gotcha again on the way (touch the
sources first). Worth remembering: **a corpus schema change is a redeploy of every content
consumer**, not just a data edit.

**Outstanding:** the item's criterion ends "the user's eyes close the stream". Everything mechanical
is verified and screenshotted; the user's own look at it is the one thing this session cannot do
for itself.
