# Primitive graph — deviations

_Where execution departed from the written plan, logged AT THE MOMENT of deviating. Chronological append._

### D-1 — P8's DDA landed at 52.3%, not the planned ~59%, and KEPT a dilation (2026-07-26)
**Planned:** "Replace the 5-tile cross pad with a supercover DDA visiting each crossed tile once. Acceptance:
corridor↔brute identity still 0 mismatches; bucket fetches down ~59%."

**Shipped:** a supercover DDA that still dilates by ±1 **perpendicular to the ray** — 3 fetches per tile, not
the planned 1 — for **52.3%** fewer fetches.

**Why the plan could not be met as written.** The ~59% figure assumed the cross pad was pure redundancy,
covering only sampling misses that an exact DDA does not have. It is not: removing the pad entirely lost
**1792 texels** of shadow that brute finds, with **0** in the opposite direction. Casters are bucketed by
their tight-bbox ground cover while `casterCover` tests a wider projected extent, so a caster registered in
tile T really does occlude rays through T±1. Full detail in [I30](issues.md#i30).

The recoverable part was dilating ALONG the ray, which consecutive walk tiles already cover. Perpendicular-only
keeps the cover conservative and holds identity at 0 mismatches over 102,442 non-zero texels.

**Consequence for the plan:** P8's remaining items stand unchanged, but the "~59%" basis is retired — 52.3% is
the ceiling for this approach. Anything beyond it needs the union walk (P8 items 3–4), whose caveat is already
recorded in [I29](issues.md#i29).

### D-2 — measurement discipline: the first identity check was VACUOUS (2026-07-26)
Not a plan deviation but a near-miss worth the same visibility. P8's acceptance is "0 mismatches", and the
first run reported exactly that — while **both buffers were entirely zero**. No kind emits light since the
flora revert, so `carriedLights` was 0 and there was nothing to cast; the check compared empty against empty.

Re-ran through `__torch()` with 12 lights spread across the standing set for the real comparison.

**Rule this implies:** an identity/diff acceptance criterion must also assert a **non-zero population on both
sides**. "0 mismatches" alone is satisfiable by having no data. Applies to every remaining identity item in
P6 and P8.
