# ns-shadows — e/w shadows from n/s billboards (the perpendicular card)

_Work stream, opened 2026-07-28. Component: `client/webgl` (`shadowGather`, `coldShadowData`),
plus `docs/VARIABLES.md`. User's brief (with a 3D mockup): implement e/w shadows on n/s
billboards — "when we cast a shadow from a n/s billboard we will cast from the center line
east or west." Lineage: pawn-render [F5](../2026-07-28-pawn-render/forks.md#f5) deferred
exactly this ("perpendicular-card modeling is DEFERRED until the simple version visibly reads
wrong") — it now reads wrong, and the record's true rotation code (0=s 1=e 2=n 3=w) was wired
end-to-end there so this consumer could exist._

## The geometry (user-ratified, 2026-07-28)

An **e/w billboard** is the standard screen-facing card: it spans east–west + height, leans
back by `uCardLean` to match the oblique view, and its shadow projects north/south-ish away
from the light. An **n/s billboard** is the SAME physical subject rotated 90° about the
vertical axis: the card is a **vertical plane containing the north–south axis and height**,
standing on the sprite's **center line** (the user's mockup: the tall standing plane; the
dark band is its shadow sweeping e/w across the ground; the small leaning card is the e/w
billboard for contrast). It does NOT lean — it stands perpendicular to the ground.

Which side the shadow falls on needs **no special casing**: with the card as the plane
`x = centerline`, a light east of it projects the silhouette westward and a light west of it
projects eastward — straight from the ray–plane projection.

## Design decisions

- **D1 · Silhouette source = the SIDE (east) frame of the same stem.** The n/s frame's art
  faces the camera and cannot paint a plane seen edge-on. The side profile is the correct
  silhouette for an e/w sweep, and its WIDTH doubles as the card's n–s extent (a wolf is as
  long in side view as it is deep walking north). West = mirrored east, as everywhere.
- **D2 · The card's n–s span is CENTERED on the anchor row** (base-centre anchor), height =
  the side frame's height. Tunable if contact reads off — log the retune as a fork.
- **D3 · Head orientation follows facing.** The east frame's head end (+s in frame space)
  maps to NORTH for a north-facing (rotation 2) subject and to SOUTH for south-facing
  (rotation 0) — one flip on the card's s axis, the rot-3 mirror's sibling.
- **D4 · The caster def rides `billboard_data.A`** (fully reserved today): u16
  `caster_definition_id` + flip/valid flags, stamped by the CPU at record write for
  rotations 0/2. The DRAWN def (B word) is untouched — drawing, receiving, and normals keep
  the facing's own art; ONLY the caster geometry changes.
- **D5 · Every shadow mode inherits the arm for free.** The rotated card lands inside
  `casterCover`, which `casterOne` calls in all caster modes (cold pass skip-hot, hot delta,
  full) — the tier matrix and the cold-light delta need no changes.
- **Scope**: movers (hot class) are the only n/s billboards today — cold things are
  single-facing (`DEFAULT_FACING`). Cold n/s prims inherit the arm free when they exist.

## What this stream does NOT touch

The n/s billboard as RECEIVER (shadows climbing onto it, its normal, its drawn sprite) —
all already keyed to the facing's own art by frame-follows-facing (pawn-render F5-AMENDED).
The receiver maps, the blit, and the hot-sync correction model are unchanged.
