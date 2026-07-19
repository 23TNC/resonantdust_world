# Work — index

_The flowing execution state. Each `work/<w>/` turns component `plan`s into executable items and
spans components by nature. Convention: [`../CONVENTIONS.md`](../CONVENTIONS.md). Last updated:
2026-07-19._

**Status** = the stream's lifecycle, not the session's focus: `open` (in progress) · `done`
(delivered; may have a small hand-off) · `closed` (abandoned/refuted) · `blocked` (needs input).
The continuation hook's notion of the **active** stream is *session-scoped* (which open stream this
session is driving — inferred, not a field here); see [`docs-authority`](docs-authority/README.md) P5.

| Stream | Status | What it is |
|---|---|---|
| [docs-authority](docs-authority/README.md) | open | This system: front-door index + `docs-check`/`work-check` audits + the enforcement hooks. |
| [texture-restructure](texture-restructure/README.md) | done | COMPLETE + verified: every kind on the new leaf; `linked/` folded into `biome-tile/`; edge serves canonical + named-variant stems; `bin/art` fully cut over (read/write + `manifest`); rock reverted to a primitive. Follow-ons are a different kind of work — art re-mastering + the biome-tile object model (placing walls). |
| [shard-tables](shard-tables/README.md) | open | The `*_tables!` generalization: fold every composing shard into generic table macros; all writes via events. P1–P3 live, P4 pending. |
| [lighting](lighting/README.md) | open | Port the old game's tiered cold/dynamic lighting onto the new G-buffer; depends on art-metadata outlines. |
| [art-metadata](art-metadata/README.md) | open | Per-variant `meta.json` sidecar from `bin/art` (channel tints + shadow outline). P1–P2 done. |
| [spacetime-again](spacetime-again/README.md) | done | The shard rebuild — W1–W9 complete + live end-to-end; deferred follow-ups tracked in its `todo.md`. |
| [coord-purge](coord-purge/README.md) | done | Retired `zone_id` + legacy `packed` from live paths (A–G landed); one browser pixel-confirm hand-off. |
| [cold-rework](cold-rework/README.md) | done | Subtype fix + overlay + region router — built + live; the ongoing generalization continued as **shard-tables**. |
| [docs-migration](docs-migration/README.md) | done | Migrated scattered docs into the `components/` + `work/` convention; two items deliberately deferred. |
| [prim-batching](prim-batching/README.md) | closed | Premise refuted 2026-07-18 — the G-buffer bake is not the draw-call cost; it was the shadow pass. |
