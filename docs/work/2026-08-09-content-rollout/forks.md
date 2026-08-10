# Forks — content-rollout

_Decision points, the options, and which we chose. Context in [`README.md`](README.md); anticipated
problems in [`issues.md`](issues.md)._

## F1 — three lanes, and the corpus lane comes first {#f1}

_2026-08-09._ The user named two lanes (active, passive). The survey found a third underneath both,
and it is the one that actually broke: **the corpus lane** — every process holding the same TOML.

Passive rollout cannot work while the npc holds a boot-time corpus, because the replacement bunny is
minted from the old def. Active rollout cannot work while the worker holds a boot-time corpus,
because the worker is what composes the re-stamp. **So the ordering is forced, not preferred:**
corpus → passive → active. Phases follow it exactly ([`todo.md`](todo.md)).

Rejected: **building the active verb first** because it is the interesting part. It would have been
tested against a worker holding stale content and would have appeared to work or appeared to fail
for reasons unrelated to the verb.

## F2 — one corpus source: the edge's `GET /content` {#f2}

_2026-08-09._ Worker, master and npc all fetch from the edge, adopting the browser's poll-and-swap
verbatim. The worker's `CONTENT_DIR` disk read retires.

**Chosen because the edge is already the reconciliation point.** It is the only component that knows
whether the corpus lives on a bind-mounted `content/` or in R2, the only one holding S3 creds, and
the only one that strips server-only definitions *by what the data is* rather than by filename
(content-packages F2). Every additional disk reader is a second answer to "what is the corpus", and
[`content.rs`](../../../server/edge/src/content.rs)'s own header records that a private copy of the
content walk has already gone stale here once (content-toml-only I5). One reader, one set of rules.

It also makes the fingerprint meaningful everywhere: `GET /content-version` is a 16-hex string, so
"is my corpus current" is one cheap request for every process, not five different answers.

Rejected: **the worker keeps its disk mount and polls the directory's own mtimes** — cheaper to
write and it works in dev, but it is exactly the second reader above, it cannot work deployed (the
worker has no bucket creds), and it makes the worker's corpus differ from the client's in a way
nothing would surface. Rejected: **the master pushes the corpus to workers** — invents a
distribution channel where an HTTP GET already exists.

Conditional on [I1](issues.md#i1): the worker must be confirmed not to read `[[biome]]` blocks,
which `/content` withholds. If it does, the edge grows an authenticated server-side variant rather
than the worker regaining a disk reader.

## F3 — the swap is between passes, never inside one {#f3}

_2026-08-09._ The worker holds `Arc<Bundle>`; the poll task stores a new `Arc` and the compose loop
takes a clone **once at the top of each pass**. One tic composes against one corpus.

**Forced by the shape of a pass.** A single pass resolves an interaction's signature, gates on
affordances, evaluates needs, derives speed and composes sidecars — all against the bundle. A swap
between any two of those steps could gate on the old corpus and mint from the new one, which is a
class of bug with no symptom other than a wrong world.

The same rule holds in the npc (swap between brain ticks) and is already true in the browser (the
swap is a single `swapTo` with listeners fired after).

## F4 — the master re-seeds the registry on every corpus swap {#f4}

_2026-08-09._ The seed block at [`master/main.rs:115`](../../../server/master/src/main.rs) moves
from boot-only to boot-plus-on-swap.

**Costless, because `ensure_definition` was written for exactly this**: it no-ops on a row it
already holds and errors loudly only on a genuine collision. Its doc comment already says "every
server may call it on every boot without coordination". Re-seeding on a swap adds no new failure
mode; it removes the window in which a newly authored def or version has no number.

Non-fatal stays non-fatal: a failed re-seed logs and the metronome keeps running, exactly as the
boot path does today. Every stored id still decodes as before — the read path never consults the
registry (registry I6).

## F5 — a version bump is NOT required to roll out; one verb covers both {#f5}

_2026-08-09._ `RESTAMP_DEF` re-resolves the entity's *taxonomy* against the current corpus, takes
`max(version)`, rebinds `definition_reference` if it moved, and re-derives the sidecars from
whichever def it landed on.

**Chosen because the two cases are the same operation with a rebind that is sometimes a no-op**, and
because the case that actually prompted the stream is the in-place one. The user described a bump
("version 1 to version 2"), but `bunny` in [`things.toml:315`](../../../content/things.toml) carries
no `version` key — it is v0, edited in place. Per registry F12 that edit *should* have bumped, and
demanding the bump as a precondition for rollout would mean the tool cannot fix the exact situation
it was asked to fix.

Consequence, stated: with in-place edits the id cannot answer "is this entity stale". That is what
[F8](#f8) is for.

Rejected: **require a bump, then rollout is a pure id rebind** — conceptually cleanest, zero new
detection machinery, and it is where this graduates when data stops being in flux. Refused now
because it burns a `kind_id` and a permanent retired `[[thing]]` block per edit ([I3](issues.md#i3)),
at an edit cadence of several per day. Rejected: **stamp each entity with the minting corpus
fingerprint** — answers in-place staleness precisely, at the cost of new per-entity storage on the
hottest table in the world for a question only an operator asks; [F8](#f8) answers it per *def*
instead, where there are hundreds of rows rather than thousands.

## F6 — `RESTAMP_DEF` is per-entity; `ROLLOUT_DEF` fans out and writes nothing {#f6}

_2026-08-09._ Two verbs, next free numbers after `ACTIVATE_TRAIT` = 19
([`action.rs:145`](../../../shared/codec/src/action.rs)):

- **`RESTAMP_DEF` = 20**, arity 2 — `obj:entity_reference` (**write**) · `def:definition_reference`
  (imm, the def to land on). The write set is exactly one entity, so it groups with that pawn's own
  movement writes and nothing else.
- **`ROLLOUT_DEF` = 21**, arity 3 — `from:definition_reference` (imm) · `to:definition_reference`
  (imm) · `scope` (imm, reserved 0 = every shard the worker mirrors). **Writes nothing.** The worker
  enumerates live entities on `from` and queues one `PROMOTE RESTAMP_DEF` each.

**Forced by [`ACTIVATE_TRAIT`](../../ACTIONS.md)'s write-set law**, which the palette states in
capitals: events sharing a written target merge transitively into one conflict-component, and a
component runs on one worker. A single verb that wrote every bunny would weld the entire warren —
and anything any of them is interacting with — into one component on one core. The fan-out shape is
already the house pattern (`BUILD_WALL` expands a perimeter into per-tile `SET`s; `MOVE_TO` chains
`MOVE_STEP`s), and it means a rollout of 200 bunnies is 200 independent single-entity components.

`from` is carried rather than derived because the re-stamp needs **both** defs to diff the trait
binds ([F7](#f7)) — what the old def authored and the new one doesn't must come out.

Rejected: **one verb with an entity list operand** — a variable-arity write set, which is the
grouping hazard above wearing a different hat. Rejected: **a reducer on the pawn module** — the
module holds no corpus (stat-model I12) and could not compute what to write.

## F7 — the re-stamp CONSERVES values and RE-DERIVES shape {#f7}

_2026-08-09._ Per entity, `RESTAMP_DEF` does exactly this and nothing else:

| what | policy |
|---|---|
| `entity_state.definition_reference` | set to `def` (no-op when unchanged) |
| stored trait rows | add the new def's non-constant binds; remove the old def's; **leave anything neither def authored** |
| `PART` payload entries | re-mint slots from the new def's part count, **preserving the spawn-chosen variant nibbles**; drop slots the new def doesn't declare ([I5](issues.md#i5)) |
| need rows | keep the current value; re-clamp to the new effective bounds; mint newly authored needs at effective max; drop removed ones |
| conditions | untouched |
| position, facing, trip serial, inventory | untouched |
| intent queue | untouched — the worker's ephemeral map; completions already re-validate |

**The principle is that nothing a player could read as a reset gets reset.** A rolled-out bunny is
the same bunny, mid-life, with a new capability — not a fresh spawn wearing its position. Registry
F6 rejected a sweep precisely because per-field rules have no good general answer ("new apples
expire in 1 h and old ones in 2 h, so what happens to an apple with 30 minutes left"). This table
is not that answer: it is the narrow early-dev subset where the question doesn't arise, because the
only fields it re-derives are *set membership* (which traits, which needs) and the only field it
touches with a value is a clamp into a range the value must be in anyway.

"Leave anything neither def authored" is what keeps `ACTIVATE_TRAIT`'s runtime-acquired rows alive
through a rollout, and it is why `from` is an operand.

Rejected: **re-mint the sidecars wholesale** (one line of code; resets every need to full and every
rolled-out pawn is silently healed and fed — the reset the user is trying to avoid). Rejected:
**preserve stored rows untouched and only add** (a trait removed from the TOML would never leave any
existing entity, so the corpus stops being the truth about what a bunny is).

## F8 — staleness is REPORTED, not refused {#f8}

_2026-08-09._ The `definitions` row gains `sim: u64`, the corpus's own simulation-visible hash from
[`thing_sim_version`](../../../shared/content/src/loader.rs) (`loader.rs:1050`), which already exists
and has no consumer. On re-seed, a tuple whose recorded `sim` differs from the corpus's at the same
`version` is **logged with its live entity count** and surfaced by `rd rollout --status`.
`ensure_definition` keeps accepting it.

**Report-not-refuse is the whole decision.** Refusing would make registry F12's law mechanical: an
in-place simulation-visible edit would fail master boot until the author writes `version = 1`. That
is correct, and it is correct *later*. Today the user edits defs several times an hour and has said
data is in flux; a check that halts the metronome on an unbumped edit converts a silent failure into
a blocking one, which is not obviously an improvement. Logging it converts the silent failure into a
**sentence naming the def and the number of entities affected**, which is what was missing when the
forager trait vanished.

The escape hatch stays what it is: `rd redeploy` with a reset wipes, and always did.

Rejected: **refuse at `ensure_definition`** (above — the graduation path, gated on data settling).
Rejected: **derive the version from the fingerprint** — already settled against at registry F12:
only the author knows whether a change is simulation-visible, and a fingerprint cannot produce two
coexisting versions, which is the point of authoring them.

## F9 — the passive lane gets a counter, and the brain must re-resolve {#f9}

_2026-08-09._ Two parts, both small, both required for "wait for all of the bunnies to die and be
replaced" to be a real strategy rather than a hope.

**The brain re-resolves on a corpus swap.** [`bunnies.rs:121`](../../../client/npc/src/brains/bunnies.rs)
caches `self.def`, `self.kind` and `self.bundle` when the corpus first loads, and the replenish guard
mints from `self.def` forever after. Without this the passive lane **never terminates**: every
replacement is another old-version bunny. This is the sharpest single finding in the survey and it
is why passive rollout appeared not to work either.

**`rd rollout --status <kind>` reports the drain.** Live entity counts grouped by `definition_reference`
for the kind's taxonomy, with the newest version marked and stale rows flagged by [F8](#f8)'s `sim`
comparison. Passive rollout is then an observable number going to zero, which is also how you know
an active rollout finished.

Rejected: **poll SQL by hand each time** — it is what we do now and it is why nobody knows the
population's version split.

## F10 — the operator lane is `rd rollout`; neither verb is client-open {#f10}

_2026-08-09._ `ROLLOUT_DEF` and `RESTAMP_DEF` stay out of `CLIENT_VERBS`
([`edge/src/ws.rs:634`](../../../server/edge/src/ws.rs)). The issuing surface is a new `rd rollout`
subcommand that queues the event through the SDK, beside `rd content-check` and `rd redeploy`.

**Chosen because rollout is an operator action against the world, not a gameplay action within it.**
`SPAWN_REQUEST` is client-open only because it is gated by validation the worker performs for a
*player*; there is no equivalent gate for "rewrite every bunny", and there should not be one. It
also belongs where the content workflow already lives: the same CLI that publishes the corpus should
be the one that rolls it out.

Rejected: **an npc drill** (`NPC_ROLLOUT=bunny`, the `NPC_GRANT` pattern) — the cheapest lane and
genuinely tempting, but it couples an operator action to a bot process that must be restarted to
carry it, and a rollout is a thing you want to run *without* restarting anything. Rejected: **a
debug entry in the client's pie menu** — puts a world-rewriting verb one misclick from a player and
would need `CLIENT_VERBS`.
