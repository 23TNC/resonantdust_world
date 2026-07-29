# Blockers — ns-shadows

_Rows: what blocks, why it needs the user, options, recommendation._

## B1 · The wolf-eyes live drill (P2) — wolf sprites don't render under art-128's in-flight textures

Found mid-P1: BOTH wolves' sprites vanished (render as a thin dark sliver). Bisect: identical
on 9bd328e-era code with my changes stashed — NOT this stream. Cause: the concurrent art-128
session rewrote `textures/pawn/animal/wolf/*` variants at 20:35–20:40 and my edge redeploy +
reloads began serving that in-flight state (resolver still returns frames; the page content is
what changed). P1 verification proceeded on a SYNTHETIC hot rot-2 caster (conifer side
frames) — everything measurable passed. NEEDS: the art-128 session to land/settle the wolf
masters, then the user's eyes on a walking n/s wolf (shadow sweeping e/w from the center
line, flipping sides across the light's column). Recommendation: re-run the live drill as a
one-item follow-up once wolf art is stable.
