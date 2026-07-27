# Forks — decision points

## F4 — How does a texel learn which casters shadow it? {#f4}
_2026-07-26 · open — the question underneath "why is it walking" ([I7](issues.md#i7))_

Three ways to store the caster↔light↔tile relationship. Today's choice is (b); the design that preceded it
was (a); (c) is untested and is the only one that changes the exponent.

**(a) Per-LIGHT caster LUT** — the 2026-07-21 design. `light_data` rows 1–32, ≤256 casters/light. Fragment
loops its tile's lights, then that light's caster list.
_Against:_ **no spatial pruning.** A reach-16 light covers ~1 024 tiles; at this world's ~0.5 casters/tile its
list is ~500 entries, every one tested at every texel in the disc behind only a box cull. Worse than the walk,
and almost certainly why it was replaced.

**(b) Per-TILE body buckets + reach-walk** — today, and what `VARIABLES.md` specifies. ≤8 casters per tile,
bucketed by the caster's own tilted-card ground extent, shared across all lights; the texel marches a
supercover DDA toward its light reading each crossed tile.
_For:_ cheap to maintain — O(casters) total, light-agnostic, so a light moving costs nothing to rebuild. Prunes
spatially; most slots are empty and exit at once.
_Against:_ the bucket **has no light association**, so the march is the only way to recover it — and the march
is `≥81 %` of frame time ([I6](issues.md#i6)). Cost per texel is `walk × 3 × 8`, up to 1 536 caster tests.

**(c) Per-LIGHT SHADOW buckets** — bucket a caster into the tiles **its shadow lands on**, per light.
_For:_ keeps (b)'s spatial pruning *and* restores (a)'s light association. A texel reads **only its own tile's
list**, and every entry is a caster that genuinely shadows it from that light. The march disappears. This is
the only candidate that changes `O(lit-texels × walk × casters)` into `O(lit-texels × casters-that-hit-me)`.
_Against:_ **state grows from O(casters) to O(lights × shadow area)** — the bucket is per light, so it must be
rebuilt when a light moves, not just when a caster does. That is precisely the moving-light case this stream
cares about, so the maintenance cost lands exactly where the win is and could eat it. Also needs a bucket
capacity decision, and overflow is a *correctness* issue (a dropped caster is a missing shadow), where today's
overflow merely drops a rarely-consulted slot.

**Not decided — (c) needs costing before it is chosen.** The measurement that would settle it: for a moving
reach-16 light, how many (tile, caster) pairs does its shadow set contain per frame, and what does rebuilding
them cost on the CPU relative to the ≥81 % of GPU frame time the walk currently spends? If maintenance is a
few hundred µs against 35 ms of walk, (c) wins outright; if it is comparable, the walk stays and the lever
goes back to reach and light count.

## F1 — Where does "move a prim" live? {#f1}
_2026-07-26 · **RESOLVED → (a). The entry point already existed; nothing needed re-homing.**_

**`Viewport.movePrim(id, x, y)` is the move**, and once [I3](issues.md#i3) was fixed it does the whole job —
verified with a single public call: sprite moved, light record moved, shadow map changed, and moving the prim
**back** returned the shadow map **bit-identical** (hash 336412233 → 3373222425 → 336412233). A reversible,
residue-free round trip is a stronger statement than the acceptance asked for.

**So (b)'s plumbing is not needed and would have been wasted work.** The lean was wrong for an instructive
reason: I reasoned that because the shadow side reads a per-frame snapshot it must therefore need an explicit
notification, and proposed a subscriber mechanism to carry one. But the per-frame scan *is* the notification —
`buildCasters` re-derives every carrier's position from `prim.x/y` each frame, and it always worked for
billboards. The single missing piece was that the light path skipped that re-derivation. **A design fork
argued from an unverified mechanism proposes machinery to route around a bug.**

Objection (a) raised — that `Viewport` would gain shadow policy — never materialised: the fix landed in
`coldShadowData`, where the record write already lived, and `Viewport.movePrim` is unchanged.

_Original analysis, kept for the record:_

A move must do two things that today live in different places: change the position the graph resolves from,
and tell the lighting what region went stale. Four candidates:

**(a) Extend `Viewport.movePrim`.** It already exists and already mutates `x/y` + `refreshPrim`. Add a
`ShadowGather` notification.
_Against:_ `Viewport` is the composition root; making it the place that knows lighting's dirty rules puts
shadow policy in a file that otherwise just wires. And `movePrim` reaches into `map` (the COLD SquareCache)
only — a warm/mover prim has a parallel `warmRefreshPrim` path, so the fix would have to be written twice.

**(b) A move on the PRIM GRAPH, with the caches as subscribers.** ← lean.
The graph already owns position ([primitive-graph](../2026-07-25-primitive-graph/README.md): "the graph owns
the position; the CPU stamps resolved positions"). A carrier moves its whole subtree, so a move is naturally a
graph operation, and `markPrimDirty`'s contract already says the rect must cover "the prim **and everything it
carries**". Cold cache, warm cache and `ShadowGather` all react to the one event.
_Against:_ more plumbing than (a), and the graph has no notification mechanism today.

**(c) Leave it in the per-frame scan — make `buildCasters` diff positions.** No new entry point: remember each
prim's last position and dirty on change.
_Against:_ **this is what we already have and it is the bug.** `buildCasters` sees a snapshot; a diff there
would work, but it makes movement an *inference from a scan* rather than a stated event, which is the same
shape as the item-detector that inferred machine state from prose and failed open for six days
([continuation-hooks](../2026-07-25-continuation-hooks/README.md)). It also cannot distinguish "moved" from
"was replaced by a different prim with the same id".

**(d) Movement only on the WARM/mover tier.** Pawns already have a mover path; make lights that move be warm
prims and leave cold immovable by construction.
_Against:_ a torch is a cold thing that may occasionally be picked up. Forcing kind→tier at placement time
means a carried torch is a different object from a placed one, which the primitive graph explicitly rejects
(a carried torch is the *same* prim under a hand prim).

**Chosen: (b)**, with (a) kept as the thin public wrapper so callers still say `movePrim`. The deciding
argument is ownership — every other consumer of position already reads the graph, so anything that changes
position anywhere else is a second authority, and this stream exists because of one of those.

## F2 — How do we make a moving light affordable? {#f2}
_2026-07-26 · **RESOLVED by P0's measurements → option 2 (bound reach). Option 4 struck.**_

### The decision
**Reach is the dominant cost term and bounding it is the fix.** At zoom 1, taking three moving torches from
reach 16 → 8 tiles moves them from **55.6 ms (18 fps) to the 120 fps vsync floor** — ≥6.7×, clearing P2's
target with no architectural change at all. Reach enters the cost three times over (walk length ∝ reach;
claimed texels ∝ reach²; lights overlapping each texel ∝ reach), so it compounds where the other levers are
linear. Full table in [I2](issues.md#i2).

**Option 4 (cheap-out the empty corridor) is STRUCK.** Measured corridor vs brute with lights orbiting at
zoom 1: **57.49 ms vs 87.74 ms**. The corridor already earns ~35 %, so the empty-space skip exists and works —
there is no large win hiding in it, and the precondition it was gating (*"if most of a disc is empty and still
pays full price, the architecture was never the problem"*) is answered: it does not pay full price.

**Options 1 and 3 are deferred, not rejected.** They stay available if bounding reach proves insufficient once
lights are numerous *and* moving, but neither is needed to hit the target now, and both cost real complexity
(option 1 must respect the coarsest-since-cast rule or leak light; option 3 trades correctness-in-time for
throughput). Preferring the measured 6.7× that adds no state is the whole point of measuring first.

_Original analysis, kept for the record:_

Options as analysed in the [README](README.md). Recording them now so P0 has a scorecard rather than a blank
page, and so a rejected option stays rejected with its reason:

1. **Hot lights bake at a coarser lod.** 2 steps = 16× fewer texels, same world coverage. Reuses
   [textile-slot P4](../2026-07-26-textile-slot/todo.md). Risk: a light that stops moving must *refine* back
   to full lod, and per-light lod must be the **COARSEST since last cast** or light leaks
   ([textile-slot I2](../2026-07-26-textile-slot/issues.md#i2)) — that rule is already written down and is
   exactly the trap this option walks into.
2. **Bound reach against the visible world.** Fixes the zoom cliff specifically. Cheap, but it is a *content*
   change (it makes torches smaller) and the user chose reach 16 for how it looked.
3. **Amortise the disc over N frames**, nearest-first. Bounded worst case. Risk: visible lag/smearing on a
   fast mover, and it interacts badly with 1 (a coarse bake that is ALSO late).
4. **Cheap-out the empty corridor.** Not really an option so much as a precondition — if most of a 16-tile
   disc is empty corridor and still costs full price, then none of 1–3 is the fix.

**Sequencing rule:** P0 measures 4 first. Options 1–3 compose and are chosen on the numbers.

_Rejected without measurement:_
- **Per-pixel screen-space evaluation of hot lights** (the "rt tier" of the original tiered design). The
  arithmetic kills it: ~2M screen px × N lights corridor-walked per frame is strictly worse than ~1 000 tiles
  × 256 texels baked once into a world map. The baked map is *already* the cheap representation; the problem
  is how often we throw it away, not what it is.
- **Delta re-bake (subtract old contribution, add new).** The accumulator is exact-integer additive, so this
  is *correct* — but it evaluates the light twice over the disc, i.e. 2× the cost it is meant to save. It is
  the right primitive for light REMOVAL, not for movement.

## F3 — Is reach content's call or the renderer's? {#f3}
_2026-07-26 · **RESOLVED → (a) content stays absolute. The authored value was simply wrong.**_

The measurements settled this differently from the pre-measurement lean:

**(c) is dead.** Clamping the dirty box while leaving reach absolute barely helps, because reach also sets the
**walk length per texel** — reach 16 with the box clamped to an 8-tile radius still walks 16 tiles from every
texel it does bake. The reach sweep shows the box shrinking from 512 → 423 tiles (−17 %) between reach 16 and
8 while the cost falls ≥6.7×, so the box was never where the money was.

**(b) is rejected on a principle this repo already committed to.** A renderer clamp that varies with lod makes
a light's extent depend on zoom, and [textile-slot](../2026-07-26-textile-slot/README.md) chose a fixed
reference precisely so **every player sees the same world** regardless of monitor or zoom. Trading that away to
paper over a content value would be the worse bargain, and a silent zoom-dependent clamp is the
"worked, then went away" bug shape flagged above.

**So (a): content is authoritative, and reach 16 was the wrong number.** At zoom 1 the visible area is 28 × 12
tiles and a reach-16 light claims 32 × 32 — it does not light a place, it floods the screen. Reach 8 is both
free (120 fps) and *reads as a light* at zoom 1. This is a one-constant change with no new machinery, which is
the outcome measuring first was supposed to produce.

**It is an aesthetic call as much as a performance one**, and the cost of each choice is now known
([I2](issues.md#i2)), so it can be re-picked at will: 16 → 18 fps, 12 → 30 fps, 8 and below → 120 fps.

_Original analysis, kept for the record:_

`&thing.light.reach` is authored per kind in `content/visual/things.rd`, in TILES. The renderer currently
honours it absolutely, which is how a torch came to light the entire screen at zoom 1.

**(a) Content is absolute; the renderer copes.** Preserves authoring intent exactly.
_Against:_ there is no cope that makes a 1 024-tile disc smaller than a 512-tile map.

**(b) Content states intent; the renderer clamps against the visible world.** ← lean.
Reach becomes a request, bounded by what the map can represent at the current lod.
_Against:_ a light silently behaving differently at different zooms is a new class of "worked, then went
away" bug — and this codebase has already been bitten by exactly that shape (the atlas-size gotcha: a shader
with a hardcoded page size silently broke whenever zoom switched LOD pools). Any clamp must be **observable**,
not silent.

**(c) Reach stays absolute, but the DIRTY BOX is clamped** — the light still throws as far as authored, we
just stop re-baking the far field every frame while it moves.
_Interesting:_ this is option 3 (amortise) wearing a different hat, and it keeps content authoritative, which
[VARIABLES-style ownership](../../VARIABLES.md) generally favours.

Not decided. (c) is the most honest if it holds up; (b) is the most certain to work.
