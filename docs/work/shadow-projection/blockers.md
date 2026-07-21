# Blockers — shadow-projection

_Things needing human input / external state before they can proceed. Open → resolved (resolved rows archive
with a date)._

---

## B-1 · P5 (N/S facing regime) is gated on multi-facing casters — 2026-07-21 (open)

**What blocks:** P5 implements the **N/S (front/back)** shadow regime (rolled `R_y(±90)`, symmetric ±depth on
x). Building it is straightforward — the sandbox has the exact math — but it can't be **verified**: there are
**no N/S-facing casters in the world**. All current cold things are single-facing (`DEFAULT_FACING` → E/W, the
regime P0–P4 already ship), so every resident caster is E/W. The only multi-facing entities are **movers**
(wolves: e/s/n from rotation), and none are present — the npc driver isn't running.

**Why it needs a human:** shipping an unverified regime risks a silently-wrong shadow shape (the whole point
of the per-phase in-browser verification). I won't build a regime I can't look at.

**Solution analysis:**
- **(a) Run the npc driver** so wolves spawn with n/s facings → real N/S casters to build + verify against.
- **(b) A temporary debug** forcing a caster's facing to N/S (throwaway infra for content that doesn't exist).
- **(c) Defer P5** until movers / multi-facing things land — the stream's **core is already delivered**
  (GPU-instanced, silhouette-masked projected fans, E/W, matching the sandbox), which was the request.

**Suggested path:** (c) — defer. The user's ask ("same geometry, on the GPU, the triangles we need") is done +
verified. Re-open P5 when there are N/S casters (enable the npc driver, or when multi-facing cold things
exist); the N/S branch then drops into the same vertex shader (`aParam.w` = regime/roll is already reserved).
