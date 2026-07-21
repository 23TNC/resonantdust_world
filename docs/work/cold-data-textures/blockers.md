# Blockers — cold-data-textures

_Things needing human input before they can proceed. Open → resolved (resolved rows archive with a date)._

---

## B-1 · P4 is blocked on where the fan depth (dA/dB) lives — 2026-07-21 (RESOLVED 2026-07-21)

**Resolved:** (a) — `dA/dB` go in `prim_definition_data`'s B channel as two `u8`s in units (`u10 frame_page |
u8 dA | u8 dB | u6 reserved`), computed from the presence bake. P4 unblocked. Original analysis below.


**What blocks:** P4 has the shader build the shadow fan from the four data textures. The fan needs the
per-caster **base-spread depth** `dA/dB` (the 5-triangle ±spread `shadow-projection` derives from each sprite's
silhouette), but **the cold-data layout has no field for it** — `prim_definition_data` holds geometry + frame,
`cold_prim_data` holds position + rotation, neither has depth. So the shader can't reproduce the current
`shadow-projection` fan without it.

**Why it needs a human:** the fix extends the **authoritative packed layout** in `docs/VARIABLES.md` §Cold
shadow data textures — the user authors these bit layouts (this whole stream has been an iterative
design/contract pass with them). I won't unilaterally spend layout bits.

**Solution analysis (see [F6](forks.md#f6) for detail):**
- **(a)** Store `dA/dB` in `prim_definition_data`'s spare `A` channel (`u10 dA | u10 dB | u12 reserved`, units)
  — per-def generic geometry, beside `prim_width/height`; the presence bake (`depthFor`) already computes it.
- **(b)** Derive in the shader from `prim_width/height` (a heuristic; no layout change, loses the accurate spread).
- **(c)** Drop the ±spread in the cold-data version (body triangle only — a visible regression).

**Suggested path:** (a). It's a small addition to a currently-reserved channel and the data already exists.

**Status:** P1–P3 (all three populated data textures + the position codec) are done + verified; only P4's fan
build waits on this. Resolve → wire P4 (the shader `texelFetch`ing all four textures, replacing the instance
attributes), then P5 (rotation → regime) + P6 (verify identical, no per-frame cold upload).
