# Blockers — cast shadows onto prims

_Things that stop progress and need input/resolution. Chronological._

---

## B-1 · The receiver/caster vertical OFFSET model — RESOLVED 2026-07-24 {#b1}
**RESOLVED (user diagnosis + fix verified working).** The confusion was conflating TWO distinct things
under one "offset":
- **The shadow field** (`SHADOW_LIFT = 3`) — correct, unchanged. Moving it doesn't help; that was a red
  herring I chased.
- **Where we CUT** — the receiver MASK (`receiverCover`) was cutting the sprite-shaped hole ~2 units too far
  SOUTH of the drawn albedo (def tight-bbox base vs drawn opaque base). Fix: shift the MASK north by
  `RECV_ALIGN = 2` units (`Ac.y -= 2`), so the cut seats on the sprite. The dial-by-eye `recvOff` knob was
  the wrong lever and is gone.

Also resolved in the same pass: **self-casting**. The seen-face row cull (`cr > baseRow`) *should* kill
self at `receiver.row == caster.row`, but the receiver row derives from the tight-bbox base (`Ac`) while a
caster's row derives from the raw anchor (`prim.y+height`) — so "same prim" was never "same row". Replaced
with an **exact prim-id self-exclusion**: `receiverAt` returns the winning receiver prim, and `casterOne`
drops that exact prim (the user's "prim == prim → cull", done robustly). Verified in-browser: self-casting
gone, mask aligned to the sprite.

**Remaining (user: "artifacts we can deal with later"):** minor residual artifacts; and the seen-face/
light-side culls still compare the caster's raw-anchor row against the receiver's tight-bbox row (a small
basis mismatch, harmless now that self is excluded by id). Fold into P3/P4 polish.
</content>
