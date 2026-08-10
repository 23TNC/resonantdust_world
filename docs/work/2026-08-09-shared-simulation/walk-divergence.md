# Walk divergence — the adversarial source analysis (2026-08-10)

_A 42-agent read of the two walk implementations: four independent close-reads, four lens-specific
divergence hunts, then one refutation pass per candidate. **33 candidates, 11 survived**, and the
verifiers were strict enough to catch that this stream's own P1 extraction had moved the worker
code out from under their citations mid-run._

_Independently reached the same conclusion as [I1](issues.md#i1)'s retraction, from the source
rather than from measurement. What follows is the synthesis verbatim; it is the input to P4._

---

## 1. Ranking — and the honest verdict

**None of the eight explains the 2×, because the 2× is not real.** It was withdrawn on 2026-08-10 at `docs/work/2026-08-09-shared-simulation/issues.md:5-24` as a species-pooling artifact. The healthy per-kind re-measure (`completed.md:69-73`) reads:

| | bunny | wolf |
|---|---|---|
| implied pace p50 (p10/p90) | **24.26** (23.77 / 28.44) | **12.07** (11.91 / 15.41) |
| anchor stride / predicted | 1.326 / 1.333 | 2.652 / 2.667 |

`impliedPace = gap / stride` (`MoverLayer.ts:530`) is self-normalising, so it survives missed anchors — a dropped row inflates both gap and stride and leaves the ratio alone. It reads the authored pace on both kinds. There is no pacing divergence to attribute.

Worse for the eight: **seven of them cannot touch that number by construction.** `anchorStride` is `Math.hypot(ax - m.authX, ay - m.authY)` between two *server* rows (`MoverLayer.ts:1093-1099`), and seven candidates live entirely in `MoverLayer.ts`. The one server-side candidate (the `clear_point_fraction` clamp) has the wrong sign: `f = f.min(clear)` with `clear ≤ 1` (`shared/content/src/move_eval.rs:120-123`) can only *shorten* a hop.

The eight are also not eight. They are five distinct defects, double-reported by two lenses:

| rank | defect | contribution to the retracted 2× | contribution to the surviving reseed error (bunny p50 1.25 / p90 4.60 / max 12.53) |
|---|---|---|---|
| 1 | `walkGreedy` is Chebyshev where everything else is Euclidean (`MoverLayer.ts:135-152`, `prog` is tiles at `:648`) | **zero** | ≤ 0.39 tiles per interval, and only on the `path === null` branch |
| 2 | `walkGreedy` cannot terminate from a fractional start (`:144` vs fractional seeds at `:989-990`, `:1060-1061`) | **zero** | ≤ ~1.4 tiles, same branch |
| 3 | client reseed skips the `clear` clamp (`:1067` calls `computePath` with no `firstLegClear`; `firstLegClear` at `:919-935` is called only from `onMoveIntent` at `:985`) | **zero** (wrong sign server-side) | ≤ 1.33 tiles, terrain-local only |
| 4 | `spec.ticsPerTile` frozen at arm (`:994`, never rewritten by the reseed branch `:1058-1067`) | **zero** | ~1% of movers (`completed.md:196`) — owns part of the max, nothing else |
| 5 | same-tic intent tiebreak: client first-wins (`:954`), worker last-wins (`main.rs:804`, `:2953`) | **zero** | rare; tail only |
| 6 | chase budgets Chebyshev, spends per-axis (`:665`, `:683-684`) | **zero** | **zero** — feeds neither metric; presentation only |
| 7 | waypoint origin-vs-centre | **zero** | **zero** — no divergence exists; doc defect only |

Every one of these is bounded by *one anchor interval of progress*, which for a bunny is `32 / 24 = 1.33` tiles. They arithmetically cannot produce p90 4.60 or max 12.53. So they do not explain the surviving symptom either.

### What does explain the surviving drift — three mechanisms, none of them in the eight

**(a) The spec arms from the RENDERED point; the worker steps from the AUTHORITATIVE point.** `MoverLayer.ts:978-979` takes `bx = m.rx, by = m.ry` and seeds `fromX/fromY/appliedX/appliedY` with it (`:989-997`). The worker's `MOVE_TO` seed resolves and steps from `p.position_reference` (`main.rs:2936-2948`). So every arm bakes the current render-chase lag straight into the spec, and the chase is *allowed* to lag right up to `CHASE_SNAP_TILES = 3` before it snaps (`:252`, `:668`). The lag is slow to shed: at pace 24 the true speed is `6/24 = 0.25` tiles/s and the chase runs at `CHASE_CAP = 1.2 ×` that, so the surplus closing rate is 0.05 tiles/s — **60 s to erase 3 tiles**. With 731 of 1157 move orders interrupting a live walk (`issues.md` I4), lag is re-seeded far faster than it is paid off. This mechanism alone spans 0 – 3 tiles and matches the *body* of the distribution.

**(b) Neither walk has a stride cap, and anchors are not guaranteed.** `walkPath` distributes `prog` across the *entire* polyline (`:158-179`), and `prog = ticDelta(s.eventTic) / s.ticsPerTile` (`:648`) grows without bound between reseeds. Three worker branches produce no anchor: the blocked retry `_ => (vec![MOVE_STEP, obj, dest, serial], 8)` at `main.rs:1678` is **bare — no `PROMOTE`**; the same `_` arm swallows the on-dest-tile-not-on-dest-point case, because `find_chords((tx,ty),(tx,ty))` returns `Some(vec![])` (`path_eval.rs:341-343`) and the guard `Some(chords) if !chords.is_empty()` at `main.rs:1669` rejects it; and a failed hot write `continue`s the whole tic (`main.rs:955-958`). Any of these stretches an interval well past 32 tics while the client keeps gliding the whole route. Unbounded — this is the shape of a p90/max tail.

**(c) A server-side jump from a stale `base_tic`.** `main.rs:2938-2947` updates `p.base_tic` **only if the resolve moved the pawn**. A `MOVE_TO` on a resting pawn therefore leaves `base_tic` at the previous trip's last hop, arbitrarily far back. If a second order lands before the first hop fires, `resolve_walk_position_for` (`main.rs:407-427`) calls `position_at` with that ancient `base_tic`; `elapsed` is huge, `f = (elapsed/pace/len).min(1.0).min(clear)` saturates at 1 (`move_eval.rs:164-166`), and the pawn is written **all the way to the first waypoint** — a chord that is string-pulled and arbitrarily long. `position_at` has **no stride cap**, unlike `next_hop`. Rate is right for the tail: 35 resolves / 10 min (`completed.md:212`) ≈ 1.5% of reseeds.

### The measurement that settles it

Do not measure geometry. Emit one CSV row per reseed pairing `reseedErr` with the three quantities that discriminate the three mechanisms — extend `noteReseed` (`MoverLayer.ts:538-545`):

1. `authDt` (already computed at `:1097`) — **elapsed tics since the previous anchor**;
2. `prog` at the moment of the reseed (`d / s.ticsPerTile`);
3. `armLag` = `|m.rx − m.authX|` captured at arm time and carried on the `Spec`.

Then:
- `reseedErr ≈ armLag`, roughly flat in `authDt` → **(a)**, the arm-time seed. Fixed by P4 deleting `Spec`.
- `reseedErr` grows with `authDt`, with a mass of samples at `authDt ≫ 32` → **(b)**, missing anchors. Fixed by making every hop — including the blocked retry and the dest-point normalisation — `PROMOTE`.
- `reseedErr` large with `authDt ≈ 32` and an `AUTH` teleport on the same row (`:1104-1109`) → **(c)**, the stale-`base_tic` jump. Fixed server-side by stamping `base_tic` unconditionally and capping `position_at`.

Also add a worker counter on the `_ =>` arm at `main.rs:1678`; if it fires often, (b) is settled without any client instrumentation.

---

## 2. Spec — `shared/content/src/move_eval.rs`

### 2.0 What is wrong today, before any signature changes

The module already exists and the worker already calls it (`main.rs:2988-2993`, `main.rs:418-425`). Three internal disagreements survive inside it, and one more between it and its caller. All four must be closed by this spec.

| # | disagreement | where |
|---|---|---|
| D1 | `next_hop` caps travel at `hop_stride_tiles(pace)`; `position_at` **does not** — at large `elapsed` it walks the pawn to the *full* first waypoint | `move_eval.rs:120` vs `:166` |
| D2 | `next_hop` **recenters** onto the tile anchor when `clear ≤ EPSILON`; `position_at` returns `from` unchanged (its `f` collapses to 0 at `:167`) | `move_eval.rs:111-115` vs `:159-169` |
| D3 | `next_hop.tics` is computed (`:127`) and then **discarded** by the worker, which schedules with its own `k = (dist*pace).ceil().max(4.0)` — no `clear` term, no recenter term, and a 4-tic floor `hop_tics` does not have | `main.rs:1673-1675` vs `move_eval.rs:127`, `:133-135` |
| D4 | `Some(vec![])` ("already there") and `None` ("no route") are collapsed by three separate callers, each into a different behaviour | `main.rs:1669/1678`; `shared/wasm/src/lib.rs:536-543`; `MoverLayer.ts:909-910` |

The existing test `position_at_the_hop_tic_equals_the_hop_landing` (`move_eval.rs:236-255`) hides D1 (it samples at exactly `hop.tics`, where the two happen to coincide) and hides D2 (open field ⇒ `clear = 1.0`).

### 2.1 New constants

```rust
/// Minimum tics a hop may be scheduled for. Bounds the anchor fan rate when a hop is
/// tiny (a clear-clamped step, a recenter, a chord shorter than the stride). Was the
/// unnamed `.max(4.0)` in the worker's CONTINUE pass (main.rs:1675).
pub const MIN_HOP_TICS: u16 = 4;

/// Tics before a blocked chain re-routes. Was the unnamed `8` at main.rs:1678.
/// Public because an observer must know when to expect the next attempt.
pub const BLOCKED_RETRY_TICS: u16 = 8;
```

`REANCHOR_TICS = 32.0` (`:25`), `CHORD_CAP_TILES = 8.0` (`:28`) and `hop_stride_tiles` (`:36-38`) stay verbatim.

### 2.2 `next_hop`

```rust
pub enum Step {
    /// The pawn is on the destination tile but not on the destination POINT.
    /// Land exactly on `dest`. Arrival is an equality test, so this must be exact.
    Arrive { point: (f64, f64), facing: u8, tics: u16 },
    /// One stride along the first chord.
    Hop { point: (f64, f64), facing: u8, tics: u16 },
    /// No route this hop, or a degenerate leg. Step nothing; retry in `BLOCKED_RETRY_TICS`.
    Blocked,
}

pub fn next_hop(from: (f64, f64), dest: (f64, f64), pace: f64, pathable: Pathable) -> Step
```

**The tri-state return is mandatory (closes D4).** `Option<Hop>` is what lets `main.rs:1669`, `lib.rs:536-543` and `MoverLayer.ts:909-910` each invent a different meaning for "empty". A caller must not be able to conflate *arrived*, *hopping* and *blocked*.

Semantics, in order:

1. **Pace guard.** `let pace = pace.max(1.0);` A pace below 1 is refused upstream by `can_move_ground`; clamp rather than divide. (`hop_stride_tiles` already does this internally at `:37`; do it once at the top so the tic cost sees the same number.)
2. **Arrival.** If `tile_of(from) == tile_of(dest)`, return `Arrive { point: dest, facing: facing_of(dest − from), tics: hop_tics(‖dest − from‖, pace) }`. Preserves `move_eval.rs:103-106` **and** `main.rs:2976-2980`, which are today the same rule written twice.
3. **Route.** `find_chords(tile_of(from), tile_of(dest), 0, pathable)`; `None` or an empty vec ⇒ `Blocked`. (Empty is unreachable here because step 2 already caught `from == to`; return `Blocked` anyway rather than `?`-collapsing, so the impossible case is named.)
4. **Leg.** `leg = chords[0] − from`, the first waypoint as a raw integer world point — **the tile ORIGIN, not its centre**. This is the current convention on both sides (`move_eval.rs:73-75`, `MoverLayer.ts:911`) and it is correct given `position_to_point` (`shared/codec/src/object.rs:466-472`). The `path_eval` doc comments that say chords "connect their CENTERS" (`path_eval.rs:132-133`, `:317`) are the thing that is wrong; fix the comments, not the code.
5. **Clear fraction.** `clear = clear_point_fraction(from, chords[0], pathable)` (`path_eval.rs:376-401`) — the off-lattice segment, 1/32-tile sampling, start tile exempt, one sample of backoff.
6. **Recenter.** If `clear <= f64::EPSILON`, replace the leg with `tile_of(from) − from`, and **set `clear = 1.0`** — a segment inside one tile is legal by construction. (Today `clear` stays 0 and the subsequent `if clear > EPSILON` at `:121` silently skips the clamp; that works but only by accident, and it is the reason `position_at` diverges. Making it explicit is what closes D2.)
7. **Degenerate.** `len = leg.hypot()`; `len <= EPSILON` ⇒ `Blocked`. (Reachable after a recenter when the pawn is already on its tile anchor.)
8. **Fraction.** `f = (hop_stride_tiles(pace) / len).min(1.0).min(clear)`. One expression, both clamps unconditional — `clear` is now always in `(0, 1]`.
9. **Result.** `point = from + leg·f`, `facing = facing_of(leg)` (`:81-85`, east/west dominant on a tie), `tics = hop_tics(len·f, pace)`.

```rust
fn hop_tics(distance: f64, pace: f64) -> u16 {
    (((distance * pace).ceil() as i64).clamp(1, u16::MAX as i64) as u16).max(MIN_HOP_TICS)
}
```

**The `MIN_HOP_TICS` floor moves here — follow the worker (`main.rs:1675`), not `move_eval.rs:133-135`.** Reason: the floor is a fan-rate policy, and the worker's live behaviour is the one that has been soaked. Adopting `hop_tics`'s current floor of 1 would let a clear-clamped hop fan an anchor every tic. A floored hop simply means `position_at` reaches the landing early and holds — which §2.3 makes explicit rather than accidental.

**`Blocked` never carries a distance**; the caller schedules `BLOCKED_RETRY_TICS`.

### 2.3 `position_at`

```rust
pub fn position_at(
    from: (f64, f64), dest: (f64, f64),
    base_tic: u16, now: u16,
    pace: f64, pathable: Pathable,
) -> (f64, f64)
```

**Contract, stated as an identity rather than as a procedure — this is the whole point of the pair:**

> For all `t`, `position_at(from, dest, b, b + t, pace, pathable)` walks the *same* segment `next_hop` walks, at exactly `1/pace` tiles per tic, and **saturates at `next_hop`'s landing point**. In particular `position_at(from, dest, b, b + tics, …) == next_hop(…).point` **exactly**, and stays there for every `t > tics`.

Implementation must therefore be *the same function with a different fraction*, not a parallel derivation:

1. Run `next_hop`'s steps 1-7 verbatim (share them: extend the private `first_leg` at `:67-77` to return `(leg, clear)` **after** the recenter, so both public functions consume an identical `(leg, clear, len)`).
2. `Arrive` ⇒ return `dest`. `Blocked` ⇒ return `from`.
3. `let raw = now.wrapping_sub(base_tic); let elapsed = if raw > u16::MAX / 2 { 0 } else { raw } as f64;` — keep `move_eval.rs:164-165` verbatim. A future `base_tic` reads as elapsed 0, never as almost a whole ring. Wrapping is ordinary elapsed time.
4. `let f_time = elapsed / (pace * len);`
5. `let f = f_time.min(hop_stride_tiles(pace) / len).min(1.0).min(clear);` — **the stride cap is the new term (closes D1).** Without it, a `now` more than one hop past `base_tic` teleports the pawn to the full first waypoint, which is exactly mechanism (c) above and the reason a stale `base_tic` at `main.rs:2938-2947` can move a pawn many tiles authoritatively.
6. `if f <= f64::EPSILON { return from }` else `from + leg·f`.

`position_at` **keeps the recenter** (step 6 of §2.2). Today it silently drops it (D2): `next_hop` moves the pawn to its tile anchor and `position_at` says it never left. Follow `next_hop`, because `next_hop` is what actually writes state.

### 2.4 Required caller changes — the disagreements resolved, with the side chosen

| what | today | shared version | which side, and why |
|---|---|---|---|
| **The hop's tic cost** | CONTINUE pass hand-computes `k` from `find_chords` + `hypot` + `.max(4.0)` (`main.rs:1663-1676`); `hop.tics` discarded (`main.rs:2994-2999`) | CONTINUE pass calls `next_hop` from the post-apply position (`main.rs:1646`) and schedules `hop.tics`; the apply arm calls `next_hop` and consumes `hop.point` / `hop.facing` | **`next_hop`.** Same function, same inputs at both tics ⇒ the schedule and the landing agree *by construction*. Today they differ whenever `clear` binds or the recenter fires — the schedule pays for a hop the execution will not make. |
| **`MIN_HOP_TICS` / `BLOCKED_RETRY_TICS`** | unnamed `4.0` and `8` inside the worker | named constants in `move_eval`, public | **Worker's values.** They are soaked policy; only their *location* is wrong. Public because an observer that cannot predict the next anchor's tic cannot avoid over-running it — mechanism (b). |
| **On-dest-tile, off-dest-point** | falls into `_ =>` at `main.rs:1678`: bare `MOVE_STEP`, `k = 8`, **no `PROMOTE`** — the final normalisation onto the dest point never fans | `Step::Arrive` ⇒ `PROMOTE`, `tics = hop_tics(…)` | **`next_hop`.** The worker gives two different answers to one state (`main.rs:2976-2980` normalises; `main.rs:1663-1678` calls it blocked). Every hop must anchor. |
| **`clear ≤ EPSILON` handling** | worker/`next_hop` recenter; client **prepends a whole waypoint** instead (`MoverLayer.ts:985-987`), lengthening the route; client reseed does neither (`:1067`) | one recenter, inside `next_hop`, applied on every call including `position_at` | **`next_hop`.** Truncating along the original bearing and detouring via the tile origin are different routes; only one can be authoritative. |
| **`base_tic` meaning** | updated at every hop (`main.rs:2980`, `:2997`) but **only conditionally** at the `MOVE_TO` seed (`main.rs:2938-2947`) | precondition: *`base_tic` is the tic at which the pawn was at `from` **and** the current trip was already active.* Worker must stamp `p.base_tic = ctx.now` **unconditionally** in the `MOVE_TO` arm | **New rule, no side to follow — the worker is simply wrong.** A resting pawn's `base_tic` predates its trip by an unbounded amount. The stride cap in §2.3 step 5 bounds the damage; the unconditional stamp removes it. |
| **`Some(vec![])` vs `None`** | collapsed three different ways (D4) | `Step` tri-state; `shared/wasm/src/lib.rs:536-543` must expose the same three states, not an empty `Vec<i32>` | **Explicit.** `MoverLayer.ts:909-910` turning both into `null` is what routes an "already there" pawn into `walkGreedy`'s limit cycle. |
| **Chebyshev vs Euclidean** | `walkGreedy` spends `prog` as Chebyshev steps (`MoverLayer.ts:141-151`); everything else is `hypot` | one Euclidean rule, in `position_at` | **Euclidean.** `chord_len` (`path_eval.rs:405-414`), `hop_tics`, `position_at` and the anchor probe (`MoverLayer.ts:1093-1099`) are already `hypot`. `walkGreedy` and the header at `MoverLayer.ts:130` claiming it "mirrored EXACTLY" are the outliers, and P4 deletes both. |
| **Waypoint anchoring** | code: tile origin, both sides. Docs: "centres" | tile origin; fix `path_eval.rs:132-133`, `:317` and `MoverLayer.ts:123-127` | **Code.** No behavioural change; the comments are the defect. `MoverLayer.ts:123-127` additionally describes `find_path` output while `computePath` calls `findChords` — flatly stale. |

### 2.5 Tests the spec must ship with

Existing tests stay. Add — each pins one of D1-D4:

1. **Saturation (D1).** Open field, `from = (10,10)`, `dest = (40,10)`, `pace = 24`. Sample `position_at` at `hop.tics`, `hop.tics + 1`, `+ 100`, `+ 10_000`. All four must equal `next_hop(…).point` exactly. *This fails on the current module at every sample past the first* — it is the regression the fix exists for.
2. **Recenter symmetry (D2).** A probe where the first sample off the start tile is impathable. `position_at` at `hop.tics` must equal `next_hop`'s recentred landing, not `from`.
3. **Clamped-hop schedule (D3).** A shoreline probe where `0 < clear < 1`. Assert `hop.tics == hop_tics(len·f, pace)` with the clamped `f`, and that sampling at `hop.tics` lands on `hop.point`. Assert `hop.tics >= MIN_HOP_TICS`.
4. **Tri-state (D4).** `from` on the dest tile off-point ⇒ `Arrive`; walled-off dest ⇒ `Blocked`; ordinary ⇒ `Hop`. No two may be representable by the same value.
5. **Stale base (mechanism c).** `base_tic` 5000 tics behind `now`, first chord 20 tiles long, `pace = 24`. The returned point must be within `hop_stride_tiles(24) = 1.333` tiles of `from`. On today's code it returns the full 20-tile leg.
6. **The 20 live landings** already carried by P1 must re-pass unchanged — this spec preserves worker behaviour everywhere except the four named disagreements, and each of those is a case the current worker answers *twice, differently*.