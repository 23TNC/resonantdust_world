# Deviations — direction consistency

_Where execution departed from the plan, logged AT THE MOMENT of deviating. "Less churn" is never
a reason._

## D1 — I scoped the plan to installed tooling and dropped the design doc's own first choice
_2026-08-08 · raised by the user during P0 · **plan defect, corrected by [F5](forks.md#f5) + P6**_

**The user's question:** *"I noticed that when you drew up this plan you focused on what we had
downloaded and available. Are there better options available to us on the 3090 that we haven't
explored as we haven't downloaded the tools?"*

Yes. And the authoritative design doc already said so.
[`sprite-gen-plan.md`](../../components/dev/scripts/art/plan/sprite-gen-plan.md) § *Cracking S/N*
lists three candidate mechanisms and states a preference:

> 1. **Edit-model rotation** — feed the East hero to **Qwen-Image-Edit** (half-staged on the box;
>    needs the transformer) … Purpose-built for view-change with identity preservation. Best
>    theoretical fit …
> 2. **Organic pose-reference ControlNet + IP-Adapter identity** …
>
> **Lean:** (1) if the install pays off, (2) as the robust production default.

My [`README.md`](README.md) and [`todo.md`](todo.md) built **only** mechanism (2) — the IP-Adapter
refinement — and never mentioned (1). The "what the 3090 makes available" table was assembled by
listing what `ls` found on the box, which is a survey of *inventory*, not of *options*.

**Verified the doc's claim rather than repeating it:** `models/vae/qwen_image_vae.safetensors` and
`models/text_encoders/qwen_3_4b.safetensors` are present; there is no Qwen-Image transformer
anywhere under `models/`. "Half-staged, needs the transformer" is exactly right, and has been
sitting one download away.

**Why it matters beyond one missing option.** `docs/README.md` puts `design/` above the plan, and
the repo's standing rule is to build *toward* documented intent rather than trim it to what the
current task needs. Scoping to installed weights inverted that: it let the box's current contents
decide the architecture. The same reasoning would have kept us on `cyberrealisticXL` forever,
because it was the only SDXL checkpoint on the box in March — a mistake this project has already
made once and recorded ([beat-e07 F4](../2026-08-02-lora-beat-e07/forks.md#f4), "it is the
incumbent only because it was the only SDXL checkpoint on the box").

**Correction:** [F5](forks.md#f5) records the full option survey with licences, and P6 carries the
work. P0–P4 are **not** abandoned — the ruler is architecture-independent and the SDXL path stays
the production default per the design doc's own mechanism (2).
