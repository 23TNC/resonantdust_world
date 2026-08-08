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

## D2 — I edited `generate.py` while a labelled run was in flight, and discarded 22 sprites
_2026-08-08 · caught mid-P2.1 · **process error, no lasting damage**_

The `east-anchor` label was generating when I started wiring P2.2's IP-Adapter knobs into the same
file. `consistency_eval` invokes `generate.py` as a **subprocess per direction**, so every cell
after the edit would have used a different graph from the cells before it — a label that is half
one pipeline and half another, reported as one number.

**Killed the run and deleted its 22 sprites** rather than keep a set whose provenance I could not
state. The cost is ~15 minutes of GPU time; the alternative was a number nobody could trust, which
is the more expensive of the two.

**Then verified the change was behaviour-preserving before restarting**, rather than assuming it.
Swapping `IPAdapter` → `IPAdapterAdvanced` (needed because the composition-carrying weight types
live only on the advanced node) with `weight_type="style transfer"`, `combine_embeds="concat"`,
`embeds_scaling="V only"` reproduces the old node **bit-identically** — same md5 on wolf/9101 south
against the stored baseline sprite, with the anchor, seed, prompt and knobs held fixed. So
`east-anchor` remains a single-variable comparison against `baseline`.

The first attempt at that check was itself invalid — I ran it in an empty leaf, so there was no east
sprite, so it took `graph_hero` and never touched the adapter at all. The hashes differed for a
reason that had nothing to do with the node. Staged the baseline east into the leaf and re-ran.

**The rule this earns:** a labelled run owns the code it started with. Edit the harness or the
generator only between labels, or branch the file. The pipeline being deterministic ([I8](issues.md#i8))
is what makes both the contamination *and* the equivalence check exact.
