# Blockers — plane intersection

_Things needing human input: what it is, the analysis, WHY it needs a human, and the suggested path.
Resolved rows stay with a date. The goal is fewer of these over time — a blocker I understand well enough
becomes an issue or a fork I resolve myself._

**Nothing is blocking P2 or P3.** Both are measurement and code I can carry out. The rows below are
decisions that will block LATER phases, filed now so they are not discovered at the moment they bite.

## B1 — F5's distance-field option needs a corpus re-bake · OPEN, blocks P4

**What.** [F5](forks.md#f5) picks how the penumbra gets a smooth gradient across interior silhouette
detail. Option (b), a signed distance field in the atlas, is the only candidate whose cost does not grow
with softness — it makes the penumbra **O(1) in width** and would delete the reason the tap ladder exists
rather than tuning it.

**Why it needs you.** It is the only option that touches the **art pipeline**. `bin/art` would have to
generate the field, and because the co-pack layout is definition-authoritative (the GPU never re-resolves
by zoom), adding or changing a map means **re-baking the whole corpus**. That is your call on cost and
timing, not mine — and it is not reversible in an afternoon.

**Suggested path.** Do not commit to the re-bake up front. Prototype the field on ONE sprite, A/B it
against the checkpoint's 16-tap output, and only then decide. The branch-comb case — a conifer tip, where
several branches sit inside one penumbra width — is where an SDF models the nearest edge and will differ
most from ground truth, so that is the sprite to prototype on.

## B2 — torch reach is set for safety, not for looks · OPEN, not blocking

**What.** `content/visual/things.rd` authors reach 8. That is the measured-good value (120 fps with 3
torches; 16 → 18 fps), and I reverted it there after raising it to 20 cost a day of GPU crashes and hangs
([I6](issues.md#i6)).

**Why it needs you.** 8 was chosen because the renderer can afford it, not because the world looks right
with it. Whether a torch should light a bigger area is an art and gameplay judgement. The renderer work
in this stream may change the affordable ceiling, at which point the question reopens.

**Suggested path.** Leave it at 8 until P3 lands and is profiled, then revisit with a real number for what
a bigger reach costs. To inspect long shadows meanwhile, move the camera rather than raising reach.

## B3 — the Game View panel click loses the WebGL context · OPEN, not blocking, unexplained

**What.** Clicking the **Game View** tab reliably loses the context (`isContextLost()`, `getError()`
`37442`). Reproduced ~6 times across two browser sessions. Loading with Game View already active is fine.

**Why it might need you.** I could not find the cause and I burned a lot of a session on wrong theories
for it ([I6](issues.md#i6)). Both bake paths are budgeted, so "one enormous draw" is not the mechanism.
Not ruled out: render-target churn on the panel switch, VRAM exhaustion, a driver fault. You may simply
know what the panel switch does that I do not.

**Suggested path.** Work around it — load with Game View already active, which is reliable. If it is worth
chasing, the repro is specific enough to bisect: instrument RT create/destroy across the switch and see
whether the panel tears down and rebuilds the viewport's targets.
