# Issues — shard-tables

_Problems hit + how they were resolved. Chronological. Convention:
[`../../CONVENTIONS.md`](../../CONVENTIONS.md)._

- **2026-07-19** · **`route_reference` was in TABLES.md but not VARIABLES.md** (found by the new
  `rd docs-check` ref-integrity invariant — see [`docs-authority`](../docs-authority/README.md)).
  Its layout (`type_id:4 | region_reference:8` + reserved) lived inline in `TABLES.md` only, which
  the convention forbids (VARIABLES owns layouts; TABLES names them). **Resolved:** added a
  `route_reference` entry to [`VARIABLES.md`](../../VARIABLES.md), with bit positions read off the
  file's high→low convention (type_id bits 8–11, region_reference bits 0–7). ⚠️ **This stream owns
  the code** — verify those bit positions against the actual cold-routing pack/unpack when P4 or the
  router is next touched; the file's shorthand didn't pin them explicitly.
