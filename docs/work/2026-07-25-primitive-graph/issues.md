# Primitive graph — issues

_Problems found reviewing the design + how they resolved. Chronological append.
**I1–I7, I10, I11 RESOLVED** by the user's 2026-07-25 revisions (parent ids + CPU-stamped resolved
position, presence split, rotation semantics, fixed 8-px commands, `id = 0` sentinel, `radius` restored).
**Open: I8, I9, I12, I13 ⚠, I14.**_

## I1 — A carried record cannot find its carrier ✅ RESOLVED (2026-07-25)
**Was.** Leaves held offsets only, links ran downward only, so a light id from presence could not be
located and the inherited flags (ancestor properties) were unreachable.
**Resolved.** `u16 parent_id` added to `light_data` + `billboard_data` (RED), and `prim_data` gained a
`child` bit that reinterprets RED's top half as `u16 parent_id`. The graph is now navigable **upward**,
so either side can resolve — the CPU normally does (it must anyway, to build presence), and the GPU
*can* if a shader needs to. See [I11](#i11) for where the walk should actually happen.

## I2 — Child offsets unsigned ✅ RESOLVED (2026-07-25)
**Resolved: bias-8** per nibble (−8..+7 tiles / units) — user agreed. Applies to every offset field
(`tile_offset`, `unit_offset`) on child prims, billboards, and lights. To be written into VARIABLES at P0.

## I3 — Hot-loop fan-out through carriers ✅ RESOLVED (2026-07-25)
**Resolved** by bucketing **leaves, not carriers**: `light_presence` holds `light_data` ids and
**`billboard_presence`** (renamed from the caster buckets) holds `billboard_data` ids. The user's
reasoning is the decisive one: bucketing carriers makes the per-tile billboard count **unbounded**
(a carrier fans out to ≤4, recursively). Leaf bucketing keeps the corridor walk exactly as cheap as
today, at 8 slots/tile.

## I4 — `rotation`/`layer` precedence ✅ RESOLVED (2026-07-25)
**Resolved, and the question was mis-framed.** There is no render-time rotation precedence: **the shader
always uses the definition's rotation**, because the definition *is* the active sprite. Stored `rotation`
is the **desired facing** — a CPU-side reconciliation signal that triggers a *definition swap* when it
disagrees with the active def. **Revised placement (2026-07-25):** `inherit_rotation` lives on
`definition_data` (RED, 1 bit) — all objects of a kind behave alike, so the choice is per-*kind*, not
per-object — plus one bit on `prim_data` (GREEN) for a prim that should act independently. Inheritance is
**one step** (child takes its parent's rotation), not a chain walk; the leaves carry no inherit bit and
the GPU simply uses the **bottom-most** rotation, already reconciled. `layer` needs no precedence either —
**one object per layer** means a prim's pieces occupy distinct layers, and `prim_data` carries no layer.

## I5 — `u3` counts cannot express 8 groups ✅ DISSOLVED (2026-07-25)
The counts scheme is gone. **Fixed 8-px commands** (opcode + set + 7 ids + 7 payloads) give 56
record-writes per 64-px row with no count field, no width problem, and no group padding.

## I6 — The opcode loses its home ✅ DISSOLVED (2026-07-25)
The opcode is now **byte 0 of every command** and *defines* the rest of the command's layout —
a stronger extension point than the old reserved field. `0x01` = write-data; future 8-px operations
(presence writes, bulk clears) take their own opcodes.

## I7 — 8-record write amplification ✅ RESOLVED — `id = 0` SENTINEL (2026-07-25)
A command writes ≤7 records to **one set**, and a partial command **pads its unused id slots with 0**
(user). `id = 0` is a global sentinel, so the scatter discards those points — emit a degenerate
`gl_Position` (outside clip space) so the point is clipped and no write lands. This keeps the trivial
index map (record `p` → command `p/7`, slot `p%7`) with no per-point bookkeeping. Cost: **one burned
entry per set** ([I14](#i14)).

## I8 — Rename reconciliation (2026-07-24 → 2026-07-25) — OPEN, mechanical
Yesterday's `prim_*`→`billboard_*` rename folded `prim_data` into `billboard_data`. This design
**splits** them again. Land as: `prim_data` (node) / `billboard_data` (leaf, NEW) / `light_data` (leaf) /
`definition_data` (revert from `billboard_definition_data` — it now carries a `u4 type` and serves more
than billboards). The 2026-07-24 *vocabulary* still holds (primitive = umbrella, billboard = a
presentation); only the band split changed. Also rename the caster buckets → `billboard_presence`.

## I9 — Subtree lifetime: freeing a carrier must free what it carries — OPEN
Freeing a pawn must free its hands, their billboards, and the torch's light — a **recursive** free — and
a carried record must not be reclaimed while its carrier lives. `parent_id` now makes the child→parent
direction checkable, which helps. **Solution:** free by **reachability from placed roots**
(mark-from-roots, then sweep), replacing the flat "seen this frame" sweep in `freeBillboardsExcept`.
Also: `z` (u8) + `z_offset` (u8) can sum past u8 — clamp and document.

## I10 — `light_data` lost `emitter_radius` ✅ RESOLVED — RESTORED (2026-07-25)
**Problem.** The first spec had `BLUE: u12 reach | u8 radius | u12 reserved`; the revision has
`ALPHA: u12 reach | u20 reserved` — **`radius` is gone**. It is load-bearing: `emitter_radius` drives the
entire **16-tap area-light penumbra** (the delivered [penumbra](../2026-07-23-penumbra/README.md)
stream) — `casterCover(...)` takes `emitter` and falls back to a **hard quad** when it's < 0.5
([`shadowGather.ts:225,257`](../../../client/webgl/src/game/viewport/shadowGather.ts)). Dropping it
silently turns every soft shadow hard.
**Resolved.** User confirmed the oversight; `u8 radius` is back. Final:
`ALPHA: u12 reach | u8 radius | u8 resolved_zone | u4 reserved`.

## I11 — Where does the parent-chain walk happen? ✅ RESOLVED — CPU STAMPS IT (2026-07-25)
**Problem.** `parent_id` means the GPU *can* resolve a leaf's absolute position, but the gather's light
loop runs **per texel per light** — doing a chain walk there multiplies the hottest loop in the renderer
(the same cost that makes a fine-res gather unaffordable). The CPU, by contrast, **must** compute each
light's world position anyway to decide which tiles it reaches when building `light_presence`.
**Resolved (user).** The **CPU stamps the resolved position** and the leaf spends its whole RED lane on
it: `u16 parent_id | u8 resolved_tile | u8 resolved_unit`. Resolution starts at the root's tile/unit and
applies each child prim's offsets down the chain to the leaf. The authored `tile_offset`/`unit_offset`
stay in GREEN as the durable relative placement; RED is the derived cache the GPU reads. **The GPU never
walks the graph in the hot loop.** (Follow-on: `resolved_zone` is also required — [I13](#i13).)

## I12 — Two resolvers risk drift (2026-07-25) — OPEN, low-stakes
With both CPU and GPU able to resolve the graph, they must agree **exactly** (the class of bug that bit
us twice on zoom). **Solution:** name **one** authority per consumer — CPU-resolved for presence,
bucketing, and the baked position; GPU walk only for reads with no CPU-side equivalent — and state it in
VARIABLES so nobody adds a second path later. Also cap and document `MAX_DEPTH` for any GPU walk.

## I13 — ⚠ `resolved_tile` + `resolved_unit` alone are ambiguous beyond 8 tiles of reach (2026-07-25) — OPEN
**Problem.** The leaf's resolved position is `u8 resolved_tile | u8 resolved_unit` — tile is `x:4|y:4`,
i.e. the **in-zone** tile (0–15). The gather needs the light's position in the same frame as the sample
point: it computes `toL = Lxy - P` ([`shadowGather.ts:573`](../../../client/webgl/src/game/viewport/shadowGather.ts))
for falloff/N·L, and marches the corridor from the light. A fragment knows its own absolute world tile,
so it can reconstruct the light by picking the **congruent tile nearest itself** — but that has a period
of **16 tiles**, so it is only unambiguous when the light is within **8 tiles**. `LIGHT_REACH` is
**12 tiles** today, and the `u12 reach` field permits up to 255 — so a light 12 tiles north reconstructs
as 4 tiles south. This is precisely the position-ambiguity class that caused the two zoom regressions.
**Solution.** Also stamp **`u8 resolved_zone`** (in-region zone, `x:4|y:4`). Zone+tile+unit gives a
period of **256 tiles** — unambiguous for any reach under 128 tiles, comfortably covering the field's
range; region stays inferred by nearest-congruent (exactly what the region-torus fold already does).
Room exists with no layout pressure: `light_data` ALPHA (`u12 reach | u8 radius | u8 resolved_zone |
u4 reserved`) and `billboard_data` ALPHA (`u8 resolved_zone | u24 reserved`). **Applied in the README;
needs your confirmation.**

## I14 — `id = 0` cannot be a global sentinel: tile-keyed sets address by fold ✅ RESOLVED (2026-07-25)
**Problem.** The first plan made `id = 0` a global sentinel so partial commands could pad with zeros
("burns one entry per set"). That is safe for **record** sets (defs/prims/billboards/lights — allocators
just start at 1) but **not** for the three **tile-keyed** sets (`light_presence_lo/_hi`,
`billboard_presence`), whose in-set id is not an allocated counter but a **computed fold**:
`foldTile(0,0) = 0`, and the fold spans **0..65535 exhaustively** over a region (verified), so there is
**no spare id to bias into** (`fold + 1` overflows u16). Since a command carries **one** `set` shared by
all 7 slots, a partial command targeting a tile-keyed set would pad with `id = 0`; discarding `id == 0`
would make that tile **permanently unwritable** — exactly one tile per region-torus silently holding no
lights and casting no shadows.
**Resolved (user).** Steal bits from the header instead: **`R = u8 operation | u5 set | u3 count |
u16 id₀`**. `count` states how many of the 7 ids are live, so **no sentinel is needed anywhere** and
`id = 0` stays fully usable. `set` narrows to **u5** (32 sets; 16 in use) — widen out of `operation`
later if needed. The scatter issues 7 points per command and drops `slot ≥ count` (off-clip position),
so the trivial `p/7`, `p%7` index map survives. Nothing is burned and no allocator needs to change.

## I15 — Vite HMR gives a FALSE failure when the command format changes (2026-07-25) — verification gotcha
**Problem.** After swapping the transport to v3, the scene rendered as flat blocks with no prims — but
there were **no console errors**, and the logic was correct on inspection. Cause: **HMR had replaced the
module while the live `ColdShadowData` instance (and its already-compiled scatter `Program`) survived**,
so a NEW `flush()` was feeding an OLD shader — exactly a format mismatch. A full navigate to a
cache-busted URL rendered correctly and the data confirmed flowing (525 billboards, 3 lights, last flush
→ set 2).
**Rule for this stream:** any command-format or record-layout change must be verified on a **fresh page
load**, never on an HMR update. A silent flat/garbled frame with a clean console is the signature.

## I16 — ⚠ 16 lights/tile does NOT fit `shadow-cold` alongside the on-billboard flags (2026-07-25) — OPEN
**Problem.** Freeing the self-address takes presence from 14 → **16 lights/tile**, but the gather's
OUTPUT RT (`shadow-cold`, one `RGBA32UI` per texel) currently packs **per-slot u8 coverage + a per-slot
on-billboard flag**: today 14 slots = 14·8 = 112 coverage bits + 14 flag bits = **126 ≤ 128** — it just
fits. At 16 slots the coverages alone are 16·8 = **128 bits**, consuming the whole texel, and the 16 flag
bits have **nowhere to go** (144 > 128).
**Candidate solutions.** (1) **Move the 16 on-billboard flags into the second attachment** (`oCasterD`,
which today carries only a 7-bit caster row and is nearly empty) — cheap, no coverage loss, and the
attachment is already bound. (2) Drop coverage to **u7** (16·7 = 112 + 16 flags = 128 exactly) — costs
half the penumbra resolution the [penumbra](../2026-07-23-penumbra/README.md) stream deliberately bought
(u8→u9 then u8). (3) Cap the gather at 14 slots even though presence carries 16 — wastes the gain.
**RESOLVED (user, 2026-07-25): option (2) — u7 coverage | u1 on-billboard.** 16 slots × 8 bits = 128
exactly, so the flag rides its own slot's byte and the separate A-lane flag field is retired. The u9→u8
coverage was never fully used; u7 (128 levels) is restored to full range by a ×2 on read — and that is
free, because the stored `coverage << 1` **is** the doubled value (`byte & 0xFE`). Implemented as
`float(b8 >> 1) / 127.0`.

## I17 — Parallel slot-stride literals desynced (7 vs 8) (2026-07-25) — fixed, lesson recorded
**Problem.** Widening the caster buckets 7 → 8 slots, the *write* was updated
(`castSlots.subarray(ti * 8, …)`, allocation `cols * rows * 8`) but the *fill* still used the old
literal (`castSlots[ti * 7 + n]`). The two strides disagreed, so buckets were written from misaligned
memory. Symptom on a fresh load: **trees rendered flat (no relief) and shadows mostly vanished** — both
follow from `receiverAt` failing to find the covering billboard (no `rbillboard` ⇒ `applyNL` false ⇒
`ndl = 1`, and no caster found ⇒ no shadow). One symptom pair, one root cause, easy to misread as two
separate bugs.
**Fix.** Aligned the fill stride. **Lesson:** the presence path already does this right — it derives
every index from a single `PRES_SLOTS` constant, so widening it was a one-line change. The caster path
used bare literals. Give the bucket stride its own named constant when P2's record work lands, so the
next width change cannot desync.

## I18 — Relocating a record's fields: grep the whole LANE, not the block you're editing (2026-07-25)
**Problem.** Moving `light_data`'s fields to v3, I updated the decode block I was looking at in each
shader and missed **two** reads elsewhere in `LIGHT_FRAG`: `Lz` (still `(Ld.w >> 24)`, the old z slot —
now reach's high bits) and the **edge-refine's** `emitter` (still `(Ld.w >> 4)` — now `resolved_zone`).
Both compiled fine and produced a *plausible* picture, so `tsc` and a glance both passed; the tell was
**hard-edged shadows** (a garbage `emitter` changes the penumbra) plus a wrong light height.
**Rule.** When a record's layout moves, `grep` every read of that record's lanes (`Ld.x/.y/.z/.w`,
`D.x/…`, `Pd.x/…`) across all shader stages and check each one — the same record is decoded in several
places (gather, lightmap, the fused edge-refine, the overlay), and a stale read is silent.

## I19 — The GLSL backtick foot-gun, again (2026-07-25)
Writing the `resolvedTilePos` doc comment I used **backticks around an identifier inside GLSL** — which
lives in a JS `/* glsl */` template literal, so the literal closed mid-shader and the build broke with a
bare `TS1005 ',' expected`. This is a **known recurring** mistake with its own standing note; the error
message points at TypeScript syntax and says nothing about shaders, which is what makes it cost time.
**Rule:** never use a backtick inside GLSL — not in code, not in a comment. Prefer plain identifiers.

## I20 — Bash-heredoc edits bypass the repo's own PostToolUse hooks (2026-07-25)
**Problem.** `.claude/settings.json` registers `bin/hooks/glsl-backtick-check.mjs` on **`Write|Edit`**.
Every source edit this session went through `python3 - <<'PYEOF'` on the **Bash** tool, which matches
neither and carries no `tool_input.file_path` — so the guard never ran, and the backtick of
[I19](#i19) surfaced instead as a bare `TS1005 ',' expected` that never mentions shaders. Confirmed by
replaying the exact case into the hook afterwards: it **blocks**, naming the line and the fix.
**Rule.** Edit source files with **Edit/Write** so the guards fire (`Edit` is already all-or-nothing per
call, so the atomicity argument for scripting is weak). If a bulk mechanical rewrite genuinely warrants
a script, run the guard by hand after:
`echo '{"tool_input":{"file_path":"<file>"}}' | node bin/hooks/glsl-backtick-check.mjs`.
Generalises to any guard wired to the file-editing tools — a shell edit is invisible to all of them.

## I21 — A stale `OPEN` blocker row silently disarms the continuation hook (2026-07-25)
**Problem.** The Stop hook's stage 2 (`work_check.py --nudge`) blocks a *silent premature pause* — but
releases when the stream has an open blocker:
`if not items or _has_open_blocker(stream) or _has_stop_reason(stream): return 0`.
[B3](blockers.md) (`resolved_zone`) sat marked **OPEN** long after the user had answered it — the
reach-vs-containment reasoning settled it, the field landed in VARIABLES at P0, and P2c had been
*running on it* for several phases. So every pause in this session looked legitimately blocked and the
hook stood down. Proven by experiment: with B3 still open the nudge exits **0** (silent); the moment the
row is closed it exits **2** and names the next eight items.
**Why it matters beyond this row.** The guard is only as honest as the bookkeeping it reads. A settled
blocker left open is not a cosmetic lag — it **switches off** the mechanism that keeps a session
executing the plan, and it does so invisibly (exit 0, no output).
**Rule (this is CONVENTIONS' "close it in the same commit" made concrete).** The moment a blocker is
answered, resolve the row **in that turn** — before continuing the work it unblocked. Same for
`.stop-reason` markers: delete on resume. Cheap self-check when a pause feels justified:
`python3 bin/lib/work_check.py --nudge <<< '{"session_id":"<sid>"}'` — a silent exit 0 while real work
remains means an escape is stale.

## I22 — The continuation hook had TWO independent releases, not one (2026-07-25)
Correcting [I21](#i21), which found only half the cause. The nudge stayed silent all session because
**either** of two conditions alone releases it:
1. **The work-index row said `blocked`.** The detector selected the active stream by index status
   `open` — this row was indexed `blocked` from the write-up, and never updated once the blockers were
   answered. This is the same defect the parallel
   [`2026-07-25-continuation-hooks`](../2026-07-25-continuation-hooks/README.md) stream identifies as
   having kept the hook **silently dead for six days**; its P6 fixes the selector.
2. **A stale `OPEN` blocker row** ([B3](blockers.md)) — [I21](#i21).
I found (2) and stopped looking, which is the same shape of error as the bug itself: one plausible
cause accepted before the search was exhausted. Both are now fixed — the index row reads `open` and B3
is closed.
**Rule.** A stream's index status is machine-read, not decoration: move `blocked` → `open` in the turn
the last blocker is answered, exactly as with the blocker row itself.

## I23 — PLAN ORDERING: two P3 items depend on P5 authoring (2026-07-25)
**Problem.** The remaining P3 items cannot be built before P5:
1. **Rotation reconciliation** — the CPU is meant to swap a piece's definition when its desired
   `rotation` disagrees with the active def's. But a facing is chosen by **`cell`**
   ([`WorldBridge.ts:99`](../../../client/webgl/src/game/world/WorldBridge.ts) — "cell selects which grid
   cell of the master atlas to bake: for a facing kind that's the packed facing"). Reconciling requires
   the def to record which rotation its cell represents *and* the CPU to resolve a def for a different
   rotation — i.e. the facing→cell mapping, which is authoring's to supply.
2. **Deleting `this.lights`** — lights exist only in `seed()` / `EXTRA_LIGHT_TILES` / `__manylights`.
   Deleting the array before a placement source exists means **no lights at all**; "drive lights from
   placed prims" presupposes something placing them.
**Resolution.** Not a wrong plan, a wrong order. Both items move to **P5**, after the authoring path
lands. P4 (dirty generalisation) has no such dependency and is pulled forward — it operates on records
that already exist, whoever placed them.
**Lesson for the plan shape.** Both items *read* as executable ("delete X", "swap Y") while silently
depending on a later phase. An item is only executable if its inputs exist; phase order should be
checked against that, not against narrative flow.

## I24 — Perf regressions introduced by the graph work (2026-07-25) — three fixed, headline NOT reproduced
User reports the build at **30 fps**, previously 120 locked. Profiling found three genuine regressions
I introduced, all now fixed:
1. **`casterOne` fetched the same texel TWICE** (`fetchLin(...).x` written out twice instead of held in
   a local) — inside the **corridor walk**, i.e. per caster per light per texel, the hottest loop in the
   renderer. Now one fetch.
2. **The graph work ran ~971×/frame to conclude nothing changed.** `buildCasters` calls
   `billboardDataFor` for every standing prim every frame; P2e made it always `encodePosition` +
   `resolveCarried` (walk + decode + encode) + two 4-way compare-writes, where it used to early-out on
   an unchanged orient word. Measured **0.66 ms/frame** in `billboardDataFor` + 0.20 in `resolveCarried`.
   Added a three-read fast path (position / rotation / definition): resolve calls **971 → 133 per frame**.
3. **~1700 short-lived arrays per frame** — `lastBox.set(id, [x,y,w,h])` allocated a fresh array per prim
   per frame. Now mutated in place.

**But the headline is NOT reproduced.** Measured after the fixes, fresh loads, steady state:
| scene | billboards | window | fps |
|---|---|---|---|
| zoom 2, focus 34,25 | 525 | 240 tiles | 120.8 |
| zoom 1, focus 100,50 | 455 | 720 tiles | 120.5 |
| zoom 0.25, focus 100,50 | 3144 | 7440 tiles | 120.2 |
| zoom 0.25 + **16 dynamic lights** | 1709 | 7440 tiles | 119.7 |

Hypotheses tested and **eliminated**: light count (16 dynamic ⇒ 119.7); billboard count (3144 ⇒ 120.2);
window size; HMR loop accumulation — `Ticker` has an undisposed `requestAnimationFrame` and there are no
`import.meta.hot` handlers anywhere, but Vite therefore does a **full page reload** on update (proved: a
`window` global set before the update was gone after it), so nothing accumulates.
**Still unknown** — what differs in the reporting session. Needs: the view (zoom/focus), whether it is
steady state or during zone streaming, and whether other lights were injected.

## I25 — Three blockers to content-lit worlds, found by actually lighting one (2026-07-25)
Wiring the first carried light surfaced three separate reasons a content-lit world could not have
worked — each invisible until the debug array was emptied:
1. **`tick` bailed on `this.lights.length === 0`.** Content lights are discovered *by* `buildCasters`,
   which runs inside `tick` — so an empty debug array stopped the tick, which stopped the discovery,
   permanently. This is the concrete thing behind "delete `this.lights`": the deletion could never have
   worked without fixing the guard first. Now bails only when there is genuinely nothing (no debug
   lights, no carried lights, no standing prims). Same for the `coldData.lights === 0` guard below it.
2. **`buildPresence` ran BEFORE `buildCasters`.** Presence culled against a light set that did not yet
   include the carried ones, and its signature gate then stopped it ever retrying — a torch stayed
   invisible forever. Fixed by ordering casters → presence, folding a `carriedVer` into the signature,
   and moving the single `flush()` to after both so they still land in ONE scatter batch.
3. **A light-carrying prim that is not a caster got no carrier.** The attach sat after
   `if (def < 0) continue`, so a bare light source — or a sprite with no resolved silhouette — was
   skipped before `billboardDataFor` could allocate its carrier. `carriedLightFor` now ensures its own
   carrier, and the attach moved above the caster gate.

**Verified**: `__torch()` turns a placed billboard into a torch; with `this.lights` emptied, 131 of 240
in-window tiles list the carried light and the scene renders lit by it, shadows cast away from it.
Carrier reads `set_b = 2`, `id_b = 128` (the carried-light id space starts at `N_LIGHTS`).
