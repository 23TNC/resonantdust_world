# Issues — bug sweep (anticipated; logged before they bite)

## I1 — stale stored panel state must not resurrect On Top {#i1}

Panels persist per-`storageKey` state; `<key>.onTop` rows exist in live browsers.
The removal must IGNORE the key (and may delete it on first load) — a migration
that re-reads it as "z-order 63" would rebuild the hatch we are deleting. Verify
with a browser that had On Top enabled before the change.

## I2 — the game view is a PANEL and must stay the floor {#i2}

z-order 32 is the world itself: every other tier must draw over it, and its
`bringToFront` (clicking the world) must never lift it past 40. Input is
unaffected (lowest tier still receives its own clicks), but verify the taskbar
and drag chrome still render above the canvas.

## I3 — the geo flash fix must not break the LEGIT geo silhouette {#i3}

Geo-until-loaded is the DESIGNED first-load look (albedo-outlines memory), and
tint-square placeholders draw geo FOREVER by design. The fix targets exactly one
case: a swap between two RESIDENT textures. A probe that can tell the three
apart (first load / placeholder / resident swap) is part of the acceptance.

## I4 — impassable walls trap what they enclose {#i4}

The standing wall pen near (105–110, 60–65) will enclose its interior: orders
INTO it refuse (F5 — correct), pawns INSIDE cannot leave, and npc wander picks
inside/outside the pen burn deadlines until re-rolled. Expected behavior, stated
here so nobody reads a trapped drill pawn as a pathfinding regression. Doors are
a recorded successor, not this stream.

## I5 — the outline mask must sample EXACTLY like the draw {#i5}

Subframe crops, atlas page choice per zoom (textureSize, never hardcoded —
the atlas-size gotcha), mirrored-west flips, and the co-pack's data/graphics twin
pages: the mask pass reuses the draw's sampling or the ring misaligns by a crop
offset. The neck test in the user's screenshot is the acceptance image.

## I6 — the wolf hunt needs the vanish to happen ON CAMERA {#i6}

The disappearance is intermittent; the harness must make it cheap to catch:
long scripted trips (east–west, crossing zone seams), the gif recorder running,
and the per-frame probe log timestamped so a vanish frame indexes into the log.
If it will not reproduce in a session, record THAT (with the attempted
conditions) rather than guessing — the deliverable is identified conditions.

## I9 — FOUND: same-kind pawns at different facings fight over subframe cell 0 {#i9}

The geo-flash root cause exposed a deeper pre-existing defect: `subframeSlot`
registers the CURRENT facing's rect under `<stem>#0`, so two pawns of one kind
facing differently RE-REGISTER against each other on every visual apply —
a continuous repack churn (previously: continuous geo flashing; now: stale-crop
serving, so at worst a briefly-wrong facing crop on one of them). The real fix is
per-facing subframe CELLS applied at RESOLVE (subframe-ingest F6's direction) with
the prim passing `cell = rot` — a successor stream touching pack layout, bbox, and
the shadow card. Recorded, not built here.

## I10 — FOUND: fixed-position chrome had hand-rolled z literals {#i10}

The pie menu (z 60000, reported invisible by the user mid-stream), both tooltips
(60001), and the taskbar (50001) all sank under the game view's new tier z
(320001). All chrome now rides `Z_CHROME_BASE` (tier 64 × the stride): taskbar +1,
pie menu +10, tooltips +11. Any FUTURE fixed-position chrome must use the constant
— a literal is exactly how these four broke.

## I7 — the wall flag is a CONTENT change: the six-consumer law {#i7}

`pathable = false` on wall_smooth → golden re-blessed, worker/npc/wasm/webgl/
master/edge rebuilt-or-restarted, browser wasm cache-busted. The npc's wander
pathable-pick and the client glide read the same flag — no extra work, but the
rebuild sweep is not optional.

## I8 — the z-order change touches every panel constructor {#i8}

Details, inventory, chat, build, settings, debug HUD, viewport — each names its
tier at construction; a missed one lands in a default tier and draws wrong. Grep
for every `new DomPanel`/subclass site and assign explicitly; the acceptance
captures cover each pair from the user's table.
