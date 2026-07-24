# Issues — shadows on prims

_Bugs, gotchas, post-mortems. Chronological._

---

## I-1 · The re-sample shortcut self-shadows (why pass 2 exists) {#i1}
**2026-07-23 — the finding that drove the architecture.** Before pass 2, we tried the cheap version:
re-sample the **combined** baked ground shadow at the re-projected point `G`. It darkens lit trees — a
crown reads shadowed by its own trunk.

Cause: `G` is on the ray `light → crown → G`, but the blocker on that ray is the tree's OWN body, which
sits **between the crown and `G`**, not between the light and the crown. So "`G` is shadowed" is true while
"the crown is shadowed" is false, and a *combined* ground-shadow (all casters, one scalar) can't tell them
apart. The correct evaluation is **per-caster at the elevated point, excluding the receiver's own
(same-tile) caster** — which is exactly what pass 2 does. The re-sample scaffold (a `__depthmode 2` +
`__climb` path) was **removed** once the finding was recorded, to keep the blit debuggable; the working
default is the binary front/behind (`__depthtest`).

## I-2 · GLSL foot-guns to carry over {#i2}
**2026-07-23 — reminders.** From the sibling shadow work: a backtick anywhere in a `/* glsl */` literal
(even a comment) breaks the build (a Stop/PostToolUse hook guards it — hit repeatedly this session); a
body-modified var in a GLSL for-condition + dynamic vector subscripts can miscompile/freeze ANGLE (use
constant loop bounds + `break`, static lane ternaries). Pass 2 reuses the corridor, so both apply.
