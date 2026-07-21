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

**Lean:** (a) — keep the hot path a mirror of the cold path (same LUT entry shape, just uniform vs texture),
one decode. Hot prim **definitions** reuse the cold `prim_definition_data` (geometry is temp-agnostic) — no hot
def texture.

## F2 · The bitfield write mechanism (OR bits) — 2026-07-21 (open; biggest new piece)

Setting a light's `shadow_bit_index` bit in an integer `RGBA32UI` map: GL blend can't bitwise-OR, and
overlapping casters of one light must OR (not add → bit spillover). Options:

- **(a) Ping-pong integer OR** — draw the fans into a coverage buffer, then a pack pass reads old-map + coverage
  and OR's the bit, writing the new map (never sample the bound target). The `shadows` / `bitfield-rt`
  mechanism, on real `uint`.
- **(b) Disjoint-bit additive with per-light union** — render each light's fans into a cleared scratch (union
  via `max`), then add the light's bit into the accumulator; bits disjoint → add == OR. More passes.
- **(c) `logicOp`(GL_OR)** — WebGL2 doesn't expose `glLogicOp` for blending, so out.

**Lean:** (a). It's the design's stated mechanism and lands the integer-RT bitfield the migration promised
(retire float-mod). Non-trivial — its own sub-phase within P2.

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

**Lean:** (a). Don't build a second, conflicting tiering — this stream is largely the `shadows` output with a
cleaner input (work items + hot uniforms). Reconcile explicitly; likely **this absorbs the shadows output half**.

## F4 · One shared 128-bit map vs separate hot/cold; 128 vs 256 lights — 2026-07-21 (open, user)

`RGBA32UI` = 128 bits = 128 one-bit lights. Options:

- **(a) Two maps** — `*-hot` cleared+rebuilt each frame, `*-cold` persistent (budgeted); OR them at read.
  Simplest clearing (clear the whole hot map each frame).
- **(b) One shared 128-bit map** — cold + hot share the bit space (`shadow_bit_index` partitions them); each
  frame clear only the **hot** lights' bits (selective) and rebuild. Fewer textures/reads; trickier clear.
- **256 lights** — two `RGBA32UI` (or an `RGBA32UI` ×2 MRT); `shadow_bit_index` `u8` already addresses 256.

**Lean:** start **(a)** two maps at **128** (simplest correct); fold to one shared map / 256 if the read cost
or the light budget warrants. `shadow_bit_index` `u8` leaves headroom for 256 either way.

## F5 · Uniform sizes + budget granularity — 2026-07-21 (open)

`hot-light-work`, hot lights, hot prims, and the hot LUT are uniform arrays (GLSL max uniform components
~1024–4096 vec4, driver-dependent). Movers are few and the budget bounds the work list, but fix explicit caps
(e.g. `MAX_HOT_LIGHTS`, `MAX_HOT_PRIMS`, `MAX_WORK_ITEMS`, `COLD_BUDGET_PER_FRAME`) and `log()` any drop rather
than silently truncating. **Lean:** modest caps (tens of hot lights/prims, ~hundreds of work items) + a
per-frame cold budget dialed to keep the frame bounded; revisit if a scene exceeds them.
