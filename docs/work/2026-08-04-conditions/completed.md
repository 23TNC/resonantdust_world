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
