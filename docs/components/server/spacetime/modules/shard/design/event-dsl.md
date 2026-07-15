# The event DSL — actions as a `Vec<u64>` stack program

> ✅ **DECIDED.** An event carries **`actions : Vec<u64>`** — a flat **postfix (RPN) stream
> of 64-bit words**. Each word is either an **operand** (a qualified reference → *push*) or an
> **action** (a verb → *pop its args and execute*). The worker is a **generic stack machine**
> that chews through the words; there is no per-action Rust and no separate operand table.

This is the concrete form of "the event carries a program" ([events.md](../intent/events.md)). The
execution model is the same postfix/stack discipline as [`shared/dsl`](../../../../../../../shared/dsl/src)
(`push`/`pop`, a value stack) — so the event DSL is that VM's **binary (`Vec<u64>`)
sibling**, fed words instead of parsed `.rd` text.

---

## The word — one `u64`, tagged by op_code

Every `u64` has the same frame: a small **`op_code` tag**, a `server_reference` (which
shard/server the word lives on or was issued from), and a `u32` **payload**. The `op_code`
tells the worker how to read the payload and what to do with the word.

> **Named `op_code`, not `kind`** — deliberately, so it never reads as the object model's
> `kind_reference`/`kind_id`. Each word *is* a stack instruction, so its tag is the
> instruction's opcode: push-a-literal / push-an-object-ref / do-an-action / push-an-alias.

```
u64 word    (high → low)
┌──────────┬──────────────┬───────────────────┬────────────────────────────────────┐
│op_code:4 │ reserved:12  │ server_reference:16│             payload : 32           │
└──────────┴──────────────┴───────────────────┴────────────────────────────────────┘
  bits 60–63   48–59         32–47   (ref/verb    bits 0–31
  the opcode   spare          words only)          object_reference | action_reference
                                                   | literal value | prior-action index
```

**`op_code` + reserved sit on top so the low 48 bits are a clean qualified reference.**
`payload = word & 0xFFFF_FFFF` and `server_reference = (word >> 32) & 0xFFFF` — no need to mask
around the tag. The bottom `server_reference:16 | reference:32` is exactly the "server + u32
ref" target pair from object-model.md §6, unchanged whether or not the top 16 bits are present.

### `op_code` values ✏️ (proposed — confirm)

| `op_code` | the word is | payload | worker does |
|-----------|-------------|---------|-------------|
| `LITERAL` | a constant | an immediate `u32` value | **push the value** (not dereferenced) |
| `OBJECT`  | an operand | a `u32` object_reference (cold/hot), qualified by `server_reference` | **push the reference** |
| `ACTION`  | a verb | a `u32` action_reference (`MOVE`/`INSPECT`/…), issuer in `server_reference` | **pop its args + execute** |
| `ALIAS`   | an **`event_reference`** (a row) | the `event_reference` (`payload:32`); its shard in `server_reference` | **push it** (for `AWAIT` to test completion) |

(The specific verb — `MOVE` vs `INSPECT` — lives in an `ACTION` word's `action_reference`
payload; the `op_code` only says "this word is an action.")

- **`LITERAL` is how constants ride the uniform shape.** `10 wait` → `[LITERAL 10]`
  `[ACTION wait]` → push 10; `wait` pops it. A literal doesn't need a server, so its
  `server_reference` bits are spare (could extend a literal to a 48-bit immediate later; 32
  is plenty for tic counts, ids, counts).
- **`OBJECT` carries its `server_reference`** → cross-shard addressing is free (the worker
  reads the operand from the shard the word names, no routing list).
- **`ALIAS` carries an `event_reference`** (a row) for `AWAIT` to test. An `event_reference` is
  per-row and **minted when the row is written**, so it isn't known at compose time — an
  **alias is a stand-in** the shard **populates with the real `event_reference` as it writes
  the row** (a plan submitted together is written in one go, so intra-plan awaits resolve to
  concrete `event_reference`s then). The word's `server_reference` addresses the row's shard, so
  a **cross-shard `await`** just reads that shard's row. An `event_reference` is **first-class**:
  an action that returns one can push it directly, no alias needed.
- **This subsumes the old "operand vs action" discriminator** — that binary question is just
  three of the four `op_code` values.

The RPN grammar stays self-delimiting: a verb (`ACTION`) knows its arity, so it pops exactly
its operands; `LITERAL`/`OBJECT`/`ALIAS` all just push.

---

## The worked example — `MOVE`

Move object → tile. Three words, in stream order **`tile, object, MOVE`**:

| # | word | `server_reference` | `u32` reference |
|---|------|--------------------|-----------------|
| 1 | tile (destination) | the tile's shard | `object_reference` (the tile) |
| 2 | object (pawn) | the object's shard | `object_reference` (the pawn) |
| 3 | `MOVE` | the issuer of the command | `action_reference` (MOVE) |

The worker just runs the stack:

```
push            ; word 1 → stack: [tile]
push            ; word 2 → stack: [tile, object]
obj  = pop      ; word 3 is MOVE → pop object   (last pushed)
dest = pop      ;                   pop tile
move(obj, dest)
```

Hooray — no verb-specific branch, no operand table, no aliases in the wire form. Operands
push; the verb pops in LIFO order (`obj` then `dest`) and executes. Adding a new verb is a new
`action_reference` + its stack effect, not new engine code.

---

## Multi-step plans — one action string per row, chained by `await`

A **plan** is not one big program. It's **several event rows, one action string each**, each
carrying an **alias**, chained by **`await` on a prior row's alias**. "Open the crate" is *two*
rows — the unpack is **not** a step, because targeting the crate by its `cold_reference` mints it
during enqueue (the middle `MINT` row and the execute-time `GET` both vanish — see
[hot-cold.md](../intent/hot-cold.md)):

```
c AS   b a MOVE ;                                 ← row c: walk pawn a to tile b
f AS   TIMEOUT c AWAIT ? d a INSPECT : f FAIL ;   ← row f: await c, inspect crate d (a cold_reference —
                                                    enqueue mints it hot, so INSPECT gets a hot target)
```

The ordering model (all defined by the user, none of it a "phase"):

- **`alias` = a stand-in for an `event_reference`.** A row's `event_reference` is minted when
  the shard writes it, so it's unknown at compose time; the alias lets a plan name "the row I'm
  about to write," and the shard **populates it with the real `event_reference` at write**.
  `await` ultimately operates on an `event_reference` (whether from a populated alias or one an
  action returned).
- **`await` *defers*, it does not block.** Row `e` awaits row `c`. When the worker reaches `e`
  and `c` isn't complete, it **checks the timeout**: past it → run the `FAIL` branch; not past
  it → **defer** — leave `e` pending, move on to other events, and revisit on a later pass. The
  worker never spins/blocks waiting (that would stall every other row). A row is responsible for
  its own deferral, via `await` + `TIMEOUT`.
- **Rows are queued at tics** (the `event_tic = master_tic + 3` gap; the recoverable enqueue
  phase widened it from +2 — [lifecycle.md](../intent/lifecycle.md)). A worker resolving a
  row can **trigger new rows** (also queued at tics) — e.g. keyed off an alias.
- **Execution is in tic order. Order is *not* guaranteed within a tic.** If two rows must
  order, the later one `await`s the earlier (a cross-tic dependency). Don't rely on intra-tic
  order; use `await` + `TIMEOUT`. (Verbs already have to cope with arbitrary intra-tic order —
  it's the same discipline everywhere.)
- **A row is atomic.** The worker computes *all* the row's target effects in scratch and commits
  them in **one `resolve`** — so `a b MOVE ; c d MOVE` in one row moves `b` and `d` together, not
  one before the other. (Same-shard is one ST transaction; cross-shard is **convergent** —
  idempotent re-drive, not atomic — [lifecycle.md](../intent/lifecycle.md).)
- **`await`'s required `TIMEOUT` guarantees termination.** Every `await` has a tic timeout, so a
  deferred row always reaches a terminal state (runs or fails) — which is what releases its
  holds so GC can reclaim (see [lifecycle.md](../intent/lifecycle.md) §GC). A dependency that vanishes just
  times the dependent out; no cascade.

The verbs used above:

- **`AWAIT` / `TIMEOUT`** — **defer** this row until an `ALIAS` (`event_reference`) completes,
  `LITERAL` tic budget; past budget → the `FAIL` branch. Never blocks (parks + revisits).
- **`? :` / `FAIL`** — conditional continuation and abort (`FAIL` is an `ACTION`).
- **`INSPECT`** (and other verbs) — take a **target ref**; if that ref is a `cold_reference`,
  enqueue has already minted it hot by the time the verb runs, so the verb only ever sees a
  hot target.

> **No `MINT`/`GET` verbs.** Promoting a cold *target* is absorbed into enqueue's `find-or-mint`
> by location ([hot-cold.md](../intent/hot-cold.md)), so there's no `unpack`/`mint_hot` action and no
> execute-time `GET` rebind. `PACK` (settle hot→cold) and `SPAWN` (mint from nothing) remain
> distinct; explicit mint of a *non-target* object is the only case that would need its own verb.

✅ The **branch** encoding (D1): a **forward-only `SKIP`** action word carrying a `LITERAL`
word-count — `AWAIT` pushes a boolean, `?` skips the else-run when true / the then-run when
false. No backward jumps → bounded execution. The stack model, constants, operands, verbs,
back-refs, and branch are all settled.

> **No "phase."** Resolution ordering comes entirely from *tics + `await` on aliases* — there
> is no inbound/data/outbound phase band. (The current `shared/tick/domain.rs` `Phase` is
> legacy that the DSL model retires; the migration receive/transfer/ack ordering it encoded
> becomes explicit `await` chains.)

---

## The event row, restated

**Abbreviated — the DSL-relevant columns only.** The full row is
[tables.md](tables.md) + [lifecycle.md](../intent/lifecycle.md); don't read this as the
complete schema.

```
event
  event_reference : u32        a u32 so it can itself be passed as a reference
  tic             : u32
  actions         : Vec<u64>   the RPN word stream — operands + verbs, self-qualified
  worker_reference: u16        the worker assigned the job
  status          : u8
  ...                          + `targets`, `failed`, `tic_state_change` — see tables.md
```

> **`targets` is not a violation of "it's all in the stream".** The row also carries
> `targets : Vec<u64>` — the **issuer-designated** entities the row writes, which `bump`/`stand_up`
> read **without interpreting the program**. That's deliberate: enqueue must know the write set to
> stand up rows + take holds *before* anything executes, and making the scheduler run the
> interpreter to discover it would put game semantics in the spine. What the stream owns is the
> *program*; what `targets` owns is the *write set*.

No `action:u16` column, no `object_reference`/`server_reference` columns, no operand table —
it's all in the `actions` stream, each word carrying its own `server_reference`.

---

## What this buys

- ✅ **The worker is a generic stack machine.** Push operands, pop-and-run verbs — the
  *scheduling* never grows a branch for a new verb. "Workers become generic just chewing through
  the DSL passed."
- ✅ **Verb effects live in `shared`.** A verb's actual rule (movement, damage, …) is a function
  in a **shared crate** the **worker** (execute), the **edge** (validate/compose), and the
  **client** (predict) all call. One implementation → client prediction matches server execution
  by construction, and it stays deterministic (a hard requirement — [risks.md](../intent/risks.md) R2).
  Adding a verb adds a shared rule, not engine/scheduling code.
- ✅ **Cross-shard is built in.** Every operand word carries its `server_reference`, so the
  interpreter reads each reference from the right shard with no separate routing list.
- ✅ **Plans are first-class.** `AWAIT`/`TIMEOUT`/`? :`/`FAIL` let one event express a
  multi-step, control-flowed plan with failure handling.
- ✅ **Cold targets auto-promote** — enqueue's `find-or-mint` makes a `cold_reference` target
  hot before execute; no `MINT`/`GET` verb, no rebind step ([hot-cold.md](../intent/hot-cold.md)).
- ✅ **Uniform word shape** — every `u64` is `op_code:4 | reserved:12 | server_reference:16 |
  payload:32`; the low 48 bits are a clean `server + u32 ref` pair (plain-mask extraction, no
  tag-stripping).
- ✅ **Constants ride the same shape** — a `LITERAL` word carries an immediate in its payload.

## Settled by the `op_code` tag

- ✅ **Operand vs action discriminator** → the `op_code` field (`LITERAL`/`OBJECT`/`ACTION`/`ALIAS`).
- ✅ **Constants** → the `LITERAL` op_code.
- ✅ **Referring to a prior action** → the `ALIAS` op_code.
- ✅ **The reserved bits** → 4 now hold `op_code`; 12 remain spare.
- ✅ **Branch (`?:`)** → a forward-only `SKIP` action word + `LITERAL` count (D1).

## Still open

- ❓ **The `op_code` values / exact field widths** — the 4-value set above is proposed; confirm
  (and whether more are needed, e.g. a distinct `COLD`/`HOT` operand split vs letting the verb
  decide).
- ❓ **Determinism & termination** — deterministic ops, bounded execution (the forward-only
  `SKIP` gives it), `TIMEOUT` measured in **tics**, no wall-clock / ambient reads.

## Not reusing `shared/dsl` (decided)

`shared/dsl` is a **text-parsed (`.rd`) VM for tile *content/visuals*** — it builds a `Cell`
tree (maps/arrays/colors) of render prims via `@on_create`/`@on_update` hooks. Different
syntax (source text, not `u64` words), different value model (visual `Cell`s, not game
references/state), different purpose (paint prims, not resolve entity state). The only overlap
is the *abstract* "postfix stack + op-dispatch loop" shape, which is a few lines, not a
library. So the event VM is **purpose-built** in the worker; reuse `shared/dsl` only if a
genuine shared primitive later falls out — don't design around it.
