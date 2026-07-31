# Blockers — pawn part placement

_Only what genuinely needs the user. A decision I can make is a [fork](forks.md), not a blocker._

None open.

**Judgement calls deliberately routed to the user rather than guessed**, when their items come up:
the final `head.offset.y` ([P1](todo.md)) is an eyes-on tune — the previous stream left `-1.15`
marked "first-guess placement, the P4 drill tunes it with the user's eyes", and that is still the
right way to settle it. Same for [P3](todo.md)'s vertical-gradient check: if flat lighting up the
sprite reads wrong, that is an art call, not a bug to tune around.
