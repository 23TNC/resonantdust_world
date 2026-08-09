# Findings — improvement audit (dated snapshot, 2026-08-08)

_This document is a SNAPSHOT ([I5](issues.md#i5)): statuses decay from the
date above; successor streams that pick an entry up should mark it here, but
this stream does not promise ongoing curation._

**Rubric** ([F4](forks.md#f4)) — impact: **A** player-visible correctness ·
**B** consistency/data risk · **C** performance ceilings · **D** velocity &
DX · **E** polish. Cost: **S** a session · **M** a stream · **L** several.
Status ([F3](forks.md#f3)): VERIFIED-OPEN · STALE · UNKNOWN.

## The ranked table

| # | candidate | impact | cost | why here |
|---|---|---|---|---|
| 1 | **W15 — settle the lighting-rework "reopened" shadow defects** (read + drill vs later streams) | A (if live) | S to verify, M if real | the highest-stakes UNKNOWN: four user-reported render defects either died silently or still ship |
| 2 | **W6 — per-facing subframe resolve cells** (same-kind pawns fight cell 0) | A | S-M | known live geo-flash class, small surface |
| 3 | **W14 — packCoPack retry + pool leak** (render-performance I2/I7) | A-D | M | "stuck on geo tier all session" is player-visible and a vanish-class relative |
| 4 | **W2+W3 — orchestrator assignment: single-owner events + no parking** | B | M | fixes minutes-late orders AND the double-assignment root; PREREQUISITE for the second worker |
| 5 | **W20 — satisfy into the module transaction** | B | S-M | must land before any second writer exists; cheap now, a race later |
| 6 | **C1 + W4 — worker corpus-unwrap panic; port the mint gate to bunnies** | B | S | two small known holes, one session together |
| 7 | **A2 — event retention / replay suppression** | B | M | the root the teleport class grew from; every future subscriber inherits it |
| 8 | **A6 — CREATE mint receipts** | B | M | closes the ±1 overshoot + adoption attribution class |
| 9 | **X-cluster — the DX session** (X1 mtime, X2 ANSI, X6 rd usage bug, X7 bench rename, X5 stub-race law, C3 dead code) | D | S | one sweep session; pays back every future session |
| 10 | **D3+D1 — archive sweep + current/ re-verifies** (with the user: the B1 backlog IS most of the 46 "open" rows) | D | S-M | un-buries the index; halves D2's warning for free |
| 11 | **W11+W12 — worker ceiling + burst mining** | C | S | the harness exists; two soaks and a log mine |
| 12 | **A3 — the second worker** | C | M | after #4 and #5; the scaling story's first real test |
| 13 | **X3+X4 — `rd sql` helper + live-subscription smoke check** | B-D | S-M | the two instruments every server change keeps needing |
| 14 | **W16 — zone fetch priority (+ stale-light re-cast)** | C-D | M | the user-authored deep-zoom unlock |
| 15 | **W17/W19 — primitive-graph I13 reach-ambiguity check; map-compat zoom-drift pinning** | B/A | S each to verify | old ⚠ items that today's reach-16 lights and zoom work may have made live |

Below the line (real, not urgent): W7 fall_off consumer · W8 runtime
thing-traits · W9 item-as-entity · A1 ownership · A4 pixijs retirement ·
A5 palette · A7 occupancy · A8 soul anchors · W21 hauling · W22 grid stems
· W10 logs-drop P3 · D4/D5 doc reads · P3 light density retune · P5 pacing
· art-program items (their own sessions).

## Lane 1 — work-stream debt

Coverage: all 46 index-open streams swept — the 2026-08-06..08 engine
streams from session context, the older/art streams by reader agents.

- **W1 · first-pawns I4 — phantom StateGone** · UNKNOWN · A/M — open since
  07-28; movement-hardening made StateGone trustworthy, but the original
  phantom report was never re-drilled. Cheap probe: none read-only (needs a
  soak). Successor: fold into the next movement soak's watch list.
- **W2 · spawn-authority I14 — parked events** · VERIFIED-OPEN · B/M —
  orders executed ~1000 tics late when their zones sat outside every active
  work-group; observed twice live (the "walks off on its own" teleport
  class). Structural: orchestrator assignment/activity gating. Successor:
  its own stream (pairs with W12).
- **W3 · spawn-authority I13 — cross-zone double-assignment** ·
  VERIFIED-OPEN (root) · B/S — the worker's `(event, instruction)` dedup
  guards it, but the ORCHESTRATOR still hands one event to two groups; the
  root fix is single-owner assignment. Successor: same stream as W2.
- **W4 · torch-perf I9 — the bunnies brain's mint-gate race** ·
  VERIFIED-OPEN · B/S — the latent `created < count` overshoot fixed in the
  debug brain lives on in bunnies.rs (grep confirms). Successor: port the
  `outstanding` gate; one session, with wolves' single-mint checked too.
- **W5 · bug-sweep — the vanishing wolf, not reproduced** · UNKNOWN · A/M —
  the probe eliminated prim/zone causes; suspect = pre-fix repack churn.
  Nothing to do until it recurs; the probe stays. Successor: none (watch).
- **W6 · bug-sweep I9 — same-kind pawns fight over subframe cell 0** ·
  VERIFIED-OPEN · A/S-M — two pawns of one kind at different facings churn
  the shared resolve cell (geo-flash class). Successor: per-facing resolve
  cells.
- **W7 · trait-lights I10 — `fall_off` authored, never consumed** ·
  VERIFIED-OPEN · E/M — the corpus carries it; the lighting render has no
  per-light falloff lane. Successor: a lighting-stream item when light
  authoring grows.
- **W8 · trait-lights F6 — runtime thing-traits** · VERIFIED-OPEN
  (by design, scheduled-never) · C/M — needs a thing-side gameplay
  sub-table; unlocks buffs/states on world objects. Successor: storage
  design first.
- **W9 · inventory — item `state` reserved (item-as-entity)** ·
  VERIFIED-OPEN · C/L — items are def-refs; durability/charge/nesting need
  the entity model the field reserved. Successor: object-model work.
- **W10 · logs-drop P3 parked** · VERIFIED-OPEN · E/S — the stream's last
  phase never ran (session note). Successor: finish or explicitly close it.
- **W11 · mover-perf — the real worker ceiling** · VERIFIED-OPEN · C/S —
  IN-STEP at 24; the ceiling is above the tested range and the harness
  (`NPC_BRAIN=debug NPC_COUNT=N`) makes finding it one soak per N.
- **W12 · mover-perf — the 256-event burst mechanism** · VERIFIED-OPEN ·
  C/S — transient 60–80-tic lag excursions correlate with large event
  batches; unexplained. Read-only successor: mine a soak's worker log for
  what fills those groups (queue fans? arrival clusters? seed traffic?).
- **W13 · spawn-authority I2-residual — ±1 mint overshoot on brain
  restarts** · VERIFIED-OPEN · B/S — root cause is CREATE returning no
  minted id, so adoption cannot be attributed (see A6). Tolerated at dev
  scale; listed for the ledger.
### Lane 1 addendum — the agent-swept older streams (engine)

- **W14 · render-performance I2 — a failed `packCoPack` never re-schedules**
  · VERIFIED-OPEN (code claim) · A-D/M — the stem stays on the geo tier all
  session; a plausible relative of bug-sweep's "vanishing wolf" suspect
  (pre-fix repack churn). Also I7 (SpritePool insert-only — superseded
  frames leak forever) and I4 (no resident-bytes tracking).
- **W15 · lighting-rework I12/I13 — the stream is marked REOPENED with four
  user-reported shadow defects** · UNKNOWN · A/M — its own issues.md says
  items were wrongly ticked (refine tested a rectangle, not a silhouette;
  shadows not anchored at the billboard base; z-order reads ground light
  over prims; min-bbox unverified) and that earlier ms figures ran ~4× low.
  Later streams (lighting-correctness, pawn-render, trait-lights) reshaped
  this ground — HOW MUCH of the reopened list survived them needs one
  focused read+drill before believing either way. The highest-priority
  UNKNOWN in this audit.
- **W16 · textile-slot I10 (user-authored) — zone FETCH priority missing**
  · VERIFIED-OPEN · C-D/M — bake priority exists, fetch does not;
  ring-ordered zone requests + an in-flight cap is the deep-zoom-out
  unlock. Beside it I3: stale lights on zoom-out are permanent without a
  background re-cast queue.
- **W17 · primitive-graph open cluster** · UNKNOWN-mixed · B-C/M — I13 ⚠
  (`resolved_tile` ambiguous past 8 tiles while reach runs 12–16 — if still
  live it touches today's torch lights), I38 (a light with k ≤ 0 silently
  drops its shadow — an ELEVATION authoring hazard now that traits author
  elevation), I8/I9/I12 mechanical renames + subtree lifetime. I39/I40
  (dead moving-light harness) look SUPERSEDED by pawn-render + the perf
  streams — tombstone pending one confirm.
- **W18 · subframe/def-frame residue** · VERIFIED-OPEN · A-E/S-M —
  `setSpriteScale` dead-keyed for cold things (the conifer's authored 0.5
  is INERT since def-frame-anchors P5), thing overrides don't repaint on a
  routing change, no ingest-time check that an authored subframe matches
  its art (subframe-ingest I6/I7), the drawn-box derivation + ANCHOR
  question (I9), and def invalidation on hot-swapped sprite_scale.
- **W19 · map-compatibility I-1 — on-prim zoom drift unpinned** · UNKNOWN ·
  A/M — two failed attempts; the stream's own law: pin the mechanism before
  attempt #3.
- **W20 · interactions I3 — satisfy is a read-modify-write, single-writer
  TODAY** · VERIFIED-OPEN (constraint) · B/S-M — becomes a live race the
  day a second worker exists; must move into the module transaction first.
  Sequenced BEFORE A3.
- **W21 · logs-drop I5 — hauling** · VERIFIED-OPEN · E/M — logs are inert
  scatter; the pick-up/haul successor is named, and its parked P3 (W10)
  carries four execution notes ready to run.
- **W22 · def-frame-anchors — grid stems second-class** · VERIFIED-OPEN
  (user deferred: "I'll deal with it later") · D/M — per-cell scale,
  pad-inset origins, whole-image spriteBBox; plus the C5 texture-array lift
  behind pool spill + eviction.
- Streams with unchecked boxes but EMPTY issues (not-started or paused):
  tree-occupancy (12 open), lighting-performance (9), showrt (5),
  linked-cell-pad (19, incl. the two-meanings-of-`pad` decision),
  normal-frames (18, camera-vs-world space stated up front) — inventory
  only; they are D3's archive-or-revive material.

### Lane 1 addendum — the art program (condensed per I4's bound)

The art streams are an ACTIVE parallel program with its own daily sessions;
the audit lists structure, not their backlog: the evaluation crisis is real
and being worked (metrics measure the silhouette ControlNet guarantees —
ns-evaluation I1/I5; the vision-gate stream is in flight); the template
monoculture (one authored wolf template; `auto` control picks primates —
east-pipeline I1/I6) and the untrained-species ruler gap (I3/I7) are the
named next walls. Two SMALL engine-adjacent items worth lifting out:
**lineart-lora I1** (`split_albedo` IndexError needs a degrade-to-no-layers
guard — one session) and **lora-beat-e07 I2** (prep_train silently swaps
ESRGAN for LANCZOS when ComfyUI is down, and ComfyUI is not in autostart —
a silent-quality-loss trap).

## Lane 2 — docs drift

- **D1 · stale `current/` stamps** · VERIFIED-OPEN (warns every docs-check)
  · D/S — client/webgl current/README + lighting-seam (stamped 07-31, code
  moved 08-08) and the index module (07-28 vs 08-05). Successor: one
  re-verify pass each.
- **D2 · 540 oversized plan items across ~30 streams** · VERIFIED-OPEN
  (the docs-check warning) · D/M — mostly in DELIVERED-but-unarchived
  streams, which makes D3 the real fix; decomposing live streams' items is
  the residual.
- **D3 · the work index carries 46 "open" rows** · VERIFIED-OPEN · D/S-M —
  the archive law says delivered streams move to /home/wolf/archive/;
  reality: dozens of done-but-for-B1 streams still sit open in docs/work,
  burying the genuinely active ones (this audit had to agent-sweep 46
  folders because the index cannot say which are live). Successor: a
  B1-review + archive sweep with the user (their eyes are the gate on most).
- **D4 · docs/object-model.md marked "in flux"** · UNKNOWN · D/S — whether
  it still reflects intent post-registry/spawn-authority needs a read.
- **D5 · texture layout migration PENDING** · VERIFIED-OPEN (memory +
  texpath.py) · D/S — the go-forward folder shape is documented; texpath.py
  still emits the old leaf.

## Lane 3 — perf headroom

- **P1 · the worker ceiling** — see W11.
- **P2 · the burst mechanism** — see W12.
- **P3 · light slot eviction density** · VERIFIED-OPEN (measured ~50k/s at
  25 lit movers) · C-E/S — benign at saturation (nearest-8 wins), but
  reach-16 as the default torch tuple is the lever: authored-density
  guidance or shorter reaches keep tiles under the 8-slot cap. Successor:
  a content retune when lights proliferate; watch for the
  bright-far-light-vs-8-near-dims artifact (the sort ignores intensity).
- **P4 · parked events** — see W2 (perf-visible as minutes-late orders).
- **P5 · master pacing 5.96–5.98 Hz vs 6.00 target** · VERIFIED-OPEN
  (every pacing report) · E/S — ~0.5% slow on WSL2; cosmetic today, worth a
  line when timing-sensitive features land.

## Lane 4 — code sweeps

Commands: `grep -rn "TODO|FIXME|HACK" server/ client/webgl/src client/npc/src
shared/` (16 hits, 15 in GENERATED bindings — excluded per I4);
`grep -rn ".unwrap()|.expect(" per server crate` (worker 17, master 15,
orchestrator 2, edge 54 — sampled); `cargo`-warning dead code.

- **C1 · worker panics on a corpus missing the `inventory` need** ·
  VERIFIED-OPEN · B/S — main.rs:2322-23 `.unwrap()` on
  `gameplay_reference("need","inventory")`; a content edit that renames or
  drops the need kills the worker at runtime, not at load. Successor: a
  refuse-and-log path (one session, sweep siblings while there).
- **C2 · edge unwraps are mostly mutex-locks + tests** · STALE as a
  candidate — the lock-poison recovery stream already hardened the pattern
  (lock.rs documents it); no action.
- **C3 · edge dead code** · VERIFIED-OPEN · E/S — `place_things as _`
  unused import + `is_prefix` never used; two-line cleanup rides any edge
  commit.
- **C4 · VideoPanel renderScale TODO (webgl-engine W4)** · VERIFIED-OPEN ·
  E/S — the panel exposes a knob the renderer ignores.
- Non-live lanes (pixijs, generated bindings): one line each per I4 —
  pixijs carries its own debt but is retirement-bound (A4); bindings TODOs
  are upstream codegen noise.

## Lane 5 — architecture gaps (named in design/intent/memory, unscheduled)

- **A1 · ownership model** · VERIFIED-OPEN (deliberate posture, see below)
  · B/L — the edge door is a verb allowlist; any client may drive any pawn.
  Re-decide when: anything beyond single-tenant dev.
- **A2 · event retention / re-subscribe replay** · VERIFIED-OPEN · B/M —
  "the event table replays HISTORY on subscribe (no retention yet)" is the
  root the teleport class grew from; the client now guards (authTic), but
  every future subscriber inherits the hazard. Successor: retention or
  replay-suppression at the shard/edge seam.
- **A3 · the second worker** · VERIFIED-OPEN · C/M — the
  orchestrator/worker split was built for N workers; N has never exceeded 1.
  Pairs with W2/W3 (assignment semantics must be settled first).
- **A4 · pixijs retirement** · VERIFIED-OPEN · D/M — client/pixijs remains
  in-repo while webgl is go-forward; carrying cost is real (greps, builds,
  confusion). Successor: delete when the user calls it.
- **A5 · palette generator** · VERIFIED-OPEN · E/M — the 256-color OKLCh
  master is designed, not in-repo; trait-lights' authored colors are ready
  to become palette refs when it lands.
- **A6 · CREATE returns no minted id** · VERIFIED-OPEN · B/M — the root
  under W13 and the npc adoption-attribution gap; a mint receipt (even a
  fanned request→id echo) closes a whole class of races.
- **A7 · server-side thing occupancy + n/s footprint swap** · VERIFIED-OPEN
  · C/M — deferred in the spatial model; multi-tile things collide only
  client-side today.
- **A8 · soul anchors** · VERIFIED-OPEN · C/M — deferred in
  client-anchor-zones.
- **A9 · world-storage P3′/P5/P6** · UNKNOWN · C/M — the shard-tables
  stream's tail phases; whether events-replace-reducers still matters
  post-spawn-authority needs a read.
- **A10 · refusal feedback channel** · deliberate posture (below) — listed
  here because the pie menu's silent no-ops will eventually be a player
  question.

## Lane 6 — DX friction

- **X1 · docker/WSL2 cargo mtime miss** · VERIFIED-OPEN (hit ~5× today) ·
  D/S — every build needs the `touch` ritual or risks running yesterday's
  binary; the stale-binary guard catches RUN but not BUILD. Successor: a
  `bin/sim build --force`-style always-touch, or checksum-based rebuild.
- **X2 · ANSI in sim logs breaks grep** · VERIFIED-OPEN (every log mine) ·
  D/S — the `sed 's/\x1b...//'` ritual; `RUST_LOG_STYLE=never`/`NO_COLOR`
  in the containers is a one-line fix.
- **X3 · the SpacetimeDB CLI concatenates array columns undecodably** ·
  VERIFIED-OPEN · D/S — every payload/items inspection needs a bespoke
  python decoder against the HTTP JSON API. Successor: a `bin/rd sql`
  helper that hits the JSON API and pretty-decodes known packings.
- **X4 · subscription SQL is a string** · VERIFIED-OPEN (build-gates
  memory) · B-D/M — schema changes pass both compile passes and fail live.
  Successor: a live-subscription smoke check in the redeploy path.
- **X5 · concurrent-session docs-stub clobbering** · VERIFIED-OPEN
  (TWICE today — east-pipeline I4, ns-evaluation I6) · D/S — a docs-green
  helper that WRITES into a folder another session is authoring destroys
  seconds-old work with no conflict signal. The victims' own proposal is
  right: create-only-if-absent (O_EXCL semantics) or report the gap without
  writing. Successor: encode the rule in the docs conventions + (better) a
  docs-check flag that tolerates a mid-creation folder for one run.
- **X6 · `bin/rd` prints a syntax error on every bare invocation** ·
  VERIFIED-OPEN (`command substitution: line 87: syntax error near
  unexpected token 'newline'`) · D/S — cosmetic but erodes trust in the
  tool; likely a heredoc/usage-string quoting slip.
- **X7 · instrument naming (`__lightcost`)** · VERIFIED-OPEN · D/S — a
  synthetic bench named like a live probe cost a wrong reading twice;
  rename or doc-comment it (`__lightBench`), and note it mutates
  droppedLights.

## Deliberate postures (listed, not ranked — [I1](issues.md#i1))

- **No ownership model** — the verb allowlist is the whole door, by
  design. Re-decide when: any second real tenant.
- **Refusals log-only** (spawn-authority I3) — /spawn and menu no-ops stay
  silent in chat. Re-decide when: a player can't tell why nothing happened.
- **Placeholder flat-square art** — the sprite pipeline is its own program.
- **The u8|u8 trait split, withdrawn** — available if a trait ever needs
  per-bind scalar data.
- **Debug pawns are deathless** — cleanup = pawn wipe, accepted for dev.
- **Dev-scale races accepted and stated** — the ±1 mint overshoot (W13),
  the cold-SET occupancy race (two requests, one cell, compose order wins).

## Ranking

_Assembled in P2 after the agent-swept lanes integrate._
