# Component — `shared/codec` (`resonantdust-codec`)

_Path: `shared/codec`. **Shared** crate — how it changes dictates how its consumers operate.
Last updated: 2026-07-14._

Bit-packing + the shared object model: `event_word` (the event-DSL word frame), the entity /
zone / object **reference** layouts, and the object taxonomy (`type` / `kind` / `variant`).
The wire shapes everything agrees on — consumed by shard, edge, worker, client/core, wasm.

- **[`design/`](design/)** — the shape: [`object-model.md`](design/object-model.md) (the taxonomy
  + packed references + shard classes) and [`references/`](design/references/) (the bit-layouts:
  object/hot/cold refs, the spatial ladder, reference-vs-id vocab).
- **[`current/`](current/)** — [`object-model-status.md`](current/object-model-status.md):
  implementation status + the object-model's **open decisions**, which gate the shard's
  representation re-keys ([`work/spacetime-rewrite/blockers.md`](../../../work/spacetime-rewrite/blockers.md) B-1).
- **[`intent/`](intent/)** / **[`plan/`](plan/)** — why the refs are shaped this way + the path
  to settling the open decisions.

The **event-dsl** word frame lives here (`event_word`); its *spec* is documented with the shard
([`event-dsl.md`](../../server/spacetime/modules/shard/design/event-dsl.md)) since that's where
the VM + lifecycle give it meaning.
