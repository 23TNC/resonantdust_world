# Forks — hot-shadows

_Decision points + options + which we chose + why. F1–F4 set the layouts / mechanism — resolve at/before P0._

---

## F1 · Where hot-prim associations live (the hot LUT) — 2026-07-21 (open, user)

A work item's `lut_index/lut_count` names a run of prims to shadow. For `prim_temp=cold` it indexes the static
cold LUT texture (`cold_light_prim_data`) — fine. But **hot** prims move every frame, so their light→prim
associations change every frame and **can't sit in a static texture**. Options:

- **(a) A parallel hot-LUT uniform** — a uniform array of `(definition_index, prim_data_index)` (same entry
  shape as the cold LUT); `prim_temp=hot` runs index it, `prim_temp=cold` runs index the cold LUT texture.
  Rebuilt each frame (small — few hot prims). Symmetric with the light/prim uniform split.
- **(b) Work items ARE the granular association** — drop the LUT indirection for hot; each hot work item lists
  its prims inline. Fewer indirections, but a different shape than cold.

**Decided (user, 2026-07-21):** the work item is *(light temp+index, prim_temp, prim_index, prim_count)* — a run
of PRIMS, whose **source follows `prim_temp`**: the **hot prim uniform** (hot) or the **cold LUT**
(`cold_light_prim_data`, since cold light × cold prim are both static). **No separate hot-LUT texture** — the
hot prim uniform is the list, indexed directly by the run. Hot prim **defs** reuse the cold
`prim_definition_data` (geometry is temp-agnostic). (Rejected: writing the cold textures every frame for movers
— too slow; uniforms carry the hot tier.) Work-item bits are unchanged; only the run's source is
`prim_temp`-selected. _Open detail:_ a hot prim uniform entry likely carries its own `definition_index` (no LUT
to pair it) — i.e. `cold_prim_data` shape + `def_index`.

## F2 · The bitfield write mechanism (OR bits) — 2026-07-21 (open; biggest new piece)

Setting a light's `shadow_bit_index` bit in an integer `RGBA32UI` map: GL blend can't bitwise-OR, and
overlapping casters of one light must OR (not add → bit spillover). Options:

- **(a) Ping-pong integer OR** — draw the fans into a coverage buffer, then a pack pass reads old-map + coverage
  and OR's the bit, writing the new map (never sample the bound target). The `shadows` / `bitfield-rt`
  mechanism, on real `uint`.
- **(b) Disjoint-bit additive with per-light union** — render each light's fans into a cleared scratch (union
  via `max`), then add the light's bit into the accumulator; bits disjoint → add == OR. More passes.
- **(c) `logicOp`(GL_OR)** — WebGL2 doesn't expose `glLogicOp` for blending, so out.

**Decided (user, 2026-07-21):** (a) — ping-pong integer OR. Its own sub-phase within P2.

## F3 · `*-hot`/`*-cold` map space + reconcile with `shadows` — 2026-07-21 (open, user)

A world-space **hot** map is wrong: a moved hot prim leaves a stale shadow at its old world position (the
"which-rect-when-a-light-moves" problem). The [`shadows`](../shadows/README.md) design already solved this:
cast the fresh (hot) tier in **screen space** (`shadow-hot`, rebuilt every frame → no staleness), keep the
static (cold) tier **world space** (`shadow-cold`, persistent + toroidal). Options:

- **(a) Adopt the `shadows` screen/world split** — `*-hot` = screen-space (rebuilt), `*-cold` = world-space
  (persistent, toroidal, budgeted). Our temporal hot/cold *is* their spatial screen/world. Reuse their
  4-copy screen→world bridge for anything that graduates cold.
- **(b) Both world-space** — simpler conceptually, but needs per-frame clearing of moved hot shadows (the
  problem the `shadows` stream was created to avoid).

**Decided (user, 2026-07-21): (b), WORLD-space — because shadows are separated per BIT.** The staleness worry
dissolves once each light is its own bit: a cold light × cold prim shadow bit sits in the static `*-cold`
bitfield; anything with a hot participant (a moved light, or a cold light shadowing a moving prim) sets its bit
in the `*-hot` bitfield, which is **fully rebuilt each frame** (so no stale bits — the moved shadow is recast,
not cleared-and-patched). No partial-shadow-split problem because the bits don't blend across rects — they're
independent. The **lit output** (`lightmap-cold`) then **accumulates light per rect via the dirty-rect method**
(the tiered-lighting model), kept static except dirtied rects, with a **hot light map added** on top each frame.

**Consequence — this SUPERSEDES the `shadows` stream's screen→world approach.** `shadows` went screen-space
specifically to dodge world-space rebuild + rect-fighting; the per-bit separation makes world-space viable
instead. So this is NOT "absorb the shadows output" — it's a **different (world-space, per-bit + dirty-rect)
model** that replaces it. → **close/supersede `shadows`** (its screen-hot→world-cold 4-copy is no longer the
plan). The one cost to watch: rebuilding the world-space `*-hot` bitfield each frame over the window (budget +
few hot things keep it bounded).

## F4 · One shared 128-bit map vs separate hot/cold; 128 vs 256 lights — 2026-07-21 (open, user)

`RGBA32UI` = 128 bits = 128 one-bit lights. Options:

- **(a) Two maps** — `*-hot` cleared+rebuilt each frame, `*-cold` persistent (budgeted); OR them at read.
  Simplest clearing (clear the whole hot map each frame).
- **(b) One shared 128-bit map** — cold + hot share the bit space (`shadow_bit_index` partitions them); each
  frame clear only the **hot** lights' bits (selective) and rebuild. Fewer textures/reads; trickier clear.
- **256 lights** — two `RGBA32UI` (or an `RGBA32UI` ×2 MRT); `shadow_bit_index` `u8` already addresses 256.

**Decided (user, 2026-07-21): (a) two separate maps** — F3 settled this: `*-cold` static (cold×cold) + `*-hot`
rebuilt each frame, combined at read (cold | hot). Not one shared map. Start at 128; `shadow_bit_index` `u8`
keeps 256 open (a second `RGBA32UI`) if the light count grows.

## F5 · Uniform sizes + budget granularity — 2026-07-21 (open)

`hot-light-work`, hot lights, hot prims, and the hot LUT are uniform arrays (GLSL max uniform components
~1024–4096 vec4, driver-dependent). Movers are few and the budget bounds the work list, but fix explicit caps
(e.g. `MAX_HOT_LIGHTS`, `MAX_HOT_PRIMS`, `MAX_WORK_ITEMS`, `COLD_BUDGET_PER_FRAME`) and `log()` any drop rather
than silently truncating. **Lean:** modest caps (tens of hot lights/prims, ~hundreds of work items) + a
per-frame cold budget dialed to keep the frame bounded; revisit if a scene exceeds them.
