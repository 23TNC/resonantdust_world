# Completed — lighting visual correctness

## 2026-07-31 · P0 — the fixes land; the base offset is pinned

**The two in-tree fixes, verified on a cold load**: the intensity decode now restores the
×4 overbright range (authored 1.0 displays as 1.0 — the pools went from dim smudges to
bright, wide light on screen); torches author **reach 16** and the occupancy probe shows
registration to EXACTLY d = 16 and not d = 17, at intensity lane 16. All three content
torches confirmed (100,51) / (108,53) / (104,59), reach 16 each. The reach-8 lore comment
(whose fps table measured the DELETED gather) replaced with the current measurement basis.

**The base offset pinned** ([I1](issues.md#i1)): one mechanism, counted twice — the
letterboxed master's bottom margin puts the record's plan line `C.y` ~0.5 tiles south of
the drawn feet (conifer/wolf 4.5 units) while the height window starts the same margin
above it. Vertical cancels; plan does not — hence shadows visibly detached from feet.
The P1 model zeroes both columns by construction.
