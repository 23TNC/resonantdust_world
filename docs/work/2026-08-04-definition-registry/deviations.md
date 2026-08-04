# Deviations — definition registry

_Where the code departs from the plan. Log a row at the moment of deviating, not when caught.
Rows: date · what the plan says · what the code does · why · fix/status._

## 2026-08-04 · the reducer takes the composed `id`, it does not compose one

**Plan** ([P2](todo.md)): `ensure(type, subType, kind, variant, version) → id` — the module
allocates and returns the number.

**Code**: `ensure_definition(id, version, type_name, sub_type, kind, variant)` — the caller composes
the id; the module records it, enforces uniqueness, and rejects a collision.

**Why**: the module cannot compose it ([I8](issues.md#i8)). `subtype_id` is **authored in the
corpus** (`biomes.toml` writes `subtype = 6` for forest) and the module never reads `content/`;
`variant_id` is **chosen at placement**, not a per-def fact (`worldgen.rs:127` rolls
`(seed >> 13) & 0x0F` per cell). Only `type_id` and `kind_id` are reachable from inside the module,
and a reducer that needed the corpus would need it mounted into WASM.

This is a **split of responsibility**, not a reduction in scope: the registry is still the durable,
unique, single-writer authority for what a number means. Composition moves to the master, where the
corpus is already loaded — which is also where [F11](forks.md#f11) put the allocator.

**Status**: accepted, plan item reworded to match.
