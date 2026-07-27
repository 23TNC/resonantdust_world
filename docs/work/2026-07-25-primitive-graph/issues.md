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
2. **A carried light never went through the dirty front door.** My first fix — reordering casters →
   presence and adding a `carriedVer` to the presence signature — treated the symptom (user caught it):
   the ordering only mattered because nothing invalidated presence, and `carriedVer` was a *parallel
   bookkeeping channel* duplicating `lightsVer` — exactly the hand-rolled flag-flipping P4 had just
   deleted. Worse, the torch only appeared at all because the test called `rebakeAll()`; the scoped
   path carried nothing. **Real fix:** a new/moved carried light calls **`markLightDirty`**, the same
   door a debug light uses — which queues its scoped cast region *and* bumps `lightsVer`/`coldDirty`,
   so presence rebuilds on its own. `carriedVer` deleted and the ordering reverted; neither was needed.
   **Lesson:** when a change needs a new version counter to be seen, the change is bypassing the
   invalidation path, not lacking one.
3. **A light-carrying prim that is not a caster got no carrier.** The attach sat after
   `if (def < 0) continue`, so a bare light source — or a sprite with no resolved silhouette — was
   skipped before `billboardDataFor` could allocate its carrier. `carriedLightFor` now ensures its own
   carrier, and the attach moved above the caster gate.

**Verified**: `__torch()` turns a placed billboard into a torch; with `this.lights` emptied, 131 of 240
in-window tiles list the carried light and the scene renders lit by it, shadows cast away from it.
Carrier reads `set_b = 2`, `id_b = 128` (the carried-light id space starts at `N_LIGHTS`).

## I26 — Hardening audit of the graph implementation (2026-07-25)
Reviewed everything P2–P5 landed, looking for silent failure rather than obvious breakage. Five real
weaknesses; the first two are demonstrated bugs, not theoretical.

**H1 — a carried light is never released (DEMONSTRATED).** Nothing removes one when its owner stops
presenting a light — turned off, evicted, or destroyed. Measured: setting `p.light = undefined` leaves
**131 of 240 tiles still listing it**, the record still holding its colour, and the map entry still
present. Consequences compound: a **ghost light burns forever**, `carriedLights`/`lightOfBillboard`
grow without bound, ids are never reclaimed (`carriedLightNext` only increments), and the tiles it lit
are never dirtied so nothing repaints them.

**H2 — the resolve walk fails to the WORLD ORIGIN, silently.** `rootPos` starts at `0` and is only
assigned when a root is found. Exhaust `MAX_PRIM_DEPTH` — a chain deeper than 8, or a **cycle** — and
the loop simply ends: `decodePosition(0)` is tile (0,0), so the leaf teleports to the corner of the
world with no warning. `freeSubtree` has cycle detection; the resolve walk does not. This is the same
class as the two zoom regressions: wrong position, no error.

**H3 — `carriedLightFor` claims the carrier's slot b unconditionally**, overwriting whatever was there.
Harmless while a carrier holds at most a billboard and a light; silently destructive the moment a prim
carries more.

**H4 — billboard-safety logic gates other presentations** (user). `if (def < 0) continue` sits in a loop
that now handles lights too, so a billboard-specific guard decides whether a *light* is processed. Worked
around by hoisting the light attach above it; the structural fix is per-presentation guards.

**H5 — non-issue, checked:** `coldData.lights` excludes carried lights and feeds `setConstants`'
`light_count`, but **no shader reads that field**, so nothing is undercounted today. Recorded so the next
person doesn't re-derive it — and so it is caught if a shader ever starts reading it.

## I27 — Three silent droppers on the content→light chain (2026-07-25)
Wiring content-authored light took far longer than the code warranted, because **three separate layers
dropped the value without erroring**. Recording them because each is a trap for the next field added.

1. **`SquareCache.addPrim` rebuilds `Primitive` field-by-field.** It does not spread `spec`, so a new
   field on `PrimitiveSpec` type-checks at every call site and is then **silently discarded**. This is
   what actually broke the chain: `lightFor()` was returning a correct light and `addPrim` threw it
   away. Any field added to `Primitive` must ALSO be copied here.
2. **`node_visual` bails on a missing `tint`** (`store.read("prims.0.tint")?`), returning `None` for the
   whole `VisualParts` — so *every* attribute silently reads its default. A test fixture without a tint
   looks like "the new attribute doesn't parse" when nothing parses.
3. **The embedded DSL in Rust tests is indentation-sensitive** and must start at column 0. Re-indenting
   a test to match the surrounding Rust pushes hook bodies to a structural level; the file still loads
   "clean" and every value comes back default.

**Method note, the real lesson.** I burned several cycles probing URLs that return `index.html` on a
SPA — `/content/visual/things.rd`, `/shared/resonantdust_shared.js` — and read `200` + "no match" as
evidence the server was serving stale content. It was not; both were the SPA fallback. **A 200 is not
evidence a route exists.** What finally settled each question was asking the running system directly
(`/content` as JSON, `import()` of the actual module, the parsed table read out of the client) and a
bisect against a known-good value (`&thing.size`) to separate "my fixture is wrong" from "the feature
is broken" — which is exactly what it turned out to be.

## I28 — The scaffold was MASKING a frame-ordering bug; and glowing flora was an ambient hack (2026-07-25)
Two things surfaced the moment the debug light array was deleted.

**1. Presence must be built AFTER casters — and the earlier "verification" was worthless.**
`buildCasters` is what DISCOVERS a content-carried light and queues its dirty rects; `buildDirty` then
consumes those rects and `classPass` bakes the tiles. With presence built *first*, those tiles bake
against the OLD light set, and by the next frame — when presence is finally correct — the rects are
already spent, so nothing re-bakes. The light sits in presence, **permanently unlit**.
I had this fix, then reverted it on the (reasonable) argument that the dirty system should carry the
change. The dirty system does queue it; the problem is *when* it is consumed relative to presence.
**The revert appeared to work only because the orbiting debug light re-dirtied tiles every frame**, so
some later frame happened to bake with correct presence. Deleting the scaffold removed the cover and the
bug was immediate and total. **Lesson: a always-dirty debug object can hide an entire class of
invalidation bug — verify with the scaffold OFF.**

**2. Glowing flora was an ambient term in disguise** (user). The design is *dense point lights, NOT
sun/ambient*. flora scatters at ~16% of forest cells, so authoring a light on it produced **230 lights in
one view** whose pools overlap into a uniform wash — functionally the ambient model the design rejects,
and it hides the darkness that makes point lights worth having. It was chosen only because worldgen
already places flora, i.e. a *test* requirement leaking into world design. Reverted: no kind emits light
by default. The capability, the regression test, and `__torch(id?)` (light any placed billboard on
demand) all remain, so demonstrating the path costs no content change.

## I29 — Light cost model, measured (2026-07-25): static is FREE, moving cliffs at ~8–16
User asked for 32 moving + 32 static, expecting 64 to be comfortable. Measured at
`?focus=100,50&zoom=0.25` (7440-tile window, 1915 billboards), lights `reach 6`, spread so presence
never saturates. "Moving" is simulated by re-dirtying each light's cast region every frame — exactly
what a moving carrier does via `markLightDirty`.

| moving | dirty tiles/frame | fps |
|---|---|---|
| 0 (+32 static) | 0 | 120.4 |
| 4 | 616 (8%) | 120.7 |
| 8 | 1111 (15%) | 120.3 |
| **16** | 1479 (20%) | **55.4** |
| **32** | 3005 (40%) | **33.8** |

**Static lights are free, exactly as designed** — 32 of them cost 0.6 fps and dirty ZERO tiles/frame.
They bake once and never again; the cold/hot split does what it promised.

**`hot` alone is NOT a cost.** It routes a light to the hot RT; it does not by itself cause per-frame
work. A stationary hot light re-bakes never (32 of them: 120.2 fps, 0 dirty). **Motion** is the cost.

**The cliff is not the light count.** 8 → 16 moving grows dirty area only 33% while fps more than
halves, so cost ≈ **dirty area × lights-per-texel**: as moving lights overlap, every dirty texel loops
more lights through the corridor walk. The per-tile 16-light cap bounds the second term and does
nothing about the first. Dominant absolute term: the fine lightmap is **64×64 texels per tile**, so 16
moving lights re-bake ~**6M texels/frame**.

**Levers, cheapest first.**
1. **A moving light dirties its ENTIRE reach box even if it moved one pixel** (`markLightMove` unions
   old ∪ new). For sub-tile motion that is a ~169-tile re-bake for a few px of change. Dirtying only
   the swept delta would cut the common case hugely.
2. **Hot could skip the fine lightmap.** The tiered design always said hot should be *always-fresh*
   rather than baked; baking hot lights into the same 64/tile lightmap as cold is what makes motion
   cost area × 4096 texels.
3. Reach is quadratic in area — a moving light with reach 6 costs 4× one with reach 3.

**This is very likely the earlier unexplained "30 fps"** ([I24](#i24)): I could not reproduce it with
static lights or billboard count, and 32 moving lights lands on 33.8 fps.

## I30 — Corridor-walk redundancy, measured (2026-07-26)
Ported `walkShadow`'s corridor branch to JS exactly (line march, 5-tile cross pad) and ran it over the
**8 densest tiles** in the 64-light scene (7 lights each — 7 was the real max, not 8), sample point =
tile centre. **Yes: 16 lights ⇒ 16 independent corridor walks per texel**, because each corridor is the
line from *that* light to the sample point.

| lights | tiles searched | unique | inter-light overlap | bucket fetches | vs union-walk |
|---|---|---|---|---|---|
| 1 | 159 | 159 | 0% | 390 | −59% |
| 2 | 293 | 234 | 20.1% | 690 | −66% |
| 4 | 544 | 313 | 42.5% | 1260 | −75% |
| 7 | 1002 | 518 | **48.3%** | 2260 | **−77%** |

**Two independent redundancies:**
1. **The 5-tile cross pad wastes ~59% — with ONE light.** 159 unique tiles cost 390 fetches: consecutive
   steps along the line re-fetch the same neighbours. Pure loss, independent of light count. A supercover
   DDA visits each crossed tile exactly once. **Cheapest available win, no trade-off.**
2. **Inter-light overlap plateaus at ~48%.** It rises steeply to 4 lights then flattens, because all
   corridors converge on the SAME sample point — they share the tiles near the receiver and diverge only
   out toward their own lights.

Combined, walking the union once instead of per-light is **~77%** fewer bucket fetches (2260 → 518).
**Caveat on the union walk:** `casterOne`'s per-light culls (self-exclusion, seen-face, light-side)
currently reject candidates *inside* the per-light walk, i.e. for free. Union-gathering pulls in casters
irrelevant to most lights and rejects them later — fetches down, cull evaluations up. Net effect depends
on the cull hit-rate, unmeasured. The DDA carries no such uncertainty.

### I30 — the cross pad is LOAD-BEARING; exact DDA under-covers. RESOLVED 2026-07-26 (52.3%, identity holds)
The claim above — "the pad is pure loss, a supercover DDA visits each crossed tile exactly once" — was
**half wrong**, and the identity check caught it. Reasoning was: `buildCasters` already buckets a caster into
EVERY tile its ground footprint spans (rows topY..baseY, tight-bbox cols — I-7), so the pad can only be
covering SAMPLING misses, which an exact DDA has none of. Measured, an exact pad-free DDA **under-covers**:

| | mismatches | corridor-only | brute-only | both non-zero, differ |
|---|---|---|---|---|
| exact DDA, no pad | 1984 | **0** | **1792** | 192 |
| DDA + perpendicular dilation | **0** | 0 | 0 | 0 |

The direction is diagnostic: **zero** texels where the corridor finds shadow brute misses, 1792 the other way.
So the pad is not sampling slack — a caster is bucketed by its **tight-bbox** ground cover while `casterCover`
tests a **wider projected extent**, so a caster registered in tile T can occlude a ray through T±1.

**What IS redundant is dilating ALONG the ray:** consecutive walk tiles already supply each other's ±1 on the
dominant axis. Dilating only PERPENDICULAR to the dominant axis (3 fetches/tile, not 5) keeps the cover
conservative and holds identity at **0 mismatches over 102,442 non-zero texels**. Measured saving over the 12
live lights: 115,680 → 55,224 fetches = **52.3% fewer (2.09x)**, vs the ~59% the pad-free ideal promised.

**Method note — the first identity run was VACUOUS and reported a false pass.** It read 0 mismatches while
BOTH buffers were entirely zero: no kind emits light since the flora revert, so `carriedLights` was 0 and
there was nothing to cast. A shadow identity check MUST assert a non-zero population on both sides. Re-ran
through `__torch()` with 12 lights spread across the standing set.

### I31 — the fine lightmap is ~10x OVERSAMPLED at zoom 0.25 (open, and it gates F11b)
Measured at `zoom=0.25`: window 124×60 tiles, lightmap **64 texels per tile per axis** (one texel per world
px) → RT **7936×3840 = 30.5M texels**, against a **2560×1172** canvas. That is **3.10x** and **3.28x** per
axis, **10.2x** the texels the screen can show.

The excess is entirely a zoom-out artifact: the tile window grows as zoom shrinks while texels-per-tile stays
pinned at 64. At zoom 1 the window is ~40×19 tiles and the lightmap lands at ~1:1, which is why this never
showed up. The shadow RT is unaffected (16/tile/axis = 1984×960).

**Two consequences:**
1. **Memory** — it is what makes `RGBA32F` cost 465 MB instead of ~30 MB, so it is a **prerequisite for
   F11b**, not an optimisation ([forks.md](forks.md)).
2. **Fill rate — and the mechanism matters, because the obvious reading of it is wrong.** The lightmap density
   is **world-fixed**: measured `texelsPerWorldPx = 1` (64 texels/tile/axis ÷ 64 world px/tile). So a light's
   dirty region is a FIXED texel count at every zoom — reach 512 px → π·512² ≈ **823k texels** whether you are
   zoomed in or out. Per-light bake cost is therefore zoom-INDEPENDENT, and "the window got bigger" is NOT why
   movers are expensive.

   What IS true: the screen area that region fills shrinks with zoom. At zoom 0.25 those 823k texels cover
   π·(512·0.25)² ≈ **51k screen px** — a **16x** oversample per light region. At zoom 1 it is 1:1 and correct.
   Since the mover measurements (16 lights → 55 fps, 32 → 34 fps) were all taken at `zoom=0.25`, screen-tracking
   density is a ~16x lever on **exactly** the numbers that motivated F11b — much bigger than the DDA's 2.09x.
   Still worth a direct before/after measurement rather than trusting the arithmetic.

**Fix direction — SUPERSEDED, now owned by [2026-07-26-textile-slot](../2026-07-26-textile-slot/README.md).**
The fix recorded here (track screen density, ≈ `64 × zoom`) was right in direction but the user's design goes
further and better: size every map in **TILES** on a fixed 24×16 slot grid, so the texture is constant rather
than merely screen-proportional. That also equalises gameplay across monitors and caps the art pipeline.
Note for the record that `TEXTILE_SQUARE` was a **deliberate** choice by
[lightmap-resolution](../2026-07-24-lightmap-resolution/README.md) (spend memory to buy sharpness), not an
oversight — the slot grid keeps that sharpness where it is visible and stops paying for it where it is not.
**P9 in [`todo.md`](todo.md) is retired in favour of that stream; P10 (F11b) is gated on it.**

### I32 — P10's remaining items are ONE atomic change; landing them separately REGRESSES (2026-07-26)
P10 reads as five independent rows. Two are done (extension assert, `RGBA32F` + quantised deposits). The
remaining three are **mutually dependent**, and shipping any one alone makes the renderer worse.

**Where it stands.** `classPass` draws the light pass with `blend: "none"` — each dirty texel is REPLACED by
the full sum of its tile's present lights, recomputed from current state. That is *correct* (verified: warm
pools render, accumulator holds exact integers 308–321) but **not incremental**: a tile is re-summed over all
its lights whenever anything in it changes.

**Why the three cannot be separated:**
- **"Collapse hot/cold" alone is a REGRESSION.** The split exists so a per-frame hot light never forces
  static lights to re-bake. With one buffer and replace-semantics, every hot-dirty tile re-sums all 16 of
  its lights every frame — including the static ones the cold tier was keeping untouched. The collapse is
  only affordable once a light can be updated *without* re-summing its neighbours.
- **"Differential pass" needs the ping-pong.** Emitting `new − old` requires the OLD light parameters to
  still be readable. The data texture is written by sparse scatter (only changed texels), so a plain
  double-buffer swap would leave the previous buffer missing every unchanged record — it needs an explicit
  copy of `dataTex` → `dataTexPrev` before each flush, not a swap.
- **"Rebuild-and-diff self-heal" is VACUOUS today.** With replace-semantics there is nothing to leak, so the
  check would pass for the wrong reason — the [D-2](deviations.md#d-2) trap exactly. It only becomes a real
  assertion once contributions accumulate and can be left behind.

**So the ordering is fixed:** copy-based ping-pong → refactor `LIGHT_FRAG`'s accumulation into a function
parameterised by the data sampler (so it can run against old and new) → emit the difference under
`blendFunc(ONE, ONE)` → collapse the tiers → then the self-heal becomes meaningful.

**Risk note.** The failure mode of a bug here is *light that will not turn off* — residue that survives
because the subtract did not exactly match the add. That is the specific hazard F11b was designed around, so
this wants a focused pass with the exactness assertion in place, not a hurried one.


### I33 — the mover cliff is GONE, and the slot grid (not the accumulator) is why (2026-07-26)
Measured at `focus=100,50&zoom=0.25` with the user's 32 static + 32 dynamic:

| scene | fps | dirty tiles |
|---|---|---|
| 3 seeded torches | 120.2 | 2,507 |
| +32 static | 120.0 | — |
| **+32 dynamic, moving ±2 tiles/frame** | **120.1** | 3,221 |
| 259 lights / 224 movers | 120.0 | 4,093 |

Against the pre-change baseline of **16 movers → 55 fps, 32 → 34 fps**. Every run is pinned to the 120 Hz
vsync cap, so the true headroom is UNMEASURED — all that is established is that the ceiling is somewhere
past 259 lights with 224 of them moving.

**Method note.** The first attempt jittered movers ±2 px, which at `SQUARE = 128` cannot change a light's
resolved unit (8 px) — so nothing dirtied and the "moving" lights were static. Re-run at ±2 TILES, and the
dirty count rising 2,507 → 3,221 is what confirms the movers actually force re-bakes.

**Credit where it is due: this is the slot grid, not the additive accumulator.** The differential pass is
not built ([I32](#i32)) — the bake still replaces each dirty texel with the full sum of its lights. What
changed is the dirty AREA. At zoom 0.25 the old world-fixed lightmap gave a 6-tile-reach light ~823k texels
to shade; on the fixed grid at lod 2 a tile is 32 texels, so the same light covers ~147k — **5.6× less
work per light**, independent of how many lights there are.

**Consequence for [I32](#i32).** The differential pass was justified by a cliff that no longer exists at this
scale. It is still correct and still the right design — it makes cost independent of light count rather than
merely smaller — but it is now an OPTIMISATION rather than a rescue, and should be scheduled as one.

### I34 — GPU-timed: 64 lights cost 0.109 ms/frame, 1.3% of a 120 Hz budget (2026-07-26)
[I33](#i33) could only report "120 fps, vsync-capped". Measured properly with
`EXT_disjoint_timer_query_webgl2` at `focus=100,50&zoom=0.25`:

| scene | GPU ms/frame |
|---|---|
| 0 lights | 1.388 |
| **64 lights (32 moving ±2 tiles/frame)** | **1.497** |
| **lighting cost** | **0.109 ms — 1.7 µs per light** |

That is **18% of the 8.33 ms frame** in total, of which lighting is **1.3%**. Linear extrapolation puts
saturation past 4,000 lights; treat that as an order of magnitude, not a figure — the presence cap (16/tile)
and dirty-area effects will bend the curve long before then.

**A measurement method that does NOT work, recorded so it is not retried.** Bracketing a single frame with
`beginQuery` / `endQuery` from the console is unreliable: the app renders in its OWN `requestAnimationFrame`,
so depending on callback ordering the query can span an empty window. It reported **0.003 ms for 64 lights
and a NEGATIVE lighting cost** — the tell was `minMs: 0` in the baseline. Spanning **60 frames in one query**
fixes it: whatever the ordering, sixty frames of GPU work fall inside, and `GPU_DISJOINT_EXT` confirms the
timing was not invalidated.

**Conclusion for [I32](#i32) — SUPERSEDED by [I35](#i35).** I concluded here that the differential pass was
not needed for performance. That held at 64 lights and is false at 128; see below.

### I35 — the cost is LIGHTS PER TILE, not light count: 2x lights = 42x cost (2026-07-26)
Doubling the user's benchmark broke the linear reading in [I34](#i34):

| lights | GPU ms/frame | lighting cost | per light |
|---|---|---|---|
| 0 | 1.388 | — | — |
| 64 (32 moving) | 1.497 | 0.109 ms | 1.7 µs |
| **128 (64 moving)** | **6.029** | **4.641 ms** | **36.3 µs** |

**42.6× the cost for 2× the lights**, and the frame goes from 18% to **72.4%** used.

**It is not more dirty tiles.** The dirty count barely moved (3,221 → 3,552). It is dramatically more work
*per* tile: with the same area holding twice the lights, tiles approach the **16-light presence cap**, and
under replace-semantics **every dirty tile re-sums every light present on it**. Cost is
`dirty_tiles × lights_per_tile`, and only the second term moved.

**This is exactly what the differential pass fixes**, and it reinstates the performance case for
[I32](#i32) that [I34](#i34) had (prematurely) retired. Under `new − old`, a moved light touches only its
own contribution — a dirty tile costs **1** light-evaluation instead of up to 16, so cost becomes
`dirty_tiles × lights_MOVED` and stops scaling with local density altogether.

**Lesson about the earlier measurement.** 64 lights was below the knee, so the curve looked linear and
extrapolated to "~4,000 lights". That extrapolation was worthless — I even flagged it as an order of
magnitude rather than a figure, but still drew a scheduling conclusion from it. **Two points either side of
an unknown knee do not define a curve.** Measure at the density that matters, not the one that is convenient.

### I36 — the differential needs the previous SHADOW too, not just the previous light state (2026-07-26)
Found while refactoring `LIGHT_FRAG`'s accumulation. [F11b.1](forks.md#f11b1) and [I32](#i32) both frame the
ping-pong as "old vs new **light** records", but a light's contribution is

    colour x intensity x falloff x (1 - shadow) x N.L

and `shadow` does not come from the data texture. It is `texelFetch(uShadow, fcC, 0)` — a per-slot coverage
value read from the shadow RT, which the gather pass **regenerates every frame**. So evaluating the old term
against `uDataPrev` alone reproduces the old light *parameters* against the NEW *shadow*. Whenever a caster
moved, that is not what was originally deposited, and the subtract leaves residue — the exact failure mode
(light that will not turn off) the design exists to prevent.

**Resolved: ping-pong the shadow RT as well.** It is far cheaper than the data texture — 384×256 with two
RGBA32UI attachments is ~3 MB against the data texture's 16 MB — and it makes the old term exactly
reproducible: old light records against old shadow. Same discipline, same pre-flush moment.

Two alternatives, both rejected:
- **Re-derive the old shadow in-shader.** `walkShadow` IS callable from `LIGHT_FRAG` (the shadow-edge-refine
  work moved it into `GATHER_COMMON`), so the old coverage could be recomputed from `uDataPrev`'s caster
  records. But that is the corridor walk — the most expensive thing in the renderer — run a second time per
  texel per light, to save a 3 MB copy. Backwards.
- **Constrain instead: a caster move forces a full re-cast** of every light reaching it, and only
  light-moves use the differential. Correct, and it needs no extra memory, but it reintroduces exactly the
  `lights_per_tile` cost ([I35](#i35)) for the caster-moves case — which is the common one, since every
  walking pawn is a caster.

**Consequence:** [I32](#i32)'s ordering gains a step. Ping-pong the DATA texture (done), then the SHADOW RT,
then the parameterised accumulation, then the differential emit, then the tier collapse.

**The general principle (user, 2026-07-26).** This is the THIRD instance of one shape, and naming it should
stop a fourth:

| where | the trap |
|---|---|
| [F11b](forks.md#f11b)'s original "subtract before flush" | correctness depended on read-before-overwrite ORDER |
| [textile-slot I4](../2026-07-26-textile-slot/issues.md#i4) reproject | the slot permutation makes src/dst overlap |
| I36, the shadow | the old value is gone by the time the reader runs |

**If a pass needs the previous value of something another pass overwrites, keep two copies.** Do not sequence
around it. Ordering constraints are invisible in the code that depends on them and fail silently when
someone later reorders passes for an unrelated reason.

Strictly the shadow case is *lost state* rather than a same-pass read-modify-write — the gather and the
light pass are separate draws today. But the user's framing anticipates where this goes: `walkShadow` is
already callable from `LIGHT_FRAG` (shadow-edge-refine moved it into `GATHER_COMMON`), so if those passes
ever fuse, it becomes a LITERAL read-and-write of one resource. The ping-pong is what makes that fusion
safe to attempt later.
