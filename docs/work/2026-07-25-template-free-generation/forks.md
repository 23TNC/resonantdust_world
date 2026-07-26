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

## F3 — Where does s/n control come from? · RESOLVED 2026-07-25 (P4)

East's silhouette cannot drive south/north — a quadruped's front and rear views are different shapes,
not transforms of the side view. The IP-adapter carries *appearance* consistency, not *shape*.
Candidates: per-direction corpus silhouettes from the same reference species (leans consistent, since
the bank already has all three); or `--control none` for s/n with IP-anchoring alone; or a hybrid
(corpus for shape, IP for identity). Resolve in P4 before implementing.

**CHOSEN: per-direction corpus silhouettes from the same reference species** — i.e. exactly what
`--control family:<f>` / `corpus:<Species>` already does, since the bank holds all three directions.
Measured on a red fox generated both ways at the same seed:

| | e↔s | e↔n | s↔n | geometry |
|---|---|---|---|---|
| `none` (IP anchor alone) | 0.124 | 0.273 | 0.183 | **south + north collapse to head-only busts** |
| `family:canine` | 0.144 | 0.286 | **0.078** | full body, correct tail-up convention in both |

It wins on *both* axes: tighter s↔n colour coherence and — decisively — correct geometry, which the
IP anchor cannot supply because it carries appearance, not shape.

**Rejected: `--control none` for s/n with IP-anchoring alone.** The fox proves the failure directly
(`.staging/p4/fox.png`, row 1): east is a good standing fox, but south and north render a head and
white chest with no body. Identity transferred fine; structure did not.

**Rejected: hybrid (corpus for shape + IP for identity as separate sources).** Not needed — the
chosen path already runs both: the bank supplies the per-direction silhouette *and* the east hero
still anchors s/n through the IP-adapter. There was no third mechanism to add.

## F4 — The geometry pass-gate · RESOLVED 2026-07-25

A candidate is **valid** when all four hold:

| check | threshold | catches |
|---|---|---|
| `blobs` | `== 1` | sprite sheets / multi-subject |
| `bg_uni` | `>= 0.75` | scenery bleed, non-keyable plate ([I5](issues.md#i5)) |
| `d_aspect` vs corpus reference | `<= 50%` | bust-instead-of-body, wrong facing, blobs |
| `solidity` | `>= 0.35` | wispy / fragmented output |

Calibrated against hand-judged sprites rather than picked a priori — it must reject the two known
failures and accept the four known-good, which it does:

| sprite | d_aspect | verdict |
|---|---|---|
| wolf east (the bust) | 58.5% | FAIL ✓ |
| bear east (the blob) | 56.3% | FAIL ✓ |
| tiger east (good) | 40.1% | PASS ✓ |
| bear north (user: "perfect") | 0.4% | PASS ✓ |
| wolf east @ new defaults | 0.4% | PASS ✓ |
| wolf south @ new defaults | 10.1% | PASS ✓ |

**50% aspect is deliberately loose** — the two real failures land at 56–58%, so a tighter gate would
start rejecting acceptable art (the good tiger sits at 40%). Rejected: a tighter 30% gate (fails the
approved tiger) and using the 0–100 `score` composite as the gate (it blends fill drift into the
verdict, so a correctly-shaped-but-differently-scaled sprite fails for the wrong reason — scale is a
P3 concern, not a validity one).

## F5 — Rejection granularity: per sprite or per variant leaf? · RESOLVED 2026-07-25

**Per LEAF.** If any direction of a seed fails the gate, the whole variant leaf is quarantined to
`_rejected/<seed>/`.

Rejecting per *sprite* was considered and dropped: it would leave a variant holding a good east and
no south, which is not usable as a set — the renderer expects a leaf's directions to belong to the
same animal, and a half-populated leaf is worse than none (it looks complete to the manifest walk).
Keeping the leaf whole also means a rejected candidate stays inspectable as a set, which is what the
curation step in the self-training flywheel needs.
