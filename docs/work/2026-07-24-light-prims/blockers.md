# Light prims — blockers

_Things needing your input before the build proceeds. Open → resolved (archive resolved with a date)._

## B1 — Ratify the record + def model (P0) — it changes VARIABLES bands (2026-07-24, OPEN)
This is a **write-up, not greenlit to build.** Two forks change authoritative
[`VARIABLES.md`](../../VARIABLES.md) layouts, so they're your call before P1 touches code:

- **[F1](forks.md#f1) — light-record wiring.** Lean **(a) dual-record**: a light is a real prim
  (`prim_data` position + a light-def) and `light_data` is derived for the gather. Alternatives: (b)
  derived-only (keep just `light_data`, a light is not a real prim), (c) full-merge (retire `light_data`,
  rewrite the gather's light read).
- **[F2](forks.md#f2) — where light props live.** Lean **(a) a dedicated light-def band** (shared
  "torch"/"moonlight" types) for the documented many-lights future; **(c) inline** is the low-regret
  minimal start (a strict subset — add the def band later).

**Why it needs you:** VARIABLES owns cross-component layouts; a band change outranks code and shouldn't
be chosen unilaterally. **Suggested path:** confirm F1-(a) + start F2 at (c) inline (grow to a def band
when a second light type exists), or say "build it" with your own picks. F3 (delivery list) and F4
(dirtying) are mine to resolve once F1/F2 are set.
