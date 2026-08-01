# Speculative direction — facing follows the motion, not the server — 2026-08-01

_Component: [`client/webgl`](../../components/client/webgl/). The user's directive,
verbatim: **"When pawns are moving they will ignore the direction provided by the
server, instead they will use the direction they are actually moving. This should avoid
visual bugs where the server takes a bit to update the direction while the pawn is in
motion locally."**_

## The model

A pawn's facing has exactly TWO sources, and motion always outranks the wire:

- **Moving** (a spec is live, or the chase is still closing a gap): facing derives from
  the pawn's ACTUAL rendered displacement — the per-tick delta of the chase position
  `(rx, ry)` — never from `obj.facing`. What the eye sees moving is what the sprite
  faces; a server row that lands mid-motion steers position but cannot turn the sprite.
- **At rest** (no spec, gap closed): the server's facing is authoritative and ADOPTED —
  including on later rows that turn a standing pawn (today an existing mover's row never
  reassigns facing at all, so a resting pawn turned server-side stays stale; that gap
  closes as part of this stream).

## Why the current seam still pops

`walkGreedy` already yields a motion facing while a spec lives — but it is SPEC-space,
not render-space. Three leaks remain: (1) a spec RESEED (interim authoritative row)
snaps `from` under the walk, so the greedy step direction can disagree with the way the
render is actually gliding; (2) the CHASE closes gaps along its own per-axis path — post
landing or after a correction the sprite moves in a direction the walk never reported;
(3) e/w-first greedy diagonals stair-step, alternating facings every tile edge.

Deriving from the RENDERED delta fixes all three at the source, with hysteresis so the
stair-step cannot flicker: a dead-zone on tiny deltas, the dominant axis wins, ties and
sub-eps motion KEEP the current facing, and a facing must persist a minimum hold before
it may flip again.

## Out of scope

Server-side rotation semantics (the wire format stays); the n/s cast-card selection
(`cast_type 2` follows whatever facing the mover reports, unchanged); pixijs (legacy).
