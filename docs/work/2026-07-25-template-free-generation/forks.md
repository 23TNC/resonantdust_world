# Forks — decisions taken, with what was rejected

## F1 — Where does the control silhouette come from? · RESOLVED 2026-07-25

The template's only surviving job is to supply a silhouette to ControlNet ([issues I1](issues.md#i1)),
so the fork is what supplies it when no art exists.

| Option | Verdict |
|---|---|
| **(a) No control at all** — txt2img + LoRA | **Kept as a mode** (`--control none`). Proven to produce plausible sprites (cn=0.00 wolf; txt2img wolf/tiger/bear/cat), but with no footprint or pose control and known collapses (wolf-east bust). Right for maximum freedom, wrong as the default. |
| **(b) Corpus-derived silhouette bank** | **CHOSEN as the generalized path.** `.staging/quad-lora-train` already holds 459 on-model sprites over 132 species × e/s/n. Their silhouettes are control images at **zero authoring cost**, already in the target style and the right convention per direction. |
| **(c) Hand-authored family template library** (~6 families × 3 dirs) | **Rejected as redundant.** It is a strict subset of (b) — a "family representative" is just a corpus sprite we picked. Authoring 18 images to reproduce what 459 free ones already give is unjustified. The *family mapping* idea survives inside (b) as a lookup table. |
| **(d) Procedural / parametric silhouette synthesis** | **Rejected for now** (out of scope, not refuted). No evidence it beats a real sprite, and high effort. Revisit only if (b) proves too coarse for a body plan the corpus lacks — which P5 is designed to detect. |

## F2 — Keep the hand-authored template path? · RESOLVED 2026-07-25

**Yes — `--control template` stays and remains the default.** Explicit art must always outrank an
inferred silhouette: the wolf case is the best-controlled result we have, and the user may author a
template precisely to pin a pose the corpus cannot express. Removing it would regress a working path
for no gain. The generalized modes are additive.

## F3 — Where does s/n control come from? · OPEN (P4)

East's silhouette cannot drive south/north — a quadruped's front and rear views are different shapes,
not transforms of the side view. The IP-adapter carries *appearance* consistency, not *shape*.
Candidates: per-direction corpus silhouettes from the same reference species (leans consistent, since
the bank already has all three); or `--control none` for s/n with IP-anchoring alone; or a hybrid
(corpus for shape, IP for identity). Resolve in P4 before implementing.
