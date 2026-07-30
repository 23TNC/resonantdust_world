# Completed — comfy-linked-tiling

_Dated entries as items land: what landed + how it was verified. P0's albedo-seam
baseline lives here._

## 2026-07-29 · P0 — the albedo seam metric (2/2)

`atlas_check.py` gains an ALBEDO SEAM section: the same D1 in-world adjacency strip pairs
(64 E|W + 64 S|N), measured on the diffuse RGB (0–255 per-pixel distance over the
cross-section band, strips averaged like the normal seams; `albedo_seam` +
`albedo_seam_worst` in `--json`).

**BASELINE (smooth wall):** S|N mean/max **0.00/0.00** — the vertical-run edges are
PIXEL-IDENTICAL across cells (the source is far more consistent than assumed). E|W mean
2.07, max 7.29. The worst pairs all share ONE band: cell 1's WEST edge (15|1 7.29,
11|1 6.52, 10|1 6.52, 3|1 6.52) — the damage is localized to a couple of outlier edge
bands, exactly the shape the frozen-canonical-edge inpaint is built for. Captures:
`baseline-worst-1-cells15-1.png`, `baseline-worst-2-cells11-1.png` (A|B assembled as
they'd abut, 4× nearest).

## 2026-07-29 · P1 — the layout compositor (3/3)

`bin/lib/retile_linked.py` behind `bin/art retile-linked <kind>`: 12 in-world layouts
(runs, 4 corners, 4 Ts, cross, lone) whose cells derive from neighbour masks through the
D1 formula — every one of the 16 cells appears in real context; assembly is in WINDOW
units (the 8px pad ring stays out of the pipeline; `rebleed()` refreshes it after
write-back by edge replication). **Selftest: assemble→slice round-trips the atlas
BIT-IDENTICALLY, 16/16 cells covered.** Seam masks per layout (~2-unit band each side of
every abutment) + the frozen-class ledger: there are exactly TWO canonical seam classes —
E|W and S|N — so the plan is two inpaints total (`hrun` freezes EW, `vrun` freezes SN),
then the frozen bands STAMP atlas-wide and the remaining ten layouts verify only.
`--dry` emits layouts+masks to a chosen dir and prints exactly that plan (verified via
the bin/art entry).
