# Blockers — beat e07

_Things needing the user: what blocks, why it needs **them**, options, my recommendation. A
blocker is theirs; a [fork](forks.md) is mine._

## B1 — Which base model to train on {#b1}
_2026-08-02 · opened at P1.2 · **RESOLVED same day by the user: animagine-xl-4.0**, with Illustrious explicitly kept alive for a future attempt_

> "Both models have weaknesses. I agree we can start with animagine-xl-4.0 but I am not ruling out future attempts with illustrious." — user, 2026-08-02

So this is a STARTING POINT, not an elimination. See [F4](forks.md#f4) and the README's future-intent section.

**What blocks:** [P1.3](todo.md) onward (ControlNet check, VAE wiring, `generate.py` default) all
name "the chosen base". [P2](todo.md) does **not** depend on it, so execution continues there.

**Why it needs them:** [F1](forks.md#f1)/[F3](forks.md#f3) put the user's eyes on the selections,
and this is the first one. It also costs a 2-4 h training run to get wrong.

**My recommendation: animagine-xl-4.0**, which reverses my own pre-measurement pick.

| | east cells wider than tall | mean aspect (east) |
|---|---|---|
| Illustrious-XL v1.0 | 0 / 4 | 1.02 |
| **animagine-xl-4.0** | **4 / 4** | **1.89** |

East aspect is the body-vs-bust discriminator; the sheet agrees with the number. Sheet at
`.staging/p1-base-probe.png`.

**Caveat:** 8 cells, one seed, one loaded prompt, and an untrained prior only correlates with what
a base will learn. Enough to choose where to spend a run; not proof.

**If the user picks Illustrious anyway** that is fine and cheap to honour — only
`eval_set.json:model` and `generate.py:MODEL` change.

Expected to appear here: the per-epoch **selection** calls in [P2](todo.md) are the user's by
design, not blockers — they are the point of the stream. A real blocker would be something like
"ComfyUI must autostart, which changes how the box boots" or "this run costs N hours, proceed?".
