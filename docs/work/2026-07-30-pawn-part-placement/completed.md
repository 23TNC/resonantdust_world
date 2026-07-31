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

## P1 — The DSL tune (body 0.8, head 0.5)

- **2026-07-30 · [I6](issues.md#i6) ROOT-CAUSED, and the "cold layer" guess was wrong.** There is
  no cold `pawn/human` prim (`__viewport.map.prims` holds none near the pawn) and the warm cache
  holds only the wolf, the cursor light and the pawn's two parts. Removing the head prim removed
  BOTH heads; the `albedo-warm` overlay shows ONE head at the correct 80 px. So the ghost is the
  **lighting silhouette**: `coldShadowData.definitionFor` sizes the shadow/light card from
  `round(prim.width / SQUARE)` rounded up to pow2 TILES, so an 80 px head gets a 128 px card —
  `1.6×`, concentric, which is exactly what the crop shows. **Proved by experiment:** growing the
  head prim to 128 px collapsed the two heads into one correctly-lit head, nothing else changed.
- **2026-07-30 · P1.1 · Per-slot `scale` moved pre-atlas ([F4](forks.md#f4), superseding
  [F3](forks.md#f3); plan change logged in [D1](deviations.md)).** The def model cannot express a
  free draw scale — `ppu = 2^lod / spanU` must be a whole pow2, and 80 px = 10 units gives 12.8 —
  so the fix is to stop producing one. `MoverLayer` now draws every slot at its own
  `span × SQUARE` and registers the slot's `scale` as the resolved stem's `sprite_scale`; the
  resolver scales all four co-packed maps AND the opaque bbox together, so albedo, normal, surface
  and the shadow card move as one. Read back live: body and head both `w 128`, `spriteScale`
  carries `female/7/e → [0.8, 0.8, 0.5, 1]` and `female/11/e.1 → [0.5, 0.5, 0.5, 0.5]`.
- **2026-07-30 · P1.1b · Scaling now pivots on `sprite_anchor`, not the bbox centre.** The resolver
  re-centred scaled art on the frame centre, which lifts a bottom-anchored body ~11 px off its feet
  at 0.8. `setSpriteScale` takes the pivot and `packCoPack`/`transformedBBox` scale about the
  pivot's point on the opaque bbox. At the default (0.5, 0.5) this is algebraically the old
  behaviour, and both pawn stems' bboxes are frame-centred, so no existing result moves.
- **2026-07-30 · P1.2/P1.3 · `head.offset.y` re-seated to −0.87 tiles** against the 0.8-scale body
  (its art runs 0.238–0.938 tiles above the anchor), and the "first-guess placement" comment in
  `pawns.rd` replaced with what it was tuned against. `body.size 1.5` removed — inert and
  misleading (see [D1](deviations.md)).
- **2026-07-30 · P1.4 · All four facings verified live** at game zoom 2 (e/s/n/w, by driving
  `applyVisual`): one head each, seated on the shoulders, single silhouette, no ghost, no console
  errors. No facing needed its own offset. The head still draws OVER the body in north — that is
  [P2](todo.md)'s job, not a seating fault.
