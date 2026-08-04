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
