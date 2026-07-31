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
- **2026-07-30 · P0.4 / P4.1 · [I2](issues.md#i2) REFUTED — the normal cannot scale differently
  from the albedo.** `__bridge.resolver.resolve()` returns one frame per stem, and both pawn parts
  land on identical **128×128** frames of the same 2048 page (body `x256 y0`, head `x256 y256`).
  The resolver CO-PACKS a stem's four maps as quadrants of a single frame, so albedo and normal are
  the same frame by construction — no scale can reach one without the other. The reported symptom
  is fully explained by the 256/128 map-size split found and fixed earlier today. **P4 closes as
  refuted**; its remaining items are struck rather than built.
  Incidental: the head's 128 px frame is drawn at 80 px, confirming per-slot `scale` acts at DRAW
  time, not in the atlas.
- **2026-07-30 · P0.1 · Before-image captured; it found the dominant defect.** Live client at
  `:5174`, human female at 104,51 facing east (mover 813694981). Captured at game zoom 1 and 2.
  The zoom-2 crop shows the head rendering **twice at two scales** — a large pale textured head
  up-left and the correct 80 px one in the selection outline. Mover layer holds exactly 2 prims
  (body 128, head 80) and no duplicate, so the oversized copy comes from the COLD layer
  ([I6](issues.md#i6)). Facings e and n captured; s/w not reached before hand-off.
  Also confirmed [I5](issues.md#i5) first-hand: after a reload the pawn had 1 part with
  `tex: null`; a select + right-click move restored both parts and their textures.
