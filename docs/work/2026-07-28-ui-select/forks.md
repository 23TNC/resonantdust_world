# Forks — ui-select

_Decisions resolved during execution, with reasoning. Plan-stage decisions D1–D6 live in the
[README](README.md); departures or gap-fills land here (expected: the D5 outline
silhouette-vs-box call)._

## F1 · D5 outline — silhouette shipped, no fallback demotion

The silhouette read is 5 texel fetches per fragment over a handful of sprite-sized quads —
disproportionate never materialized. BOX mode remains for tiles and unresolved frames (and
would catch a future perf surprise by demoting per-item).
