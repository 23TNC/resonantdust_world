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
| [shard-tables](shard-tables/README.md) | open | The `*_tables!` generalization: fold every composing shard into generic table macros; all writes via events. P1–P3 live, P4 pending. |
| [art-metadata](art-metadata/README.md) | open | Per-variant `meta.json` sidecar from `bin/art` (channel tints + shadow outline). P1–P2 done. |
| [shadows](shadows/README.md) | open | The clean restart after `lighting` was nuked: build the shadow-cold **bitfield** + shadow-hot staging + billboard projection + bit-decode overlay as the thin end-to-end foundation. |

**Archived out-of-repo (2026-07-19)** — the delivered/reverted streams were moved out of the repo
(to `../resonantdust_world_docs_archive/work-stripped-2026-07-19/`) to keep the working tree focused on
live work; git history retains them at their last in-repo commit: `texture-restructure` (done),
`spacetime-again` (done), `coord-purge` (done), `cold-rework` (done → continued as **shard-tables**),
`docs-migration` (done), `prim-batching` (closed), and `lighting` (closed — nuked 2026-07-19; the
restart is **shadows**; its durable target survives in the pixijs component `intent`/`design`).
