# Blockers — spacetime rewrite (need human input)

Open items I can't resolve alone, with enough analysis to bring you up to speed without digging.
Open→resolved; a resolved blocker gets a resolution + date, then archives to the bottom.

---

## B-1 · The object-model taxonomy is in-flux — blocks the representation re-keys

**Status:** PARTIALLY RESOLVED · raised 2026-07-14 · narrowed 2026-07-14

**Resolved 2026-07-14 — the reference model is settled**
([`components/shared/codec/design/reference-model.md`](../../components/shared/codec/design/reference-model.md)):
the **definition / position / data** split + the **identity** side (`object_reference` union +
`entity_reference = reserved:10 | reference_id:6 | server_reference:16 | object_reference:32`), all
`u32`. This closes the taxonomy/naming, `region_zone` (**#4**), **and** the `hot_reference` re-key
(**#5** — a hot object is `object_reference = hot_reference:32`, server-qualified). Implementing
them is now a codec re-cut ([codec plan](../../components/shared/codec/plan/README.md)), not a
decision.

**Still blocked (needs your call).** One decision remains: the **geographic vs functional
`server_reference` internal layout** (**#10** — `realm:8 | server_id:8` vs `server_type:6 |
server_id:10`). `server_reference` is a `u16` field in `entity_reference` + the DSL word regardless,
so this is the last piece of its *interior* that the identity model leaves open.

**Description.** These all re-key or re-encode data on the object-model taxonomy — `kind` / `type`
/ `variant`, the packed `*_reference` u64 layouts, `hot_reference:u32` (decision D3), geographic
vs functional `server_reference`. But the design docs themselves mark this taxonomy **in-flux**:
`docs/object-model.md` carries "4 open decisions," and D3 (the `hot_reference` re-key) is
explicitly "deliberately deferred." So the target shape these re-keys must hit isn't settled.

**Analysis of possible solutions.**
1. **You settle the open object-model decisions, then I implement the re-keys to match.**
   Correct and low-risk. Cost: your time to make the calls. This is the intended path — the
   decisions are genuinely design choices, not implementation details.
2. **I pick defaults for the open decisions and implement.** Fast, but this is exactly the
   failure that triggered the whole full-design pass ("you simplified the design we spent hours
   working on"). I'd be committing design you reserved, on docs that say "undecided." High risk of
   rework + eroded trust. Rejected.
3. **Defer indefinitely.** Viable — the system works fully without any of it (these are
   representation-only, functional-neutral). But it leaves known divergence from the design.

**Why this needs *you* (not something I can decide).** The blocker isn't technical — I know *how*
to do each re-key. It's that the **target** is an open design decision the docs flag as unsettled,
and settling it shapes the whole object model (how every entity is addressed, stored, and routed
across shards). Choosing wrong is expensive to unwind (PK changes touch every table + every
consumer) and you've explicitly reserved these calls. There's no default I can pick that isn't me
making your design decision for you.

**Suggested path forward.** A short working session (or a written decision doc) on
`docs/object-model.md`'s 4 open decisions + D3: (a) is `hot_reference` u32 worth the PK churn now,
given #3 proved actor-reads don't need it? (b) `region_zone:u16` cold key — confirm the layout;
(c) geographic vs functional `server_reference` — pick one; (d) the hot→cold type map `PACK`
needs. Once those land in the design docs as decided, I move the todo items into `remaining` and
implement them to match — each is mechanical from a settled target.

---

## Resolved

_(none yet)_
