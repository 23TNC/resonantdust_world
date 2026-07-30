# shadow-polish — two shadow bugs: the flipped n/s cast, the wolf wearing its own shadow

## What

Two user-reported graphical bugs (2026-07-30):

1. **N/S billboard shadows are flipped north/south.** The perpendicular-card cast
   (ns-shadows: a vertical card on the center line, sweeping e/w) lands with its n/s
   axis inverted — the head end of the shadow sits at the tail end of the wolf.
2. **The wolf's ground shadow draws ON TOP of the wolf.** The wolf's own cast shadow
   (correctly darkening the ground around it) also darkens the texels where the wolf
   itself is drawn.

## The user's design steer (bug 2)

"We draw the ground light then the on-billboard light. As we are now drawing the ground
light as on-prim light for the tiles… I'd expect we would need to draw the tile's light
first and then overwrite with the wolf's lighting." **Answer: yes, we can overwrite —
that IS the architecture already.** The hot class is a pure CORRECTION over cold
(hot-sync P4: the blit sums cold+hot unconditionally; the hot deposit at a mover's texel
= mover-lighting − ground-lighting, so the sum REPLACES). The bug is therefore not a
missing capability but a leak in what the correction covers: at wolf-drawn texels some
path still evaluates the GROUND result (which includes the wolf's own cast shadow)
without the wolf's on-billboard replacement — the receiver priority between the D8
tile mode-2 path (texture-generalization) and the hot billboard receiver is the suspect
seam. No new strategy needed unless P0's pin disproves this.

## Suspects (located, not yet confirmed — P0 pins before P1/P2 fix)

- **Bug 1:** `casterCoverNS` (`shadowGather.ts` ~l.360): the card's u axis maps world-y
  through `u0 = (L.y + tx·(Q.y−L.y) − (A.y − W/2)) / W` onto the side frame, oriented by
  `mrot` (caster_flip, D3 head-follows-facing). A sign/orientation error in u0-vs-frame
  or in the flip selection would mirror the cast n/s while leaving the e/w sweep (which
  the ns-shadows drill verified) correct.
- **Bug 2:** the class split at `shadowGather.ts` ~l.1041: cold sees a hot receiver as
  GROUND (bakes terrain beneath — intended), hot owns the mover's lighting via
  `accumulateLights(..., rbillboardN, recvHotN)` + the mode-2 walk (hot casters only).
  Since texture-generalization, tiles ALSO claim texels through slot-0 receives mode 2 +
  `tileNormal`; at wolf-drawn texels the wolf must win (frontmost), the tile only where
  no billboard covers. The fine receiver map's hot classification (pawn-render's
  conservative classing) is the other half of the seam.

## Constraints

- The corridor↔brute identity (P6) must survive any caster change — `debugReadShadow`
  gates bug 1's fix.
- Cold bakes must not increase for a walking wolf (movement-hardening: cold 0 during
  walks) — bug 2's fix must stay inside the hot correction, not add cold work.
- The in-page `__fillmode`-era discipline: fixes verified against the LIVE page with the
  npc wolf walking, both zooms.

## Added scope (user, 2026-07-30): bilinear shadow sampling

The lightmap/blit currently samples NEAREST everywhere (the TEXTILE_SLOT universal rule) —
shadow edges render as hard fine-texel blocks (the same granularity as bug 2's squares).
The user wants BILINEAR on the shadows. Constraint: the accumulators are INTEGER textures
(rgba32uint, quantised deposits) — hardware filtering is unavailable, so the blit gains a
manual 4-tap decode-then-weight (each tap /QUANT first; the hot class's deposits can be
NEGATIVE). Hazard to guard: filtering across the receiver-class seam would smear ground
shadow back onto billboard texels — the very artifact bug 2 removes — so the taps get
clamped/class-gated if the drill shows halos. NEAREST stays the rule for every other map
(albedo/normal/surface); only the light accumulators soften.

## Out of scope

Wall cast shadows (all tiles author cast=false — future stream); soul/multi-pawn
layering; the I1 oracle residual (texture-generalization, separate).
