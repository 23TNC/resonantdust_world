# Blockers — texture-restructure

_Things needing human input: what's blocked, why it needs a human, suggested path. Open→resolved;
resolved rows keep a dated resolution line. Goal: fewer over time (a well-understood one becomes a
[`fork`](forks.md) I resolve myself)._

---

## B1 · ~~Dense-storage widening~~ — WITHDRAWN, never real — 2026-07-19

**Resolved 2026-07-19 (withdrawn).** This blocker asked whether a `u8→u16` cold-tile widening should be
in-scope. There **is no widening** — the dense tile vector is **already `Vec<u16>` `kind_reference`**
(`action.rs`, `tile/*.rs`), so a linked object (`kind_id ≥ 0x800`) already fits. The `u8` came from a
misread of the **legacy** `VARIABLES.md` line + a stale pixijs-intent doc (both now corrected). No human
input needed; no storage work in this stream. See [F4](forks.md#f4).

## B2 · `type/subtype` registry — kind_id / variant_id authority (soft) — 2026-07-19

**Needs.** Confirmation of who **owns** the `kind`→`kind_id` and `form`→`variant_id` assignments authored
into the `type/subtype` `meta.json` ([todo P0](todo.md), [forks F2](forks.md#f2)) — is the registry file
itself the source of truth, or does it mirror a codec-side enum?

**Why a human.** `kind_id`/`variant_id` are append-only object-model ids ([VARIABLES.md](../../VARIABLES.md));
picking their allocation authority now avoids a later re-number.

**Suggested path.** Make the `type/subtype` `meta.json` the **authoring** source, and (later) have the
codec read/validate against it — mirror the "folder-as-SoT" discipline used elsewhere. Confirm before
P0 authors ids. **Not hard-blocking** the shape; blocks *committing* specific id numbers.
