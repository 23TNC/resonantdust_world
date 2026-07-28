# first-pawns — a wolf pawn in a `pawn` shard, driven by an npc container

_Work stream, opened 2026-07-28. Components: `server/spacetime` (new `pawn` module),
`server/{master,orchestrator,worker,edge}`, `shared/codec` (`CREATE`), `client/npc` (harness +
brains), `client/webgl` (observes). User's brief: ONE wolf pawn generated in a pawn spacetime
shard, driven by a rust npc container through the whole pipeline — npc → client/core → edge
(validate + queue) → event shard → orchestrator → worker → pawn data shard → promote on tic →
edge subscriptions → fan-out to webgl AND npc. The npc drives movement between tiles, structured
in anticipation of more advanced commands and multiple "brains" (a villager-brain, a wolves-brain,
a region-steward-brain) built on the same npc base._

## What exists, verified 2026-07-28 (build on it, don't re-invent)

- **The pipeline is live end-to-end** (spacetime-again W1–W9 + shard-tables P1–P4 core): edge
  `queue` → orchestrator union-find grouping + per-(shard,tier) `claim` → worker compose →
  `write` with the inline smart `PROMOTE` → `entity_state` → edge per-zone subscription →
  `ServerMsg::State` → client `StateObject`. Wolves have moved over WS through exactly this path.
- **A pawn shard is a one-line stamp**: `data_shard`'s whole module body is
  `resonantdust_codec::entity_tables!(data: u8);` — the shard-tables README lists "move wolves to
  a `pawn` table" as an enabled follow-on, and `TABLES.md`/`world-storage` already write
  "`data_shard` (→ `pawn`)". Module discovery is glob-based (`rd_list_modules`), so
  `modules/pawn/` is auto-picked-up by `rd redeploy`; DB `resonantdust-<env>-pawn-0` (no
  underscore → no name-mapping entry needed).
- **Movement verbs**: `Command::Move` → `[PROMOTE, MOVE_TO, entity, pos]`; the worker's `MOVE_TO`
  arrives in one step ("multi-tile stepping + self-requeue is the movement-content follow-up").
  For ADJACENT-tile hops, one-step semantics is exactly correct — see the design stance.
- **`CREATE` is specified, not built**: `ACTIONS.md` §`CREATE` — mints the entity, spawn-log
  keyed `(event_reference, index) → minted id` on the data shard for replay, conflict-free
  singleton at grouping. The worker's arm is a no-op. Today's wolves are CLIENT-minted
  (`wolf_key(i)` = `0x30000001..`), a documented stopgap that collides with the browser's own
  client-minted `selfEntity()` (`0x30000000 + player_id`) — two npcs would also collide.
- **`PLACE` carries no `definition_reference`** — which is why the live wolves render with the
  TREE def (`MoverLayer` kind fallback → 1). The wolf visual is complete and unused: `::wolf>`
  stem `pawn/animal/wolf`, masters `template.{e,s,n}.0.png`, facing from `data >> 6`
  (`0→s 1→e 2→n 3→e+flipX`). `CREATE` carrying the def is what turns wolves into wolves.
- **npc is an automated player** over `client/core` (gateway login, anchors, same wire as a
  browser — no privileged path): one 266-line `main.rs` with a `Bot` harness explicitly commented
  as the piece to extract when a second behavior lands. Built via `rd build npc`
  (`client/npc/compose.yml`); today it is RUN MANUALLY — no supervised container.
- **`bin/sim`** builds + runs the server sim crates as detached `rd-<crate>` containers
  (`--network host`, repo bind-mount). The sim stack is currently DOWN (containers exited) —
  standing it up is P0's first item, not a blocker.

## Design stance

- **The pawn shard is a NEW shard class, not a rename** (F1): `modules/pawn` stamped from
  `entity_tables!(data: u8)`, `TYPE_PAWN` (3) routed to it at the orchestrator (claim) and worker
  (compose/write) and edge (subscribe/relay). `data_shard` stays as the catch-all hot shard the
  worker's `_ =>` arm lands on; its retirement is a follow-on once nothing routes there.
- **Server-minted pawns** (P1): build `CREATE` per `ACTIONS.md` — spawn-log for replay
  idempotence, minted target never an operand. This kills the client-minted id collision class
  and finally plumbs `definition_reference` so a wolf IS a wolf on every client.
- **The npc drives movement tile-by-tile** (F2, the user's own framing): the wolf brain issues
  an adjacent-tile `MOVE_TO` per hop at its own cadence. The worker's one-step `MOVE_TO` is
  CORRECT for adjacent hops — the self-queueing multi-tile chain + sparse promote cadence
  (`ACTIONS.md` §Movement) stays tabled, to be built when commands get more advanced. We do not
  simplify the documented chain away — we simply don't need it for hop-sized commands.
- **Brains are a library seam, not processes** (P2): `client/npc` splits into a lib (the `Bot`
  harness + a `Brain` trait) and a thin bin that dispatches a named brain. One container = one
  brain instance (`rd-npc-<brain>`). The wolves-brain is the first implementation; a
  villager-brain or region-steward-brain is a new impl of the same trait, not a new harness.
- **Docs first**: every table change lands in `TABLES.md` before the module (`# Changing a
  table`), and subscription SQL is string-typed — every item touching a table re-checks the
  subscriptions naming it.

## Known hazards (accepted, recorded)

- **No ownership model** — any session may move any pawn (documented in `Command::Move`).
  Ownership/validation at the edge is a follow-on with players; out of scope here.
- The browser's own `selfEntity()` stays client-minted this stream — only npc pawns move to
  `CREATE`.
- Movement renders as per-tile snaps: NO client interpolation exists (nothing consumes
  `syncedNowMs() − RENDER_DELAY_MS`; `valid_at` is retired, tic is the clock). Smoothing is a
  follow-on (`client/core/intent/sync.md`), not this stream.

## Follow-ons (each a future stream, deliberately not here)

- `MOVE_TO` self-queueing multi-tile chain + queue-at-future-tic + sparse re-anchor promotes +
  `PROMOTE_EVENT` intent announcements (`ACTIONS.md` §Movement, tabled).
- Client movement interpolation on the tic clock (render delay `D`, projection model).
- Ownership: edge validates who may drive which entity.
- More brains: villager-brain (a village's pawns), region-steward-brain (a region in players'
  absence); a brain fleet supervisor.
- `data_shard` retirement (or its re-purposing) once no type routes to it.
- Sim process resilience is **2026-07-27-sim-self-heal** (open, separate) — its acceptance
  ("wolves resume moving") runs exactly this stream's pipeline.
