# Forks — bug sweep

## F1 — numeric z-order tiers; last-active wins ties; On Top dies {#f1}

The user's design. `DomPanel`'s banded z machinery (named bands + a per-band recency
counter + the `_onTop` escape hatch) becomes: each panel declares `zOrder` (a small
number; the tier), CSS z-index = `zOrder × STRIDE + tier-recency`, and
`bringToFront` advances the TIER's counter only — so a click reorders panels within
their tier and can never lift one over a higher tier. The assignment:

| panel | z-order |
|---|---|
| game view | 32 |
| details, inventory | 40 |
| chat, build | 48 |
| settings, debug HUD | 56 |

`inventory` rides beside details (a sibling info panel — the user's table predates
it); transient chrome (the pie menu, tooltips, the settings POPUP) sits above every
tier at a fixed top band — chrome is not a panel and takes no part in the ordering.
The On Top settings row, `_onTop`, its storage key, and the `ontop` band are
DELETED. Rejected: keeping On Top as "z-order 63" — the user asked for its removal,
and an escape hatch would erode the tiers the moment it existed.

## F2 — the geo flash: never geo between two RESIDENT textures {#f2}

The user rules out texture loads, and e/w agrees: west is the SAME east texture
mirrored (texture-serving model), so a facing flip has nothing to fetch — yet it
flashes. The suspect is the warm re-bake path: a facing change writes a new
`texName`/subframe through the eps/skip compare, and somewhere between "prim
updated" and "texture bound" the bake emits the geo-silhouette fallback for a
frame. The rule after the fix: a prim swapping between two RESIDENT textures never
draws geo — hold the OLD binding until the new one is bindable in the same bake.
The first-load geo silhouette (albedo-outlines memory: geo-until-loaded is the
DESIGNED look) stays untouched (I3).

## F3 — walls block through the derived-pathability law {#f3}

`pathable = false` on the `wall_smooth` tile def — one TOML line; pathability is
DERIVED (pathfinding F1), so the worker's A*, the wasm glide, and the npc estimate
all follow without code. Walls already refuse `move_to` as a DESTINATION (they bind
no interactions — input-rework F9); this makes them block ROUTES too. Consequences
accepted: enclosures genuinely trap (I4), and the six-consumer content law applies
(I7). Rejected: a wall THING — walls are tiles by build-walls' design; the tile
flag is where the law already lives.

## F4 — the outline is the union silhouette of ALL the object's prims {#f4}

The `OutlineOverlay` outlines a prim's own alpha and draws topmost — a body ring
crossing the head (the user's screenshot). Fix at the MASK: stamp every prim of the
selected object into one silhouette mask (same atlas pages, same subframe crops,
same flips as the real draw — I5), then edge-detect the MASK. Edges interior to
the union (body pixels under the head) vanish by construction; the ring hugs the
composite figure. Rejected: per-prim outlines with sibling-alpha stencil tests —
same result, N passes instead of one mask.

## F5 — the vanishing wolf: evidence before surgery {#f5}

"At least identify the conditions" (user). The plan is a REPRODUCTION HARNESS, not
a guess: drive the wolf on long cross-zone trips at speed while recording (the gif
tool), with a probe logging per frame when any mover's carrier prim (a) skipped its
bake via the eps gate, (b) resolved a zero-area subframe, (c) lost its warm prim
(cache eviction / zone close), or (d) drew outside the camera's culled set. Every
vanish correlates against the probe log; the conditions land in issues.md with
captures. Ordered suspects: zone-close mover drops on boundary crossings; a facing
whose authored subframe never registered (skip-draw); eps/skip eating a re-bake
after a snap; warm-cache eviction; the chase-snap teleport; the subtile anchor
placing the sprite off-screen. Fix only what proves cheap; the rest becomes named
successor issues.
