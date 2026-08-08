# Spawn authority — one spawn request, server-governed; then the teleport hunt

**What** (user, 2026-08-08): sync issues are teleporting pawns. First, UNIFY SPAWNING:
a spawn EVENT the server's components can VALIDATE, VERIFY and IMPLEMENT — the server
is the governing authority; clients may REQUEST spawns, the server decides validity
and executes. The command is a DEBUG tool (likely removed later), but it unifies how
webgl's chat command and the npc both request pawns. The server becomes responsible
for MINTING the pawn and its data ("quite a bit of data logic is happening client
side" — true: `/spawn` packs PART opcode words in the chat handler). With authority
in place, INVESTIGATE the teleporting — very likely desync.

Plan review (user): the request's layout — "u32 event_id (spawn), u16 unit.x u16
unit.y, u4 rotation u4 type u12 subtype u12 kind, vec<u4> variant (one per part)…
we would probably want parts declared in the toml so we know what to expect passed
during the spawn". Pinned (with one refinement, accepted): the ≤4-parts law means
the variant vec fits ONE nibble-packed u32, so the verb stays FIXED arity 3 and the
event shard/orchestrator frame it corpus-free —

    SPAWN_REQUEST [x:16|y:16] [rotation:4|type:4|subtype:12|kind:12] [v0:4|v1:4|…]

The corpus part declarations are the tail's CONTRACT: nibble i = part i's variant;
nonzero nibbles beyond the declared part count REFUSE. Rotation seeds the initial
facing (0–3; 4–15 refused) — spawns stop all facing south.

## The stance

- **One client verb: `SPAWN_REQUEST def position variants`** ([F1](forks.md#f1)) —
  pure request, writes nothing. The WORKER's arm validates (the def is a REGISTERED
  pawn kind — checked against the `definitions` table it already mirrors; the
  position is in-world and PATHABLE) and then queues the real worker-only `CREATE`
  program — the BUILD_WALL/EXECUTE pattern: requests never write, validated
  programs do. `CREATE` LEAVES the client allowlist ([F4](forks.md#f4)).
- **Refuse, never nudge** ([F2](forks.md#f2)): an impathable or unstreamed requested
  position is a logged refusal — the lake-mint class (a wolf stranded mid-lake;
  attack I2) dies SERVER-SIDE for every client at once, instead of each client
  carrying its own pathable-pick.
- **The server composes ALL the data** ([F3](forks.md#f3)): PART entries (body/head
  from the request's variant hints, validated ≤15), TRAIT rows, and need rows all
  compose in the worker from ITS corpus — the chat handler's opcode packing is
  DELETED, the npc's payload argument goes, and `mint_sidecars` grows a
  `mint_parts` sibling. A client that lies about parts simply cannot any more.
- **Both clients become one-line requesters** — webgl `/spawn` and the npc brains
  (wolves + bunnies) send the same three words; their def resolution stays (they
  hold the registry), their pathable-picks demote to POLITENESS (the server is the
  gate — [I2](issues.md#i2)).
- **Then the teleport hunt, evidence-first** ([F5](forks.md#f5) — the bug-sweep I11
  discipline): an anchor-jump probe over the authoritative rows (consecutive
  `entity_state_log` positions jumping farther than a hop stride can carry) +
  client-side render-vs-auth divergence sampling, run under the traffic that
  reproduces (npc wander + player trips + interrupts). Conditions land in
  issues.md with captures; a fix lands only where the cause is proven.

Authoritative docs touched: ACTIONS.md (the SPAWN_REQUEST verb + the authority
law), VARIABLES.md (the verb row; CREATE's client-open note dies).
