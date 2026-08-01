# Issues — speculative direction

## I1 — the pops, named from the live log {#i1}

P0's instrumentation (`[facing] old->new src d=(rendered delta) r=(render pos)`), one
wolf soak + one diagonal human trip:

- `1->0 d=(0.036,0.036)` — a PERFECTLY DIAGONAL rendered step (the chase closing both
  axes at once) while the greedy walk crossed a stair-step: the eye sees diagonal
  motion, the sprite snaps east→south. The stair-step pop.
- `2->3 d=(-0.010,0.007)` — the render gliding SOUTH-west while the sprite had been
  facing NORTH (2) and flips WEST: spec-space (post-reseed walk) fully disagreeing with
  the rendered direction.
- `3->1 d=(0.002,0.010)` — the render stepping dominantly SOUTH while the sprite flips
  EAST: the walk's e/w-first leg leads the render's actual path.

All three are `src=spec-walk` — `walkGreedy`'s spec-space facing driving the sprite
while the CHASE moves it somewhere else. Exactly F2's prediction; the fix derives from
the rendered delta.
