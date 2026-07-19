# Todo — docs migration (remaining)

_Executes: [`docs/CONVENTIONS.md`](../../CONVENTIONS.md). The migration itself is done (see
[`completed.md`](completed.md)); only two items remain, both **deliberately deferred**. Newest-first._

- **2026-07-14** · **server/edge** — no edge-specific top-level doc remains (worldgen is inline in
  `edge/src`); when edge is next worked, create `components/server/edge/{intent,current}` from the
  map's edge entry + the worldgen relationship. Deferred by design: lazy-create means a component
  earns its folders when worked, and edge hasn't been — the [map entry](../../components/README.md)
  is its home until then. Low priority.
- **2026-07-14** · **`docs/prompts/*`** (sprite prompt templates) — move **with** the
  `bin/` → `dev/scripts/` reorg, which is itself proposed-not-done (see the map's open list). Not
  worth moving twice.
