# Blockers — texture-restructure

_Things needing human input: what's blocked, why it needs a human, suggested path. Open→resolved;
resolved rows keep a dated resolution line. Goal: fewer over time (a well-understood one becomes a
[`fork`](forks.md) I resolve myself)._

---

## Open

None open.

## Resolved

- **B1 · `type/subtype` registry — kind_id / variant_id authority — resolved 2026-07-19.** The
  `type/subtype` `meta.json` is the **authoring source of truth** for `kind`→`kind_id` and
  `form`→`variant_id` (the "folder-as-SoT" discipline used elsewhere); the codec may later
  read/validate against it, but there is **no competing codec-side enum**, so there is nothing to
  re-number against — which is exactly why picking this authority now closes the re-number risk. P0
  authors the ids there. Adopted the suggested path (soft, low-regret) per decide-and-proceed.
