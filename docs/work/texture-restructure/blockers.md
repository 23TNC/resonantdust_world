# Blockers — texture-restructure

_Things needing human input: what's blocked, why it needs a human, suggested path. Open→resolved;
resolved rows keep a dated resolution line. Goal: fewer over time (a well-understood one becomes a
[`fork`](forks.md) I resolve myself)._

---

## B1 · Confirm dense-storage scope (soft — sequencing, not code) — 2026-07-19

**Needs.** A yes on the [F4](forks.md#f4) boundary: the `u8→u16` cold-tile dense widening (so linked
rows fit the biome-tile vector) is a **separate server-side stream**, not part of this file-structure
migration.

**Why a human.** It's a scope/sequencing call with a downstream cost (codec/worldgen/edge + the
[cold-rework](../cold-rework/README.md) storage). Reasonable either way; the user flagged it.

**Suggested path.** Keep it separate (this stream stays file-only, ships independently); open a
`biome-tile-dense-width` stream when the runtime side is taken on. **Not hard-blocking** — P0–P6 here
can proceed without it (texture files don't depend on the dense cell width).

## B2 · `type/subtype` registry — kind_id / variant_id authority (soft) — 2026-07-19

**Needs.** Confirmation of who **owns** the `kind`→`kind_id` and `form`→`variant_id` assignments authored
into the `type/subtype` `meta.json` ([todo P0](todo.md), [forks F2](forks.md#f2)) — is the registry file
itself the source of truth, or does it mirror a codec-side enum?

**Why a human.** `kind_id`/`variant_id` are append-only object-model ids ([VARIABLES.md](../../VARIABLES.md));
picking their allocation authority now avoids a later re-number.

**Suggested path.** Make the `type/subtype` `meta.json` the **authoring** source, and (later) have the
codec read/validate against it — mirror the "folder-as-SoT" discipline used elsewhere. Confirm before
P0 authors ids. **Not hard-blocking** the shape; blocks *committing* specific id numbers.
