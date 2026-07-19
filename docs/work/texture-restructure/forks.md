# Forks — texture-restructure

_Execution decisions. The **shape** is Decided upstream in
[`texture-layout/`](../../components/dev/textures/design/texture-layout/README.md) — not re-litigated
here. These are the calls that come up **carrying it out**._

---

## F1 · Variant folder = named OR numeric (decided, user) — 2026-07-19

**Decision.** `<variant>` accepts **both**: numeric (`0..15`, art variations straight off a sprite
sheet — flora etc. have many) and **named** (a linked object's **form**: `wall`/`fence`/`rock`). Both
resolve to `variant_id`; the `type/subtype` registry ([todo P0](todo.md)) maps names → index. **≥16
truncates** out of the manifest (`u4`) — extra on-disk variations exist but go unused, and the manifest
walk must `log()` the drop, not silently cap.

## F2 · `form → variant_id` numbering (open — proposal) — 2026-07-19

**Context.** Linked forms need a stable `variant_id`. Numbering lives in the `type/subtype` registry,
not hardcoded in `texpath.py`.

**Proposal (confirm).** Assign in the registry by first-appearance / explicit list, e.g.
`wall=0, fence=1, rock=2, …`, reserving low indices for the common forms. Keep it **explicit in the
registry file** (not derived from a `readdir` order, which is unstable). Open until the registry is
authored (P0).

## F3 · Linked old-variant remapping (open — needs inspection) — 2026-07-19

**Context.** Old linked leaves like `wall.smooth/1.l.0/0..8/` carry numeric `<variant>` folders. In the
new model a linked object's `<variant>` is the **form**, so these old numerics must map to *something* —
but it's unclear whether they were **auto-tile connectivity pieces** (→ belong in `dir`/`part`, the `l`
facing's sub-indices) or genuine **art variations** (→ stay variations, but of which form?).

**To clear.** `find`/inspect the actual `linked/*/1.l.0/*` contents before P3 collapses them; the mapping
follows what they are. Do **not** blind-collapse. Flag per-kind if they differ.

## F4 · Storage needs NO change — the dense tile vector is already `Vec<u16>` kind_reference — 2026-07-19

**Decision.** Folding `linked/`→`biome-tile` requires **zero server-storage work**. The dense tile
vector already holds a **`u16` `kind_reference` per cell** (`kind_id:12 | variant_id:4`) —
`shared/codec/src/action.rs` ("tile = dense `kind_reference` by index") + `server/st-bindings/src/tile/*.rs`
(`tiles: Vec::<u16>`). A linked object is just a `kind_reference` with `kind_id ≥ 0x800`; it drops into
the existing vector unchanged. This stream is therefore **purely file structure** — no codec/worldgen/edge
storage change rides with it.

**Correction (2026-07-19):** an earlier draft of this fork + [B1] claimed a `u8→u16` widening was needed.
That was a misread of the **legacy** `u8 tile … dense Vec<u8>[256]` line (`VARIABLES.md`, "Legacy —
retiring") and a stale pixijs-intent doc — **not** the current storage, which is already `u16`. No
separate stream exists; withdrawn.
