# Work — index

_The flowing execution state. Each `work/<w>/` turns component `plan`s into executable items and
spans components by nature. Convention: [`../CONVENTIONS.md`](../CONVENTIONS.md). Last updated:
2026-07-20._

**Status** = the stream's lifecycle, not the session's focus: `open` (in progress) · `done`
(delivered; may have a small hand-off) · `closed` (abandoned/refuted) · `blocked` (needs input).
The continuation hook's notion of the **active** stream is *session-scoped* (which open stream this
session is driving — inferred, not a field here); see [`docs-authority`](docs-authority/README.md) P5.

| Stream | Status | What it is |
|---|---|---|
| [docs-authority](docs-authority/README.md) | open | This system: front-door index + `docs-check`/`work-check` audits + the enforcement hooks. |
| [shard-tables](shard-tables/README.md) | open | The `*_tables!` generalization: fold every composing shard into generic table macros; all writes via events. P1–P3 live, P4 pending. |
| [art-metadata](art-metadata/README.md) | open | Per-variant `meta.json` sidecar from `bin/art` (channel tints + shadow outline). P1–P2 done. |
| [shadow-tiered](shadow-tiered/README.md) | done | The shadow rework fixing `shadow-world`'s aliasing: cast the realtime lights in **screen space** (window-bounded) → `screen-shadow-*`, merge into the **world-space** `shadow-*` bitfield (clear dirty bits + OR the remapped screen field, one pass), display `shadow` OR `screen-shadow`. **T1–T7 verified 2026-07-20** — no aliasing on pan, moved lights re-cast same-frame; the stale-on-pan slot-invalidation (I-9) fixed during verify. Graduates toward **shadows**. |
| [light-data-texture](light-data-texture/README.md) | open | Feed per-light data to the shader through a **5×5 data texture** (one column per light) instead of uniforms/hardcoded colours — the mechanism dense many-lights needs. `RGBA32F` float (ES 1.00 rules out true `u32`); shadow coloured from the texture, colours rewritten every frame to prove the live path. Extends `shadow-tiered`. |
| [shadow-world](shadow-world/README.md) | closed | Proved world-space `shadow-cold` storage + `/overlayRT` decode (W1–W5), BUT its direct world-cast **aliased off-window lights** (raw `mod`, no window-bounds check) — surfaced on pan. Superseded by **shadow-tiered** (screen-cast + copy). Kept for the mapping + overlay-decode lessons. |
| [shadows](shadows/README.md) | open | The clean restart after `lighting` was nuked. Cast shadows **screen-space** (`shadow-hot`, every frame, billboards) → copy into a **world-space** `shadow-cold` **light-bitfield** via the 4-way toroidal wrap; round-robin 3/frame → 24 lights; per-bit colour `/overlayRT`. Sidesteps the per-rect world-space baking that sank the last attempt. |

**Archived out-of-repo** — completed/reverted streams live in **`../archive/`** (git history retains them
at their last in-repo commit). **2026-07-20:** `bitfield-rt` (done — bitfield RT round-trip + ping-pong)
and `shadow-cast` (done — shadow-casting into a bitfield + incremental updates), which the new
`shadow-world` builds on. **2026-07-19** (`../archive/work-stripped-2026-07-19/`): `texture-restructure`,
`spacetime-again`, `coord-purge`, `cold-rework` (→ **shard-tables**), `docs-migration`, `prim-batching`,
and `lighting` (nuked; restart is **shadows**, target in the pixijs component `intent`/`design`).
