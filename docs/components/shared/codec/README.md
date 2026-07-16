# Component — `shared/codec` (`resonantdust-codec`)

_Path: `shared/codec`. **Shared** crate — how it changes dictates how its consumers operate.
Last updated: 2026-07-15._

The bit-packing layer: the reference layouts (`refs`, `object`), the tic ring (`tic`), and the
legacy `zone_id` / thing-tile packing (`packed`). Consumed by the spacetime modules, edge,
client/core and wasm.

**It has no `design/` or `intent/` of its own.** Its shapes are cross-component by definition, so
they live where every consumer looks:

| | |
|---|---|
| the layouts — names, widths, bits | [`docs/VARIABLES.md`](../../../VARIABLES.md) — **authoritative** |
| why they're shaped that way, and what was removed | [`docs/notes/variables.md`](../../../notes/variables.md) |

This crate is the **code of record**, not the source of truth: when it disagrees with VARIABLES.md,
the code is the bug.
