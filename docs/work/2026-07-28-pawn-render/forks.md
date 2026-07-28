# Forks — pawn-render

_Decisions resolved (or leaned) at plan time; each names the rejected options and why._

## F1 · Depth = the zdepth-world key that already exists, compared in the blit

Both tiers have baked `0x80 | (baseRow & 0x7f)` into `zdepth-world`'s B lane since the warm
cache landed (`Viewport.ts:199-205`) — the blit binds both composites and reads neither. The
fix is a wrap-aware serial row compare in the ONE shader that composites (viewport spans ≪
64 rows, so mod-128 serial arithmetic is unambiguous), the winner carrying albedo + surface
+ emissive + the light-select (F3). Rejected: a new/wider depth channel (nothing needs more
than the base row — the painter's key cold things already sort by); rejected: depth-testing
at bake time (the tiers bake independently and at different cadences — the decision point
is the composite); rejected: CPU-side splitting of mover sprites (the whole point of the
per-pixel key is not doing that). Guard: the zdepth reproject is NEAREST for exactly this
reason (`SquareCache.ts:124-127`) — keep it that way.

## F2 · Warm shadow/light data rides UNIFORMS in the cold record format

The hot-shadows stream's ratified design: hot data is passed by uniform in the SAME packed
layout as the cold data textures, so the shader decodes a record identically from either
source. Movers are a handful; a small per-frame uniform upload beats `texSubImage2D` churn,
and re-uploading per frame is the hot tier's nature anyway. Rejected: extending the cold
data TEXTURES with mover rows (per-frame texture uploads + dirty-tracking complexity for
data that changes every frame by definition); rejected: a bespoke mover format (two decode
paths forever).

## F3 · The blit's light-select IS the tier matrix

Per pixel: warm wins depth ⇒ `ambient + hotLight`; else `ambient + coldLight + hotLight`
(unchanged). The hot pass computes warm-receiver texels against ALL lights (cold + hot) with
the wolf's own normal + receiver mask, so the hot map already holds the wolf's complete
lighting — adding the cold map on top would double-count the cold lights, and consulting the
cold map for a mover pixel is wrong anyway (it holds the GROUND's bake: ground N·L, ground
shadow — the exact "behind shadow-cold" artifact this stream kills). This selection
implements every cell of the user's matrix: cold×cold stays baked; anything touching the
mover is recomputed per frame into hot; a moving wolf never dirties `lightmap-cold`.
Rejected: subtracting/masking cold contributions (fragile); rejected: a third "warm"
lightmap RT (the hot map's per-frame cadence already matches — hot-shadows reserves finer
tiering for its budget system).

## F4 · Mover participation extends the LIVE receiver/climb machinery, not attempt #3's plan

The billboard-receiver machinery attempt #3 planned (receiver coarse/fine maps, the
`zElev = k·(baseY − y)` climb, seen-face + light-side cone culls, the on-billboard shadow
bit) is ALREADY LIVE in `shadowGather.ts` — built by the lightmap-resolution/lighting-feel
line after that folder was written; its 0/8 todo is stale. This stream adds warm prims as
receivers/casters IN THE HOT PASS using that machinery; the wrap phase reconciles the stale
folder rather than leaving two competing plans. Guard rails inherited wholesale: no
world-coord reads of `textile_slot` composites from light/shadow passes (the attempt-#2
revert); receiver identity from in-family maps; zoom stability as an acceptance test.

## F5 · N/S casting: the FRAME changes, the card doesn't (yet)

An n/s-facing wolf is the same physical wolf; what the caster must project is the n/s ART's
silhouette. So: records carry the true rotation (0=s 1=e 2=n 3=w — the two free values of
the existing 2-bit code), the referenced atlas frame follows the facing, and
`sampleCard`/`casterCover`/`billboardNormal` grow the n/s arms (u-mapping + mirror rules per
rotation; normals sample the n/s frames). The card GEOMETRY stays the standard billboard
plane — modeling n/s cards as perpendicular 3D planes (different shadow foreshortening) is
DEFERRED until the simple version visibly reads wrong; the user flagged "may require
additional shader work", and this is the minimal shader work that makes an n/s wolf cast an
n/s shadow. Cold prims with n/s rotations inherit the fix free. Rejected for now: full
perpendicular-card projection (real complexity, unproven visual need).

## F6 · Depth flip line = base row (painter's key), not per-pixel height

Front/behind between billboards resolves by BASE ROW alone — the same key cold things
z-sort by, now honored per pixel between tiers. Two overlapping billboards on the SAME row
keep today's coverage order (warm over cold) — acceptably rare at wolf scale, revisit only
if it visibly pops. Rejected: true per-pixel height intersection between tilted cards
(needs per-pixel card height reconstruction in the blit for a case the art style barely
produces).
