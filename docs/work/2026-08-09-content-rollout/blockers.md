# Blockers — content-rollout

_Things that genuinely need the user. A decision I can make is a [fork](forks.md), not a blocker.
Open rows first; resolved rows keep their date._

**Nothing blocking as of 2026-08-09.** The stream is executable end to end. Two calls were made
rather than asked, and both are recorded here as *decided* so that a later disagreement is cheap to
find — they are the two places where a different answer would change what gets built.

## D1 — report vs refuse on an unbumped simulation-visible edit — DECIDED: report {#d1}

_2026-08-09, my call, recorded as [F8](forks.md#f8)._

Registry [F12](../2026-08-04-definition-registry/forks.md#f12) says a change to any field the
simulation reads must bump the def's `version`. The bunny/forager edit did not bump, and nothing
complained — which is half of why it failed silently.

`ensure_definition` could **refuse** such a row, making the law mechanical: master boot fails until
the author writes `version = 1`. I chose to **log it with the affected entity count** instead,
because the user's framing is "we are still early in development where our data is in flux", and a
check that halts the metronome on an unbumped edit trades a silent failure for a blocking one.

**Say so if you want the stricter behaviour** — it is a one-line change to the equality check in
`ensure_definition`, and it is the intended graduation once defs stop changing hourly.

## D2 — what a rollout is allowed to reset — DECIDED: nothing a player could notice {#d2}

_2026-08-09, my call, recorded as [F7](forks.md#f7)._

The re-stamp re-derives *set membership* (which traits, which needs, which part slots) and preserves
every *value* (need levels, conditions, position, facing, inventory, the spawn-chosen variant
nibbles), clamping a value only where the new def's bounds require it.

The alternative — re-mint the sidecars wholesale — is one line of code and it would mean every
rolled-out pawn is silently healed and fed. That is the reset the stream exists to avoid, so I ruled
it out rather than asking. The visible consequence to accept: a rollout **can kill**, because an
honest clamp under a lowered `corpus` cap feeds the death sweep ([I4](issues.md#i4)).

Full per-field migration rules — the apple with 30 minutes left, the inventory now over capacity —
stay deferred, as the user asked.
