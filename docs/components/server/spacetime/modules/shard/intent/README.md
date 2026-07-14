# Intent — `shard` (the what + why)

_How the shard is used and why the design is shaped the way it is: entry/exit points, what we
store into the shapes and why, what we read where and why. The [`design/`](../design/) holds the
*shape*; this holds *what goes in it, and why*. Last updated: 2026-07-14._

- **[`events.md`](events.md)** — how events work: an event is a `Vec<u64>` action program +
  targets; how they're produced (edge) and consumed (worker).
- **[`lifecycle.md`](lifecycle.md)** — the two-phase recoverable lifecycle, drop-on-miss
  causality (no watermark), deterministic composition, convergent cross-shard writes, refcounted
  GC. The *why* behind the tables.
- **[`hot-cold.md`](hot-cold.md)** — the cold↔hot bridge: find-or-mint (mint absorbed into
  enqueue) and `PACK` (settle hot→cold). Why "to touch a cold object, promote it first" is
  automatic.
- **[`risks.md`](risks.md)** — the load-bearing assumptions + what fails silently (determinism,
  cross-shard tic sync).
- **[`event-reference.md`](event-reference.md)** — why `event_reference` is a `u32`.
