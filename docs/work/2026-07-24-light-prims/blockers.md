# Light prims — blockers

_Things needing your input before the build proceeds. Open → resolved (archive resolved with a date)._

## B1 — Ratify the light-def band decision (P0) — it changes VARIABLES bands (2026-07-24, OPEN)
This is a **write-up, not greenlit to build.** The 2026-07-24 vocab ratification (a *primitive* presents
as a *billboard* or a *light*; per-presentation bands) **resolved F1**: `light_data` is the light
presentation's placed record (position + props), symmetric to `billboard_data` — no separate
`billboard_data` record for a pure light. The one layout call left is yours (VARIABLES owns bands):

- **[F2](forks.md#f2) — where light props live.** Lean **(a) a `light_definition_data` band parallel to
  `billboard_definition_data`** (shared "torch"/"moonlight" types) for the documented many-lights future;
  **(c) inline** on `light_data` is the low-regret minimal start (a strict subset — grow to the def band
  when a second light type exists). The vocab rules out (b) reusing the billboard def band.

**Why it needs you:** a new band (or the def-index bit-layout on `light_data`) outranks code and shouldn't
be chosen unilaterally. **Suggested path:** start **F2-(c) inline** now, add the `light_definition_data`
band when a second light type appears — or say "build it" with your pick. F3 (delivery list) and F4
(dirtying — the two `markDirty` entry points + cold/hot generalisation you specified) are mine to resolve.
