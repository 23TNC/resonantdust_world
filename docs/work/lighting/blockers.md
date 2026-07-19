# Blockers — lighting

_Dependencies gating a `todo` item. Each: what's blocked, why, the plan to clear. Move the item back to
`todo` once cleared (dated resolution line here)._

---

## B1 · Outline serve + consume — RESOLVED 2026-07-18

Edge serves `GET /textures/meta/{stem}` (`serve_meta`) + client `OutlineCache` decodes the earcut
outline; P3 scatter + P4 casters unblocked. Shipped — see [`completed.md`](completed.md).

## B2 · No static-light content source (soft — blocks the cold tier's payoff, not the code) — 2026-07-18

**Partially blocked.** The cold tier exists to amortize **many static lights**, but content authors none
yet (the DSL has no point-light primitive).

**Why.** "Dense many-lights" needs authored cold lights (torches, glows) classified to the cold tier at
worldgen/load. Without them, the `cold_lightmap` bake is real but has nothing to bake.

**Plan to clear.** Not a hard blocker for the *plumbing* — P1's bake + textures can be built and verified
against a **hardcoded / debug** cold light. The DSL point-light primitive + worldgen classification is
**P5**; do it once the pipeline is proven so we're authoring into a working system.

