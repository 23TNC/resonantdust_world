# Todo — incremental presence (execution order)

_Gated on the corridor↔brute identity diff (0 mismatches) + the `__cold.debugLastFlush` /
`__gather.debugDirtyTiles` counters (per-frame commands must stay ≈ the mover's boundary; CPU tiles
visited ≈ the reach box, not the window). Deferred — build on the user's go._

## P1 · Gate buildCasters (cheap immediate win)
- [ ] Skip `buildCasters` entirely when the standing set + window are unchanged (a light-only-motion
      frame does no caster work). Track a signature like `buildPresence`'s `presSig`.

## P2 · Bidirectional index
- [ ] Persistent `tile → lights` (CPU presence mirror; stop clearing/rebuilding each frame).
- [ ] `light → tiles` reach-box set per light.

## P3 · Incremental update on move
- [ ] Upsert the mover into its new-box tiles DURING the dirty walk; prune old − new (trailing).
      Changed tiles compare-write to the GPU (unchanged).
- [ ] Verify: per-frame CPU tiles-visited ≈ reach box (instrument), identity 0 mismatches, the
      3-light rig renders identically (green motion, red/blue static).

## P4 · Pan fallback + rank churn
- [ ] Full rebuild ONLY on window change (pan); incremental otherwise.
- [ ] Nearest-N rank churn (>14 overlap): re-scan on remove-from-overfull, or record the limitation.
- [ ] Verify: pan across a region, many-light overlap — presence correct.
