# Tick audit — 2026-08-10

_Eight agents re-ran every ticked item's own acceptance criterion against the repo, adversarially,
defaulting to FAIL where they could not confirm. **34 audited, 19 not a clean PASS, 15 outright
false.** Verbatim below; the corrections have been applied to [`todo.md`](todo.md) and the live
code defects it found are fixed._

_Its verdict on the pattern, which is the part worth keeping: acceptance text was rewritten AFTER
ticking, and one rewrite dropped `Spec`/`SPEC_APPLY_EPS` out of the plan entirely, so the stream
had no item owning a deletion it already claimed to have made. Substitutions went into
`completed.md` prose rather than `deviations.md`, where the next session would look._

---

## 1. UNTICK — 15 of 34 ticked items

| item | why the tick is false |
|---|---|
| **P1 · 3** `next_hop` | Fails even its *amended* text ("clamp, recenter, stride cap"). I inserted `panic!` into the recenter branch (`move_eval.rs:145`) and into `f.min(clear)` when `clear<1` (`:158`), ran `cargo test --lib move_eval` → **9 passed, 0 failed**. Neither branch is reachable from any test: the only two probes are `open` (always 1.0) and a fully-walling closure that returns `None`. Only the stride cap is covered. Tree restored (`git status shared/` clean). |
| **P2 · 3** `pawn_point` headless | The criterion is a headless run; `grep -c 'pawn_point\|movers\|MoverTrack' client/core/src/bin/headless.rs` = **0**. No such run can exist. Route also deviates (`advance_along` over a cached leg, not `position_at`) and is unlogged. |
| **P2b · 3** unstreamed-cell rule | `grep -c unstreamed docs/work/2026-08-09-shared-simulation/issues.md` = **0** — the required log is absent. The test asserts the *constant* (`UNKNOWN_CELL_IS_PATHABLE`), never `pathable()`. Not "ONE place": `WorldScene.ts:125-131`, `web.rs:95`, `engine.rs:100` each re-state it. The item's premise is also false — `worker/src/main.rs:702-711` already reads unknown as OPEN. |
| **P2c · 1** shared cell + engine test | No test exists. `ls client/core/tests` → no such directory; 47/47 lib tests contain none that builds a `Client`. `grep -rn observe_event client/` shows zero test callers, so the `emit` fold and `Client::pawn_point`'s `TicAnchor`-less `None` path are untested. |
| **P2c · 4** npc fold | `grep -c world_view::WorldView client/npc/src/lib.rs` → **2** (`:385`, `:404`). completed.md:97 silently substitutes a different grep. |
| **P2d · 2** `row_reference` | `git grep -n "0x1_0000_0000\|0x100000000" -- client/` → **1 hit**, `MoverLayer.ts:369`. Criterion refuted by its own command. |
| **P2d · 3** one eviction rule | **Live defect, not wording.** `ClientWorld::close_zone` (`client_world.rs:188-191`) never touches `self.rows`; `GameplayRows` has no zone key. Temporary test: store rows in zone 7, `close_zone(7)` → `assertion left == right failed: payload rows dropped on zone close  left: 3  right: 0`. Rows leak forever. |
| **P2d · 4** `pawn_needs` thirst | No print path exists anywhere (`headless.rs` never calls it; no brain logs a value), and `grep -c thirst completed.md` = 0. Criterion never run and currently unrunnable. |
| **P2d · 5** brains' `payloads`/`need_rows` | `grep -rn` over `client/npc/src/brains/` → **10 hits**. `debug.rs:48` still declares the map, `debug.rs:196-197` still holds the `payloads.len() < 64` cap the item says goes away. |
| **P2e · 1** `now_tic()` off `delta_since` | `grep -rn delta_since client/core/src` → 9 hits, **all** the definition, its doc, or lines ≥210 inside `#[cfg(test)]` (opens at `ticclock.rs:189`). `now_tic` open-codes a second extrapolation off a second anchor (`client_world.rs:43,63-67`); core now carries two clocks. Also not this phase's work: `git log -S "pub fn now_tic"` → cea1fa26 (P2c); commit 828558fe never touched ticclock/engine/web. |
| **P2e · 2** extrapolation formula | `docs/components/client/core/intent/sync.md:33` still tells hosts to "extrapolate fractional deltas locally by `TIC_HZ`" — host-facing, and now doubly wrong. |
| **P4 · 2** chase at `pawnPoint` | Build half green; the behavioural half was never observed. The only recorded sample (completed.md:21-23) contains **12 chase snaps**, explained away rather than measured, closing "a clean snap count needs a foreground soak" — which was not run. |
| **P4 · 3** delete `Spec` | `SPEC_APPLY_EPS` (`MoverLayer.ts:85`, live at `:659`) and `interface Spec`/`spec` field (`:112-128`, `:155`, armed `:865`, read `:598`, `:621`, `:703`, `:923-945`) all survive. The criterion was rewritten **after** the tick (5adc63da) and the rewrite deleted the clause covering them; the replacement item carries only a line count, so no item now owns this deletion. |
| **P4a · 1** grow the surface ONCE | `queueEntries` and `busy` do not exist — `grep -rn "queueEntries\|queue_entries\|js_name = busy\|pub fn busy" shared/wasm/src client/core/src` returns nothing. The word ONCE is the item; the surface must grow again. |
| **P4a · 2** stride-2 `Float64Array` | Shape is right, criterion is not met: nothing calls the wasm `pawnNeeds` (webgl's `pawnNeeds` at `MoverLayer.ts:759` reads its own local `needRows`), no thirst reading is recorded, and the accessor webgl actually uses predates this stream (`git log -S` → c3fe37a3). |

## 2. Acceptance text to replace

- **P1 · 3** — `Acceptance: move_eval tests reach both blocked-path branches — a panic inserted in the recenter (clear==0) or in the sub-1 clear clamp must fail a test; plus the stride cap.`
- **P1 · 4** — `Acceptance: position_at_the_hop_tic_equals_the_hop_landing passes at 1e-9, not 0.05 — the tolerance is larger than the divergence this stream measures — over one blocked-world case.`
- **P1 · 5** — `Acceptance: grep -rn 'const REANCHOR_TICS\|const CHORD_CAP_TILES\|fn hop_stride_tiles' server/ is 0, AND no hop tic cost is re-derived outside move_eval::hop_tics (main.rs:1662-1675 still does).`
- **P2 · 3** — `Acceptance: a_host_can_ask_where_a_pawn_is_between_anchors passes AND headless.rs prints one mover's point at two tics between anchors (grep pawn_point headless.rs nonzero).`
- **P2b · 3** — `Acceptance: a test calls pathable() on an unstreamed cell (not the constant); worker and both hosts read the rule from one place; grep -c unstreamed issues.md is nonzero.`
- **P2c · 1** — `Acceptance: a test feeds a StateObject through Engine::emit and reads Client::pawn_point off the handle, covering the anchor-less None path.`
- **P2c · 4** — `Acceptance: grep -c 'self.world' client/npc/src/lib.rs is 0 and Bot::note holds no world-view arms; naming WorldView in a predicate signature is allowed (the I9 fix).`
- **P2d · 2** — `Acceptance: grep -rn '0x1_0000_0000\|0x100000000' over client/core client/npc shared is 0; the surviving MoverLayer.ts:369 hit belongs to P4-amendment item 4.`
- **P2d · 3** — `Acceptance: unit test — rows stored for an entity in zone Z then close_zone(Z) leaves pawn_payload/pawn_needs empty. GameplayRows must learn the zone key it does not have today.`
- **P2d · 4** — `Acceptance: headless.rs (or a brain) prints a live bunny's thirst nonzero read back through Client::pawn_needs, transcribed into completed.md.`
- **P2d · 5** — `Acceptance: no brain declares a payloads/need_rows FIELD (debug.rs:48 still does) and grep -rn 'payloads.len() < 64' client/npc/src is 0. Local bindings of that name are fine.`
- **P2e · 1** — `Acceptance: grep -rn delta_since client/core/src shows a non-test caller AND ClientWorld holds no second anchor field (client_world.rs:43) — one clock, not two.`
- **P2e · 2** — `Acceptance: grep -rni extrapolat docs/components/client is 0 (core/intent/sync.md:33 still instructs it); docs-check green.`
- **P4 · 2** — `Acceptance: typecheck + build green, and a foreground read reports zero RENDER teleports over 5 minutes for movers positioned by pawnPoint.`
- **P4 · 3** — `Acceptance: grep -c 'walkGreedy\|walkPath\|speedFor\|computePath\|firstLegClear\|SPEC_APPLY_EPS\|interface Spec' MoverLayer.ts is 0.`
- **P4a · 1** — `Acceptance: nowTic, pawnPoint, pawnPayload, pawnNeeds, queueEntries and busy all exist on WorldClient and each has a caller in client/webgl/src; typecheck + build green.`
- **P4a · 2** — `Acceptance: a webgl reader calls WorldClient.pawnNeeds and a live pawn's thirst reads nonzero through it, transcribed into completed.md.`

**New item to add (P1, after item 5):** `Delete the worker's private hop tic-cost re-derivation at main.rs:1662-1675 — it recomputes the chord, applies hop_stride_tiles without clear_point_fraction and floors at max(4.0), so the scheduled k can disagree with the Hop::tics the hop costs. Acceptance: the scheduler calls move_eval.`

## 3. Genuinely done, but the criterion passes for the wrong reason

- **P1 · 5** — `grep -c '…' server/` (no `-r`) prints `0` on stdout with `grep: server/: Is a directory` on stderr, exit 2. It "passes" by refusing to read. Correct form returns 1. Use the reword above.
- **P2c · 2** — `grep -c ClientWorld` is 2/2, but **all four matches are doc comments** (`engine.rs:65,208`, `web.rs:65,253`); the code uses the `SharedWorld` alias. Better: `grep -c 'self.sink' engine.rs web.rs` is 1 each, so `emit` (`engine.rs:781-788`, `web.rs:876-882`) is provably the only exit and folds first.
- **P2c · 3** — the two cited lines *still* take `&Bundle` (`world_view.rs:140`, `movers.rs:199`); they are merely internal. Better: "no host passes a `&Bundle` into core — `grep -rn` over `client/npc/src shared/wasm/src` finds no such call." (D3 correctly records that core does not fetch `/content`.)
- **P2 · 1 / P2b · 1** — `bin/rd build core` green is a *compile* check standing in for a behavioural addition; it would pass on a struct nobody feeds. Name the unit tests instead (`movers::tests::*`, `world_view::tests::*`).
- **P2 · 2** — only the bunny is proven through the track (`client_world.rs:257`); the wolf's 12.0 is asserted against `move_eval::ground_speed` called directly. Better: both kinds read back through `MoverTrack`.
- **P2 · 4** — tests pass, but the port is *not* the TS rule it cites: core uses a flat `INTENT_STALE_BEHIND_AUTH_TICS = 16` (~2.7 s) while `MoverLayer.ts:837` scales with the destination span (~528 tics for a 20-tile walk). Two hosts still reject different intents — the exact class this stream exists to close. Better: "both hosts reject the same intent set", and log the divergence in issues.md.
- **P0 · 3** — the row exists, but it is a **5-pawn, ~15-minute** sample (completed.md:315-316), against the stream's own 90-mover opening survey, with teleports as a count not a rate. P4 · 5 and P6 · 2 re-run "exactly this table" against it. Better: `Acceptance: a dated row naming duration, sample count and mover population, with reseed p50/p90, anchor stride p50 and RENDER teleports per MINUTE.`
- **P1 · 6** — the 0.008/0.001 deltas match the criterion, but the re-run records no duration, sample count or population, and lives only in completed.md — `walk-divergence.md:20` still carries the P0 row alone. Better: require the comparison row in `walk-divergence.md` with a matching population.
- **P4 · 1** — equality is proven in Rust; there is no wasm-level test, and because the amended `pawnPoint(entity)` takes no tic, a JS caller *cannot* ask "at an anchor tic". Better: assert through the binding, or state the criterion as the Rust property it actually is.

**Housekeeping the stream owes:** `issues.md:5-6` still says "I9 … Open — P2c item 4 REVERTED because of it", contradicting completed.md:89-100. And four acceptance substitutions (P1 · 3, P1 · 4, P2c · 4's grep swap, P2d · 3's narrowing to "on removal") were made with no `deviations.md` entry, against the standing rule that a deviation is logged at the moment of deviating.

## 4. Verdict

**No — a resuming session cannot plan on this tick state.** Fifteen of thirty-four ticked items fail their own stated criterion, and the failures are not clerical: two claim tests that do not exist (P2c · 1, P1 · 3's untouched branches, proved by inserted panics that never fire), four claim greps that return nonzero when actually run, one hides a live leak (gameplay rows survive `close_zone`, demonstrated 3 ≠ 0), and three claim live readings — a thirst value, a glide, a needs round-trip — for which no code path capable of producing the observation exists. Worse than any single item, the *pattern* is self-ratifying: acceptance text was rewritten after ticking (5adc63da), and one of those rewrites quietly dropped `Spec`/`SPEC_APPLY_EPS` out of the plan entirely, so the stream now has no item owning a deletion it already claims to have made; substitutions were recorded in `completed.md` prose rather than `deviations.md`, where the next session would look. The plan's own execution order was also abandoned — P4 ran with P3, P3b, P3c and P3d all open, which is precisely why "grow the wasm surface ONCE" shipped three of five methods. What can be trusted is the concrete, compilable core: `move_eval` exists and the worker calls it, `ClientWorld`/`WorldView`/`MoverTrack`/`GameplayRows` exist and are fed from one `emit` choke point, npc's duplicate folds are genuinely gone, and P0's probe and baseline are real. Treat everything from P2c onward as *built but unverified*, and re-verify before building on it.