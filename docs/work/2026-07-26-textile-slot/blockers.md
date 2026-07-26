# TEXTILE_SLOT — blockers

_Things needing human input: what blocks, why it needs you, and a suggested path. Newest first;
resolved rows keep their date. The goal is fewer of these over time — a well-understood blocker becomes a
[fork](forks.md) or [issue](issues.md) I resolve myself._

## B-1 — deep zoom-out streams ~225 full zones; is that the intended cost? (open, 2026-07-26)
**What.** At lod 3 the window is **192×128 tiles ≈ 225 zones**. The subscription now correctly requests all
of them ([I8](issues.md#i8)), so the world does fill to the edges — but it takes **a second or two**, during
which the view is a small square that grows outward. Verified live, not theoretical.

**Why it needs you.** Three defensible answers and they are product calls, not engineering ones:
1. **Accept it.** Deep zoom-out is a map/overview gesture; a brief fill-in is normal for that idiom.
   Costs nothing to implement — this is the current behaviour.
2. **Raise `ZOOM_MIN`** from 0.125 to 0.25, dropping lod 3. Halves the far window to 96×64 tiles (~56
   zones) and the fill becomes near-instant. Costs a zoom level you deliberately gained when `SQUARE`
   went to 128.
3. **Add a coarse zone payload** — fetch distant zones at reduced detail (tile kinds only, no things, or a
   pre-baked per-zone image). Best result, but it is a **wire-protocol + server change**, so it is the one
   answer I should not pick unilaterally.

**Why it is not urgent.** Nothing is broken and nothing regressed relative to before — the old code simply
never subscribed that far, which is precisely the bug [I8](issues.md#i8) fixed. This is a new cost that
arrived with correct behaviour.

**Suggested path:** take (1) for now and revisit if deep zoom-out becomes a common gesture rather than an
occasional one. If it does, (3) is the real answer and deserves its own stream; (2) is a stopgap that
spends a feature to buy latency.

---

## Dependencies — NOT blockers (no human input needed)
- **[P4](todo.md) waits on [primitive-graph P10](../2026-07-25-primitive-graph/todo.md).** `light_data.
  coarsest_lod` is ratified in VARIABLES but has no consumer until the invertible `RGBA32F` accumulator
  exists — there is nothing to subtract from, so building it now would be untestable bookkeeping. Sequencing,
  not a decision.
- **[F5](forks.md#f5) (24 vs 32 slots wide) is an open fork with a default already taken** (keep 24; 32
  would make both wrap axes bitmasks at ~+90 MiB). Recorded so the choice is visible, not because it needs
  answering.

## Resolved
- **F4 — max supported aspect** — RESOLVED 2026-07-26 as an executable default (size for the 5:3 reference,
  let cover trade area away on other aspects) rather than waiting on input. See [F4](forks.md#f4); revisit
  only if the "off-aspect sees less area" property is not the fairness behaviour you want.
