# Forks — the east pipeline

_A choice I resolved, with what was rejected and why. A fork is mine; a
[blocker](blockers.md) is the user's._

## F1 — Measure the one-flag fix BEFORE building any pipeline {#f1}
_2026-08-08 · resolved at plan time · **S1 runs first, and it may end the stream**_

**Chosen.** [P1](todo.md)'s S1 — point each species at its own corpus silhouette
(`--control corpus:<Species>`) — is measured before a single stage is built.

**Why.** It already exists ([I2](issues.md#i2)), it is one flag, and it directly removes the cause
of the defect that opened this stream ([I1](issues.md#i1)). Building a silhouette-generation stage
first, and only then discovering that reading the answer off disk scores as well, would be inventing
a solution to a solved problem while the solution sat in the repo.

**Stated plainly so the result is not a disappointment:** if S1 wins outright on trained species,
that is the *correct* outcome and the multi-stage work is not wasted — it moves to
[P4](todo.md), where there is no corpus silhouette to read and a stage that *generates* one is the
only option. **S1 cannot generalise to untrained species by construction**, so P2 keeps its
justification whatever S1 scores.

**Rejected — start with the user's silhouette→texture proposal.** It is the more interesting
build and it is the right long-term shape. But an unmeasured baseline makes its number
uninterpretable: "two stages score 0.72" means nothing without knowing that one flag scores 0.70.

## F2 — Trained species are a calibration set, not the deliverable {#f2}
_2026-08-08 · resolved at plan time_

**Chosen.** [P0](todo.md)–[P3](todo.md) work entirely on species the LoRA was trained on and whose
real sprites we own; untrained species are [P4](todo.md) and use the winner unchanged.

**The obvious objection, answered:** if the corpus already contains the real east sprite, generating
one is pointless. True as a product, false as a method check — **a pipeline that cannot reproduce an
animal it has seen will not invent one it has not**, and trained species are the only place
`iou_ref` is defined ([I3](issues.md#i3)), so they are the only place a method can be scored against
a known answer rather than against taste.

This is the user's own ordering — *"Once we have a decent method to handle this, we will attempt
animals we do not have training data for"* — and the reason it is right is that it buys a ruler.

**The limitation this creates, recorded now:** the method that wins on trained species is chosen by
a metric that will not exist for the animals it ultimately has to serve. P4 must therefore be run
before the method is called settled, not after — which is why it is in this stream rather than
deferred to the next one.

## F3 — East only, by instruction, and the reason it is also correct {#f3}
_2026-08-08 · the user's scope call, adopted_

**Chosen.** East alone. South and north are a follow-up stream.

**Why it is also the right technical call.** East is the strongest cell in every run this project
has made: the design doc calls side profile "in-distribution, *easy*" and front/back "*hard* — SDXL's
side-view prior resists turning the subject", and the predecessor measured east as the only
direction passing 10/10 in both control modes. A method that cannot win on east cannot win anywhere,
so east is the cheapest possible falsification.

**The cost, stated:** a method tuned on side profile may not transfer. `silhouette_bank` holds
e/s/n for all 133 species, so the S1 path transfers trivially; a *generated* silhouette stage
(P2) is the one at risk, because generating a correct front view is the exact thing this project has
repeatedly failed at. [P5](todo.md) names what the follow-up needs rather than assuming it inherits.

## F4 — `iou_ref` leads; the eye ratifies; interior metrics are secondary {#f4}
_2026-08-08 · resolved at plan time_

**Chosen.** `iou_ref` against the real corpus sprite is the primary number. Signed aspect,
interior luminance/saturation and the structural gate are reported beside it. No verdict is accepted
before the sheet is looked at.

**Why `iou_ref` and not the predecessor's metrics.** Every one of them approved of a wolf-shaped
bear ([I3](issues.md#i3)), and `iou_control` actively rewarded it. `iou_ref` is the only available
statistic that compares against the animal rather than against the instruction.

**Why the eye keeps a veto.** Five times now the images have overturned the numbers in this project
— the anteater, run-4 south, run-4 east, run-4 overall, and on 2026-08-08 the prompt-form comparison
where the metric called two forms equivalent and one had a glowing disc stamped on the wolf's back.

**What `iou_ref` cannot see, so it does not decide alone:** it is a silhouette statistic. A correctly
shaped bear painted in the wrong colours scores perfectly. That is what the interior metrics are
for, and it is why colour fidelity against the real sprite is reported per method even though hue is
discarded downstream.

## F5 — Avenues carried rather than scheduled {#f5}
_2026-08-08 · resolved at plan time · **named so they are not silently lost**_

The user asked for other promising solutions to be investigated. These are real and are **not** in
[P1](todo.md)–[P3](todo.md), each for a stated reason. They are written here so that dropping them
stays a decision.

| avenue | why it is not scheduled yet |
|---|---|
| **hires second pass** (generate at 512, latent-upscale, re-denoise at 1024) | a quality lever, not a *species-identity* lever; it cannot fix a wolf-shaped bear. Fold into the winner afterwards. |
| **palette conditioning from the real sprite** via IP-Adapter on colour only | only defined for trained species, so it cannot be part of a method that must serve untrained ones. Worth it if colour fidelity is the residual after P3. |
| **regional colour** — segment the silhouette, paint per region | the layer-map decomposition downstream already does material separation; doing it twice risks fighting it. Revisit if flat-region count is the failure. |
| **edit-model restyle** ("make this wolf a bear") with Qwen-Image-Edit | the weights are still downloading; and it belongs to the [direction-consistency P6](../2026-08-08-direction-consistency/todo.md) evaluation rather than being duplicated here. |
| **per-body-plan authored templates** | the honest alternative to generating silhouettes, and it is *hand-authoring work*, which is a different kind of project. `family:<f>` already approximates it from the corpus at zero cost. |

## F6 — The silhouette stage emits a FILLED SILHOUETTE, and `edge_map()` still derives the edges {#f6}
_2026-08-08 · resolved at P2.1_

**Chosen.** Stage 1 emits the generated sprite's **alpha, flattened to a solid dark shape on white**.
Stage 2 receives that image and `edge_map()` derives its control exactly as it does for a bank
silhouette or a hand-authored template.

**Why not pass the stage-1 sprite itself.** `edge_map()` is `FIND_EDGES` + threshold over the whole
image, so a full sprite contributes **every interior line** — muzzle, eye, fur breaks, colour
boundaries — not just the outline. Feeding that to stage 2 at `cn 0.5` would lock in stage 1's
interior *drawing decisions*, which is precisely the thing stage 2 exists to redo. A filled shape
contributes one closed contour: the silhouette, and nothing else.

**Why not emit a pre-baked edge map.** `silhouette_bank`'s docstring already settled this for the
bank and the same reason applies here: *"Emitting the sprite on white rather than a pre-baked edge
map is deliberate, so the pipeline's own `edge_map(rgb, thresh)` must stay the single place edges
are derived… and `--edge-thresh` keeps working."* Two edge derivations would drift, and a stage-1
output that bypassed `--edge-thresh` would silently ignore a knob every other control respects.

**Why not lineart.** It is a third representation with no consumer — `CN_MODEL` here is a generic
SDXL ControlNet fed `edge_map` output, not a lineart-specific model. The
[`lineart-lora`](../2026-07-27-lineart-lora/README.md) stream is where a real lineart path belongs.

**Consequence, stated:** the filled shape discards stage 1's interior entirely, so stage 1 is judged
**only on shape**. That is the correct division for a stage named "silhouette", and it is what makes
[P2.2](todo.md)'s acceptance — "a silhouette whose `iou_ref` beats the wolf template's, judged as a
silhouette alone" — meaningful rather than a proxy.
