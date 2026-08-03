# Issues — LOD aftermath

## I1 — render-performance coordination: idle, custody taken for ONE bug {#i1}

State read 2026-08-02 ~22:40: `render-performance` is ARMED with 5/22 done. Its I11
(targeted `invalidateStem`, commit `2c908012`) LANDED and measured well (one stem
dirties 916 squares vs 2310; queue drains in 3 ticks). Its P1 complete-on-arrival items
are OPEN and its last resolver-touching commit is ~5 h old — in-flight but idle.

Custody: lod-aftermath lands ONLY the complete-on-arrival re-pack (the black-flora
root) with the smallest possible diff in `ensureCoPack`, and leaves the rest of their
charter (await removal, decode caps, first-paint probes) untouched. A note is written
into their `issues.md` so their session verifies rather than re-implements.

## I2 — the black-flora mechanism, precisely {#i2}

`loadMapBytes` returns null on a 404, and `ensureCoPack` packs the co-pack WITH the
missing quadrant transparent, then sets `packedHash` — so ONE transient 404 (the edge
was down/half-deployed during several of today's boots) permanently bakes an
incomplete co-pack for the session. Nothing retries: the hash matches the manifest, so
even `onManifestChange` keeps it. Flora's layers quadrant read zero on BOTH twins with
healthy disk bytes — the pack simply ran during an outage window. The fix is
completeness bookkeeping + a bounded retry, not a blit bug.

## I3 — tree clipping: the variant rode the CELL, but variants are STEMS {#i3}

Reopened by the user's eyes 2026-08-03 ("something off with how we derive the bbox and
load to the atlas… scaling or not handling span"). Three stages spoke two dialects:

1. `thingTexture` still returned the canonical stem (`conifer/e`) + `cell = variant` —
   the GRID dialect.
2. `resolve()` honours `cell` only on a `grid` entry. Conifer variants are FOLDERS
   (human-pawns P1: stem-addressable, folder 1 = the bare stem) — no grid, cell
   dropped, every variant served the one canonical frame.
3. `packCoPack` crops a non-grid stem with `subframeFor(stem, 0)` — cell 0, hardcoded.
   The subframe-ingest I10 registration put nine per-variant rects at
   `conifer/e#0..8`, but ingest only ever read `#0` = **variant 0, the sapling** — the
   tightest rect in the set, applied to folder 1's master, then scale-to-FIT blown up
   into the quadrant. Cropped by a too-small rect and enlarged: clipped edges,
   oversized art, and all nine variants identical.

The resolver's own keying comment predicted it verbatim ("keying by stem alone would
give every conifer variant 0's crop and clip eight of nine") — the registration side
learnt cells, the ingest/resolve sides never got the matching move.

**Fix (mirrors `moverSlotTexture`, human-pawns P3): the variant rides the STEM.**
`thingTexture` routes `<base>/<v>/e` when the manifest lists it (`has`), canonical
otherwise; `registerThingSubframes` puts each mastered variant's rect on ITS OWN stem
at `#0` (linked kinds and true grid entries — probed via the new `resolver.gridOf` —
keep the per-cell dialect); `onTexturesChanged` re-registers + repaints THING rows when
a late manifest changes the routing (`thingArtSig`). Note the earlier "trees NOT
clipped" verdict (one-resolution P6) was measured on the canonical frame alone — the
clip was uniform across every tree, so nothing stood next to it to compare against.

**Two adjacent findings, deliberately NOT fixed here** (recorded for subframe-ingest,
their I12): the cold-thing `setSpriteScale` registration keys the BARE base stem, which
ingest never looks up — conifer's authored `sprite_scale 0.5` has been inert since
def-frame-anchors P5 (both before and after this fix; re-keying it would suddenly halve
every tree). And thing OVERRIDES painted before the manifest lands are not repainted on
a routing change (their raw params aren't cached); they self-heal on the next state
update.
