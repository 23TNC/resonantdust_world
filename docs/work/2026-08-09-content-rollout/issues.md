# Issues — content-rollout

_Problems hit, and problems anticipated from the survey, with the candidate solutions and which we
chose. Opened 2026-08-09 as an anticipated-issue inventory; rows gain outcomes as the stream runs.
Decisions live in [`forks.md`](forks.md)._

## I1 — does the worker need the server-only `[[biome]]` blocks? {#i1}

_2026-08-09, open — a VERIFY item, not a guess._

[F2](forks.md#f2) repoints the worker at the edge's `GET /content`, which withholds `[[biome]]`
blocks and any source left empty by that stripping (`strip_server_only`, content-packages F2).
Worldgen lives in the edge, so the worker should not need them — but "should" is how a private
content reader went stale here before.

**Check before building:** grep the worker for biome reads, and load the served corpus in a worker
test to confirm every accessor it calls still resolves. If it *does* need them, the fix is an
authenticated server-side corpus endpoint on the edge, **not** giving the worker a disk reader back
— one reader, one set of rules.

## I2 — a corpus swap mid-flight {#i2}

_2026-08-09, open._

[F3](forks.md#f3) makes each pass atomic against one bundle, but an event **queued** under corpus A
can **complete** under corpus B: an interaction whose signature changed, an affordance that no longer
passes, a queued completion for an act the def no longer offers.

The expected degradation is already benign by construction — the intent queue's completions
**re-validate** at fire ([lumberjack](../2026-08-07-lumberjack/README.md)), so a def that stopped
offering an act yields a refused act rather than a wrong one, and the crossing scheduler's re-stamps
"write the HONEST current value" by design ([`ACTIONS.md`](../../ACTIONS.md) `RESTAMP_NEED`).

**Verify rather than assume**: swap the corpus while a pawn is mid-walk-then-act and confirm the log
shows a refusal or a clean completion, never a half-executed effect.

## I3 — an authored version bump costs a kind_id and a permanent TOML block {#i3}

_2026-08-09, open — a constraint to state, not a bug to fix._

Registry F5 burns a fresh `kind_id` per version, and F17 requires the old `[[thing]]` block to stay
authored for as long as the world runs an entity on it. Kind ids are declaration-ordered and
append-only (the ORDER IS LAW comment at [`interactions.toml:110`](../../../content/interactions.toml)),
so authoring `bunny` v1 means **appending a second `[[thing]]` block at the end of the file** and
keeping the v0 one indefinitely.

At the current edit cadence that is a growing pile of retired blocks with no reclaim
([registry F7](../2026-08-04-definition-registry/forks.md#f7), deliberately unbuilt). Which is the
concrete argument behind [F5](forks.md#f5): **in-place edit plus active rollout is the everyday
loop**; authored versions are for when two versions genuinely need to coexist in one world.

Worth a line in the content authoring docs so the choice is made knowingly.

## I4 — a rollout can kill {#i4}

_2026-08-09, open._

[F7](forks.md#f7) re-clamps need values into the new effective bounds and writes them through the
normal path, which means they feed the **need-write sweep** (food-chain) — and the sweep kills at
`corpus <= 0`. A def that lowers the leveled `corpus` cap, or drops the `corpus` trait so the
effective max collapses, will honestly drop pawns to a lethal value and the sweep will fire.

That is **correct** — the alternative is a pawn whose stored health exceeds its own maximum — but it
must be expected rather than discovered. Mitigation is disclosure, not suppression: `rd rollout`
reports the per-need clamp deltas it is about to apply, and `--dry-run` prints them without writing.

Do **not** special-case survival needs out of the clamp. A rollout that silently leaves a value
above its ceiling reintroduces exactly the corpus-vs-storage disagreement this stream exists to
close.

## I5 — PART entries when the part count changes {#i5}

_2026-08-09, open._

`mint_parts` ([`worker/main.rs:108`](../../../server/worker/src/main.rs)) writes one `PART` payload
entry per slot for a kind declaring ≥2 parts, substituting the spawn request's variant nibbles;
single-part kinds carry the variant on the def itself. A def that goes 1 → 2 parts (or 2 → 1) leaves
existing entities with a payload the renderer will read against the wrong slot count.

**Policy** ([F7](forks.md#f7)): re-mint the slots from the new def's part count, **preserving the
existing nibbles** slot-for-slot where both defs have that slot — the nibble is a placement choice
(this human's head, that wolf's coat), not a def property, and re-rolling it would visibly change
who the pawn *is*. New slots take nibble 0; removed slots drop.

Watch the single-part boundary specifically: crossing 1 ↔ 2 moves the variant between the def ref
and the payload, so it is the one case where preserving "the same nibble" means moving it.

## I6 — an entity whose def no longer exists in the corpus {#i6}

_2026-08-09, open._

A deleted `[[thing]]` leaves live entities pointing at a taxonomy the corpus cannot resolve. The
rollout must **refuse loudly and leave the entity alone** — never re-stamp onto a zero def, never
delete the entity.

This matches the posture already taken elsewhere for retired ids: pathability degrades a retired id
to OPEN rather than freezing ([`loader.rs:1307`](../../../shared/content/src/loader.rs)), on the
principle that a corpus/state skew must degrade, not trap. Same principle, opposite direction: here
the safe degradation is inaction.

`rd rollout --status` should count these separately — "3 live entities on a def the corpus no longer
authors" is exactly the sort of thing that should not be a surprise.

## I7 — `payload` is SLAVED; the re-stamp must ride a state write {#i7}

_2026-08-09, open._

The pawn module's `payload` and `needs` tables are never claimed
([`pawn/lib.rs:29`](../../../server/spacetime/server/modules/pawn/src/lib.rs)): "the entity's state
claim is the lock, and every write here rides a state-write transaction". `RESTAMP_DEF` writes the
entity, so it *holds* the claim — but the implementation must go through a state-write reducer and
not poke the sidecars directly, or it breaks the module's stated invariant in the one place where
the invariant is easiest to break (the def ref lives on `entity_state`, the traits on `payload`, and
this verb touches both).

The `spawn` reducer is the existing precedent for writing all three in one transaction.

## I8 — re-seed before rebind: the mirror ordering {#i8}

_2026-08-09, open._

The worker resolves ids through its `SELECT * FROM definitions` mirror. A rollout to a **newly
authored version** requires: corpus swap → master re-seeds → the worker's mirror applies the new row
→ rollout runs. Out of order, the worker rebinds an entity to an id the registry has not recorded,
which is precisely the aliasing the registry exists to prevent.

The browser already treats this ordering as load-bearing and says so in a comment
([`contentBoot.ts:122`](../../../client/webgl/src/game/definitions/contentBoot.ts): "ORDER MATTERS
… the registry loads BEFORE the corpus swaps in"). Adopt the same rule server-side.

**Mechanism**: `rd rollout` resolves `from`/`to` against `/definitions` *itself* and refuses if the
target id is absent — so the ordering is checked at the door rather than assumed by every caller.
`--wait` polls until the re-seed lands.

## I9 — the boundary against `--reset` {#i9}

_2026-08-09, open — a scope line, so nobody expects too much._

`rd redeploy` republishes modules with `--delete-data=always` and that remains the tool for **schema**
changes: a new column, a widened need row, a new table. This stream does not touch it and does not
try to.

What it removes is the need to reach for a wipe on a **content** change. The two are easy to conflate
because today they have the same remedy.

## I10 — enumeration scope, and what breaks at scale {#i10}

_2026-08-09, open._

`ROLLOUT_DEF`'s fan-out scans the worker's mirrors of `pawn` and `player_pawn` — both subscribed
whole ("the shard holds pawns only, so 'all rows' is small by construction",
[`worker/main.rs:593`](../../../server/worker/src/main.rs)). At dev scale a full scan is honest and
cheap, and `scope` is a reserved operand precisely so this can change without a new verb.

Named so it is not discovered later: at real scale this becomes a per-shard fan-out with chunked
batches and a resumable cursor, because one worker enumerating every entity in the world is the same
centralization the write-set law exists to avoid. Not built now; the reserved operand is the hook.
