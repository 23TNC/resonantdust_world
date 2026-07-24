# Forks — world geometry

_Decision points + options + which we chose + why. Chronological._

---

## F1 · Where the canonical model lives {#f1}
**2026-07-23 — open (P0).** This stream is *flowing* state; the geometric model is **durable truth** and
must outlive it. Options: (a) a `design/` doc under the client component (it's a rendering model —
"the shape"); (b) a top-level cross-cutting doc alongside `VARIABLES.md` / `TABLES.md`, since geometry
spans everything that draws or casts. Lean **(a)** — it's a client-rendering model, not a data layout —
with this stream's README linking to it, not restating it (a layout/model must live in exactly one place).

## F2 · Conform the caster projection, or keep the tuned card? {#f2}
**2026-07-23 — lean: CONFORM the code to the model.** `casterCover` places the card leaning
(`0.5·H·cos65` north, `H·sin65` up ⟹ `2·tan65` per screen-unit) vs the confirmed `sin65`. Options:
(a) **conform** — re-derive the projection from the ratified geometry; the shadow becomes explainable and
the prim-shadow work inherits one vertical frame; (b) **keep the card**, document it as a deliberate
art fiction, and give receivers a matching constant. (b) preserves the current tuned look with less
churn, but leaves two models in the codebase — which is the exact problem this stream exists to end.
Take (a); if the look regresses, tune *within* the honest model rather than reintroducing the card.

## F3 · What happens to `SHADOW_LIFT` {#f3}
**2026-07-23 — expect it to trend to ~0.** The 3-unit lift was measured empirically to seat shadow bases
on sprite bases — i.e. it is compensating for a base misplaced by the old card. If the model is right,
the seating should be correct *by construction*. Keep a small dial only if the art genuinely wants one;
don't preserve the number just because it's there.

## F4 · One tilt constant {#f4}
**2026-07-23 — yes.** `65.0` is currently written as a literal inside shader math in more than one place.
Name it once (shared constant + the model doc) so a future change is one edit, and so the value is
visibly *the* world tilt rather than a magic number per shader.

## F5 · Scope — align before or after prim shadows? {#f5}
**2026-07-23 — DECIDED: before.** [`shadows-on-prims`](../2026-07-23-shadows-on-prims/README.md) needs a
receiver's true elevation. Building it while casters still project on the leaning card means two vertical
frames and constants tuned against a fiction. Align first (this stream), then build prim shadows on it.
