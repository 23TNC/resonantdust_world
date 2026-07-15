# Deviations — pathfinding (code that departs from the plan)

Where the implementation departs from [`docs/intent/pathfinding/`](../../intent/pathfinding/README.md)
(and the decisions in [forks.md](forks.md)). Per [`CONVENTIONS.md`](../../CONVENTIONS.md): log a row
**at the moment you deviate**, not when someone catches it. A deviation needs a **strong** reason —
*"less churn" / "the existing code already did X" / "it's only cosmetic" are not reasons*. If the
plan looks wrong, change **the plan** (with input).

Row: date · plan says · code does · why · fix/status.

_None yet — nothing is built._

> Two traps this feature is especially prone to, from the 0.2.3 rewrite's scars:
> - **Floating point** anywhere in the shared path/speed code. Native worker vs wasm client; FP can
>   differ. It will look like a rare, unreproducible desync, not a compile error.
> - **Renumbering an `ACTION_*` id.** They're an append-only palette — stored programs carry the
>   numbers. `place` keeps `0x100`.
