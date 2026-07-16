# Work — spacetime-again (the shard rebuild)

_Opened 2026-07-15. Executes [`intent/spacetime-again/`](../../intent/spacetime-again/README.md)
against [`TABLES.md`](../../TABLES.md) + [`VARIABLES.md`](../../VARIABLES.md), and the module plans
under [`components/server/spacetime/`](../../components/server/spacetime/README.md)._

## How this one runs

**One component per item. The game stays broken until the last one.** That is the method, not an
accident of it:

- **No item spans components.** A piece is finished when *it* is right — not when something
  downstream lights up. If an item can't be finished without editing a second component, it's two
  items.
- **A piece interfaces only through the intended surface** — a crate's public API, a module's
  reducers + tables, the WS protocol. Never by reaching around one.
- **Verify each piece against its own surface**, in isolation. A module is done when its reducers
  do the right thing to its tables under `spacetime call`, with no worker in existence. Don't wait
  for the stack to prove a module.
- **Broken downstream is expected and is not a defect.** `client/core`, `shared/wasm`, `npc` and
  pixijs do not build today and will stay that way until W7. Do not patch them to keep the tree
  green; that is how the last pipeline grew the legacy we just deleted.

The prize is that every piece gets built once, properly, instead of being bent around whatever was
half-done next to it.

## Files

`todo` → `remaining` → `completed`. `blockers` needs you; `forks`/`issues`/`deviations` appear when
they have content.
