# Completed — pawn part placement

_Nothing delivered yet. Items land here with their measured result when ticked in
[`todo.md`](todo.md)._

## P0 — Verify before changing anything

- **2026-07-30 · P0.3 · Live part read-back** (done ahead of P0.1/P0.2, which need the pawn moved
  first — see [I5](issues.md#i5)). `__moverLayer` on mover 813694981 (human female, facing north,
  2 parts) gives body `w 128 / zIndex 53.0009` and head `w 80 / zIndex 53.0109`, textures resolving
  correctly to `female/7/n` and `female/11/n.1` — so the part→texture path works. **Two mismatches
  recorded:** head scale is applied (80 = 128 × 0.625) but `body.size 1.5` is inert because
  `placeThing` sizes off `span`, not `size` ([I4](issues.md#i4)); and the head's top sits 10.1 px
  above the body's while being 80 px tall, i.e. inside the torso, so `head.offset.y` is not
  resolving to the 147 px that -1.15 tiles implies at SQUARE 128.
