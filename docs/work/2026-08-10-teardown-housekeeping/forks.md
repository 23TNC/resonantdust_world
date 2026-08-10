# forks — teardown-housekeeping

Decision points, options, which we chose, why. Chronological append. Prefixed `H` so they don't
collide with the login stream's `F` rows. All resolved — the one thing genuinely needing the user is
in [`blockers.md`](blockers.md), not here.

---

### H1 — Salvage-first, or clean up in place? · 2026-08-10 · resolved

**Chosen:** P0 salvages every single-copy artefact before any phase deletes anything, and P2/P3
declare themselves gated on it.

**Why.** The teardown already produced one near-miss: `../resonantdust_world_old` was verified as a
complete duplicate *of tracked content* before 42G was deleted from the working tree — which was the
right check for tracked files and the wrong one for gitignored files. `bin/keys/*.env` was
deliberately never committed, so "it's in the backup copy" is the whole of its existence. A cleanup
stream whose first act is deletion would repeat that reasoning error at a moment when the backup is
itself the target. Ordering is the mitigation; a warning in prose is not.

---

### H2 — When does `../resonantdust_world_old` go? · 2026-08-10 · resolved

| Option | Verdict |
|---|---|
| **Mark read-only now, delete in the stream that re-establishes textures** | **chosen** |
| Delete this stream, after salvage | rejected |
| Keep indefinitely | rejected |

**Why.** Once P0 lifts the credentials out, the only thing the 45G copy uniquely holds is the 142M
texture corpus — everything tracked is on `origin` and reachable with `git show 0.2.3:<path>`.
Textures are regenerable in principle (`bin/art`, laigter, marigold, ComfyUI) but that pipeline is
itself deleted, so regenerating means restoring a toolchain first; and 0.3.0 cannot render anything
yet, so nobody can confirm what a replacement needs. Deleting now would be trading 142M of
irreplaceable-in-practice art for disk we measurably do not need (863G free).

Keeping it indefinitely is also wrong, because an un-owned 45G copy called `_old` is exactly the
ambiguity this stream exists to remove. So: `chmod -R a-w` plus a `RETIRING.md` at its root naming
the stream allowed to delete it. It stops being a live copy without becoming a mystery.

---

### H3 — `docker system prune -a`, or remove by name? · 2026-08-10 · resolved

**Chosen:** remove by name; keep `busybox`, `alpine`, `rust:slim`, `debian:bookworm-slim`,
`python:3-slim`.

**Why.** `prune -a` would reclaim maybe 2G more and immediately cost it back on the next re-pull —
0.3.0 will want a Rust image, and the teardown itself needed `busybox` to delete root-owned files.
On a disk at 10% the blanket option is strictly worse. Naming the six 0.2.x build images
(`rd-sim-builder`, `gateway-build`, `server-build`, `edge-build`, `simulation-build`,
`simulation-run`) plus the spacetime and art images also leaves a readable record of what was
removed, which `prune -a` does not.

The 19 volumes are the exception — every one reports 0 containers, and their names
(`rd_cargo_gate`, `actionmanager_cargo-target`, `simulation_cargo-cache`, …) map to iterations that
predate `resonantdust_world`. They go as a set.

---

### H4 — Who owns porting `docs_check.py`? · 2026-08-10 · resolved

**Chosen:** this stream, P4. The login stream's F7 deferred it to "its own small stream"; this is
that stream, and the F7 row gets repointed here rather than left dangling.

**Why.** `CONVENTIONS.md` was restored and cites `bin/rd docs-check` throughout — index-link
enforcement, the checkbox-vs-bullet ERROR, oversized-item warnings. A convention whose enforcement
was deleted is precisely a teardown loose end. It also stops being hypothetical the moment there are
two work folders instead of one, which happened today.

Scope trimmed deliberately: the `current/` freshness-stamp check is dropped until
`docs/components/` exists, because a checker that fails on an absent tree teaches people to ignore
it.

---

### H5 — Keep the memories that describe deleted code? · 2026-08-10 · resolved

**Chosen:** three-way split, not a blanket purge. Keep-and-mark-historical the ones carrying design
*reasoning* that outlived its implementation (`client-sync`, `art-style`, `object-model-redesign`);
delete the ones that only name deleted mechanics (a `*_tables!` macro, a verb number, a `bin/rd`
subcommand); keep as-is the ones still true (`edit-via-edit-tool`, `commit-freely`, the working-style
entries).

**Why.** The failure mode a stale memory causes is specific: it names a file or flag that no longer
exists and a future session acts on it. That is a property of *mechanical* memories, not of design
ones. "The client must display what the server says, never extrapolate" cost real evidence to learn
and is a stance the rewrite should inherit — deleting it because the code that proved it is gone
would throw away the expensive half. The `v0-3-0-rewrite` memory already fronts the index with the
warning, so history is safe to keep as long as it is labelled.

---

### H6 — Replace the deleted `rd-plan` / `rd-execute` skills? · 2026-08-10 · resolved

**Chosen:** delete both now; decide on a replacement planning skill afterwards, and record that
decision as a row rather than leaving the question open.

**Why.** Both are tracked files that invoke `bin/rd work arm|brief|doctor` and read
`docs/components/`, none of which exist — so they are not "stale docs", they are commands that
fail. Their `rd-execute` half also armed a continuation hook whose scripts (`bin/hooks/work-bind.sh`,
`stop-check.sh`) went with the teardown, which is why `.claude/settings.json` is now `{"hooks": {}}`.
Keeping a skill that cannot run is worse than having none: it advertises a capability, and the
listing is what gets consulted first.

Whether 0.3.0 wants the continuation machinery back at all is a real question, and a smaller one
once there is a checker (P4) and a component tree to plan against.
