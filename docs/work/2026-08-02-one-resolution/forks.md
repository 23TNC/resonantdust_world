# Forks — one resolution

## F1 — the survivor's name: PARTITION LEVEL {#f1}

The user: "as we zoom in/out we re-size our tiles so that we do not have to re-allocate
textures. This was also called lod. I am uncertain what the new name should be."

Candidates weighed: **partition level** (chosen — `SquareCache` already speaks
"partition / re-partition" for exactly this operation, so the code names itself);
*zoom band* (accurate but implies the camera owns it, when the caches do); *grain /
density* (evocative, matches nothing existing). `partitionForZoom(zoom)` returns the
level, `slotPx = SQUARE >> level`, the lighting window carries `win.level`. One term,
already native to the file that owns the mechanism.

## F2 — the maximum size is the manifest's, not a constant {#f2}

"Always pass the maximum texture size" = each stem's largest served master (today
`BASE_LOD_PX = SQUARE = 128` for things; grid stems serve their atlas master whole).
The resolver asks for the stem's max from the manifest rather than hard-coding 128, so
a future art-resolution bump is a content change, not a client change — the same
posture as the SEED ppu lane (data-driven atlas scale, no hardcoded ×8).
