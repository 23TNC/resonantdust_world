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

## I11 — the vanishing wolf: the evidence file (2026-08-08) {#i11}

**No structural vanish reproduced this session**, and the probe makes that a
statement, not a shrug. The harness: a 100 ms probe over EVERY mover logging
(a) warm prims missing from the viewport, (b) zero-area prims, (c) lost
texNames, (d) movers dropped from the layer — run through 4 minutes of long
cross-seam trips ((98,60) ↔ (120,75), zone seams x=112/96 y=64) and 2 minutes
of hard camera panning across 9 world spots (zone churn). ZERO events — which
ELIMINATES: prim loss, zero-size resolves, texName loss, and zone-close mover
drops (movers survived every pan; the release/LRU pressure hypothesis found
no release at these budgets).

**The strong candidate is the P2 root cause itself**: before this stream, the
subframe re-registration cycle EVICTED the wolf's pack on every facing change
— and with two same-kind pawns at different facings (I9) the eviction churn
was CONTINUOUS. During each repack window the resolve answered geo; depending
on which maps were mid-cycle, a mover could draw wrong or effectively not at
all — during movement, intermittently: the report's shape. The P2 fix (never
geo between resident packs) removes that whole class. Verdict: PLAUSIBLY
CURED BY P2; re-observe before hunting further.

**Remaining suspects if it recurs** (none catchable by this probe): correct-
but-surprising warm-over-cold occlusion behind tall sprites (walking north
through forest), and the render-chase snap on interrupts. The probe is
re-armable from the console — paste:

    window.__vanishLog=[];window.__vanishSeen=new Map();setInterval(()=>{const
    t=Date.now(),s=window.__vanishSeen,l=window.__vanishLog,v=new Set();for(const
    [id,m]of window.__movers){v.add(id);const P=m.parts.map(p=>window.__viewport
    .warmGetPrim(p.id));const st=`${P.filter(p=>!p).length}m${P.filter(p=>p&&(p
    .width<0.5)).length}z${m.parts.filter(p=>!p.texName).length}t`;const pr=s.get(id);
    if(pr&&pr.state!==st)l.push({t,id:id.toString(16),was:pr.state,now:st});
    s.set(id,{state:st,t});}for(const[id,pr]of s)if(!v.has(id)){l.push({t,id:
    id.toString(16),was:pr.state,now:"GONE"});s.delete(id);}},100)

then read `window.__vanishLog` the moment a vanish is SEEN.

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
