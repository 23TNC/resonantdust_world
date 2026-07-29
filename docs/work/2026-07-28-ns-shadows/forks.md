# Forks — ns-shadows

_Decisions resolved during execution, with reasoning. The plan-stage design decisions (D1–D5,
user-ratified geometry) live in the [README](README.md); anything that departs from them or
fills a gap they leave gets a fork here._

## F1 · casterOne gate (1) — skipped for perpendicular casters

Gate (1) (`Cb.y <= Rbase.y + SELF_BAND` → no cast) encodes the E/W card's seen-face
asymmetry: a caster NORTH of a receiver sits behind its seen face and must not darken it. A
perpendicular caster throws E/W — there is no n/s asymmetry to protect, and keeping the band
would kill its climb onto e/w neighbours whose bases share its row (the common case: a wolf
passing a tree). Gates (0, self-id) and (2, direction) still apply. Revisit if an n/s mover
directly north of a tall thing ever reads wrong.
