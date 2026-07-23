# Todo — lighting rebuild (execution order)

_Planned, not started. Phases run in order; each is verifiable on `/overlayRT shadow-cold`. Items
move to [`completed.md`](completed.md) when done **and** verified. See [`README.md`](README.md) for
the model + constants, [`forks.md`](forks.md) for decisions, [`issues.md`](issues.md) for the
failures we're rebuilding away from._

---

_**P0, P1, P2 done + verified** 2026-07-22 → [`completed.md`](completed.md) (quads render at zoom
0.5/1/2; zoom-dependency fixed). Remaining P0 item: **archive the `2026-07-22-shadow-corridor`
stream** out of the repo — deferred to the end so cross-links don't break mid-rebuild._

## P3 · Tight bbox from the sprite frame (opaque-pixel bbox)

- [ ] Read the **surface** (holds presence/coverage) to compute each prim def's **frame x/y/w/h**
      tight to the **opaque pixels** of the sprite.
- [ ] Use frame **w/h** for the quad size and frame **x/y** to place it; **offset the shadow bbox by
      frame x/y**. **Prim `x/y` DOES NOT CHANGE** — we shift the *shadow quad*, not the prim.
- [ ] Bottom corners of the quad placed from the **frame height** (the real prim dimensions).
- [ ] **Drop `prim_width` / `prim_height`** — the shadow is now coupled to the frame. (Consequence:
      sprite size ≡ prim size → **LOD is now unresolved**, see [`forks.md#f1`](forks.md#f1).)
- [ ] Result: quad shadows at the **minimum bbox**; **no transparent-pad anchor shift** needed.

## P4 · Apply the shape (texture the shadow)

- [ ] Each texel already knows it's inside the quad → sample the sprite to apply the silhouette.
      Prefer **surface** (presence); albedo is what's currently passed — [`forks.md#f2`](forks.md#f2).
- [ ] **No uv-out-of-range** possible: the sample point is inside the quad by construction.
- [ ] Verify: shadows properly **placed + sized + textured**.

## P5 · Moving light

- [ ] Move the light and confirm the shadow map **updates correctly** (dirty routing).

## P6 · Rebuild the corridor (pure optimization)

- [ ] Replace the brute-force reach-walk with a corridor that yields **identical output**.
- [ ] Acceptance: output **matches the brute-force baseline** (diff the shadow map). Must NOT
      reproduce the previous corridor's failures ([`issues.md`](issues.md)): wedges, 3-tile cap,
      out-of-corridor pickups.
- [ ] Commit P2–P5 (functional + brute force) **before** starting P6.
