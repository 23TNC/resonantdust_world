# Forks — lighting rebuild

_Decision points + options + which we chose + why. Chronological._

---

## F1 · Frame-coupled shadow size vs LOD {#f1}

**2026-07-22 — OPEN (flagged by user, consequence acknowledged).**

P3 tightens the shadow quad to the sprite's **opaque-pixel bbox**, read from the surface frame
(`frame x/y/w/h`), and **drops `prim_width`/`prim_height`**. This tightly couples the shadow size to
the sprite frame.

- **Consequence:** the sprite size now *is* the prim size — a prim's shadow dimensions come from its
  rendered frame, not an independent geometric size. This **breaks the existing LOD model**, where
  one prim can resolve at several sprite resolutions (64/128/…). User: *"I've fucked myself as now
  our sprites MUST be the size of our prims. So no… I have no fucking clue how we are going to
  re-implement LOD."*
- **Why accept it anyway:** the tight opaque bbox is what makes shadows *properly sized + placed*
  and removes the transparent-pad anchor shimming. Correct shadows now; LOD reconciliation deferred.
- **Options for later (not decided):**
  - Carry a **frame → world scale** so the frame bbox is expressed in prim units independent of the
    resolved LOD pixel size (decouple size from pixel resolution).
  - Keep a **separate geometric size** for the *quad extent* and use the frame only for the
    *silhouette sample* (re-introduces a prim size, keeps LOD).
  - Bake a **canonical opaque-bbox** into the def at content time (LOD-independent).
- **Status:** proceed with the coupling for P3/P4; **do not delete the LOD intent** — record the
  reconciliation as future work once shadows are correct.

## F2 · Silhouette sample source: surface vs albedo {#f2}

**2026-07-22 — lean SURFACE, pending what the gather is actually handed.**

P4 samples the sprite to apply the shadow's shape. **Surface** carries **presence** (coverage/alpha),
which is exactly the silhouette we want. But the gather is currently passed **albedo**. Options:
sample the surface's presence channel (correct), or derive coverage from albedo (workable, less
clean). Decide when P4 lands; the README marks surface as preferred.
