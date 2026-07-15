# Forks — pathfinding (decisions, options, choice, why)

Decision points with more than one viable path. Chronological. The design is
[`docs/intent/pathfinding/`](../../intent/pathfinding/README.md); this file records what we chose
while planning it against the code as it actually is (2026-07-15).

---

## D1 · Where does `duration_tics` come from? — **RECOMMEND: an input for v1**

- **Design** ([§3](../../intent/pathfinding/README.md)): `duration_tics` comes from a
  *deterministic speed model* — tiles-per-tic × terrain cost × diagonal cost, integer, in `shared`.
- **Your spec:** *"specify an end tile and number of tics it should take to get there."*
- **These do not conflict** — the move-intent row carries `duration_tics` either way; the only
  question is **who computes it**. v1 takes it as a `LITERAL` word on the `MOVE` program; the speed
  model later becomes the thing that *produces* that number, and the row shape never changes.
- **Why input first:** it takes the speed model **off the determinism surface** for v1. Worker and
  client must still agree on *which tiles, in what order* — but not on cost arithmetic. That's a
  strictly smaller cross-target risk (§6) to prove the loop with.
- ⚠️ **Consequence to accept:** a caller-supplied duration can be *wrong* (a 2-tic sprint across 40
  tiles). v1 does not validate it against terrain. Fine for npc/debug; **not** fine once players
  issue moves — validation lands with the speed model.

## D2 · Where does the move-intent live? — **RECOMMEND: payload fields, not a new table**

You expected new tables. For the **intent row** I think the payload is the better home, and the
reason is plumbing, not taste:

- **(a) New payload fields** on the shard's `decl_tick_pipeline! { payload: {...} }` — the pipeline
  is **payload-generic**, so this is a declaration change, not engine work. Decisive advantage: the
  edge **already relays `state` rows** to clients, so motion reaches pixijs *for free* — no new
  subscription, protocol row, relay, or client plumbing.
- **(b) A new `motion` table** keyed by `entity_key` — needs its own subscription + wire row +
  relay + client decode, and re-introduces the "two sources of truth for one object" problem
  (`state` says where it is; `motion` says where it's going). It also splits the atomic `resolve`:
  today a row commits **all** its target effects in one call.
- **Why not `data0`/`data1`** (the tempting zero-schema option): **they're already overloaded** —
  `data[0]` is `hp` for pawns (`domain::hp`) *and* cold provenance for find-or-mint objects
  (`worker: type_reference = data_0 >> 32`). A third meaning keyed on `kind` is how D-3 happened.
- **Cost of (a):** every payload field is on *every* state row, including the 99% that aren't
  moving. ~16 bytes/row. Accept for now; revisit if row size bites.

## D3 · How does the far-end `place` get scheduled? — **RECOMMEND: `append_at` + the event log**

The design (§7) worries we'd need a sparse "idle until tic T, then fire". **Verified against the
code — the machinery already exists and it's safe:**

- **(a) `append_at(tic, …)` + the existing `event_log`.** `append()` today hardcodes
  `event_tic = master_tic + TIC_GAP(3)` — that's the **only** blocker; the worker *already* gates
  execution on `master_tic >= event_tic`, so a future row simply waits.
  **The read rule makes this safe, which is the non-obvious part:** a pending write at `T+50` does
  **not** block reads at `T` — resolution is "no pending row **at or below** the read tic". So an
  in-flight object stays **readable** for the whole flight and only defers readers at/after its
  arrival tic, which is exactly correct. Holders keep the pending row from GC. No new table.
- **(b) A `wake` table** (`wake_tic` btree → entity) — genuinely sparse, and the general mechanism
  (also serves growth, timeouts, decay). More machinery than v1 needs.
- **(c) An arrival sweep** — reuse `enqueue_pack_sweep`'s proven shape. No schema change, but walks
  every in-flight object every pass, which is precisely what §7 says to avoid.
- **Honest caveat on (a):** the worker's `work_pass` iterates the whole `event_log` each pass, so a
  future row is *walked* every pass even though it isn't *run* — O(in-flight) per pass, the cost §7
  warned about. It's a tic comparison, and the PACK sweep already walks every hot state row, so it's
  not a new class of cost. **(b) is the upgrade if it ever bites** — and it's a pure add, no rework.

## D4 · Obstacle scope for v1 — **static grid only** (design §9.4 already recommends this)
Moving-obstacle awareness (§5) makes the pathfinder's input "the grid **plus every active intent
projected forward**". Ship the loop first.

## D5 · Supersession of a revoked future `place` — **OPEN, and the hard one**
Design §5/§9.3. A scheduled `place` is speculative: a new `MOVE` mid-flight, or a block, must stop
it materialising a position the object was diverted from. Needed at P6, not before — but it is the
piece most likely to invalidate the shape above, so **do not defer thinking about it, only building
it.** Candidate: the interrupting write `abort`s the scheduled row by `event_reference` (the
lifecycle already has `abort` + `release_holds`); the intent row holds its scheduled
`event_reference` so it can be found. To be confirmed against the bitemporal semantics.

## D6 · Segment length policy — **OPEN, defer to P7** (design §9.2)
Fixed tile budget / fixed tic budget / distance-to-first-obstacle. Meaningless until a real
pathfinder exists; the row shape is identical for one segment or many.
