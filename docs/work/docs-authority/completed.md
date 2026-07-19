# Completed — docs-authority

_Executes: [`docs/CONVENTIONS.md`](../../CONVENTIONS.md). Done + verified, chronological._

- **2026-07-19** · **P0 — compliance sweep.** Brought the tree to green against the convention it
  already had:
  - `intent/sync.md` **moved + rescoped** (not deleted — see [`deviations.md`](deviations.md) D1) to
    [`components/client/core/intent/sync.md`](../../components/client/core/intent/sync.md); dead
    server framing dropped, header points the authoritative side at the shard rebuild. Inbound live
    link (`client/core/README.md`) repointed; `intent/README.md` sync entry replaced with a
    distributed-out pointer.
  - Strict-decay markers removed from authoritative files: `components/client/pixijs/design/lighting.md`
    (wedge "superseded" → "current approximation of the target"), `components/dev/textures/design/art-style.md`
    ("being superseded" → "is replacing"), `TABLES.md` (`faction … (deprecated)` → `bits 0–1 reserved`).
  - `work/docs-migration/todo.md` strikethrough/"void" mess cleared — kept only the two
    deliberately-deferred items; done detail already lived in its `completed.md`.
  - Verified: 0 strict-decay markers in `design/**` + top authorities; the 5 touched links resolve;
    no rendered strikethrough left in `work/**/todo.md` (remaining `~~` hits are backtick-quoted
    pattern *descriptions* in this stream's own plan — handled by the P2 code-span exemption).

- **2026-07-19** · **P1 — the front door.** Created [`docs/README.md`](../../README.md) (the root
  entry point: precedence order, the three authorities, the trees, "read `design/` not the ticket")
  and [`docs/work/README.md`](../README.md) (the work index — 10 streams with lifecycle status).
  Added the missing [`docs-migration/README.md`](../docs-migration/README.md) (invariant-5 gap).

- **2026-07-19** · **P2 — the audit `bin/rd docs-check`.** [`bin/lib/docs_check.py`](../../../bin/lib/docs_check.py)
  (engine: 6 invariants — decay-markers, ref-integrity, link-integrity, freshness, work-linkage,
  map-bijection; code-span aware; `--quiet` for the hook, `--list` for discovery) +
  [`bin/lib/docs.sh`](../../../bin/lib/docs.sh) wrapper, dispatched from `bin/rd` (`docs-check` /
  `work-check`) with a usage entry. First run caught 8 errors + 17 warnings; all real ones fixed,
  over-strict checks tightened (see [`deviations.md`](deviations.md) D4). **Tree is green:** 95 files,
  0 errors, 0 warnings; `--quiet` silent + exit 0 verified.

- **2026-07-19** · **P3 — the hooks (automatic enforcement, F2/F5).** Verified the exact Claude Code
  Stop-hook contract first (no `stop_hook_active` field → external loop guard; block via exit 2 +
  stderr; cwd = project root). Built [`bin/hooks/stop-check.sh`](../../../bin/hooks/stop-check.sh)
  (Stop hook: exit-2 block on failure, **progress-aware** loop guard via a failure-hash so a
  non-convergent case can't trap the session) + [`bin/hooks/pre-commit`](../../../bin/hooks/pre-commit)
  (hard commit gate, `SKIP_DOCS_CHECK=1` escape), wired via [`.claude/settings.json`](../../../.claude/settings.json)
  (Stop) + a `.git/hooks/pre-commit` symlink. **Tested all paths:** green→exit 0 silent;
  violation→exit 2 with output; identical re-fail→exit 0 (loop guard); pre-commit pass on green.
  Hook behaviour + install documented in the stream README. work-check stays *out* of the Stop hook
  until the F6 autonomy dial is set.

- **2026-07-19** · **P4 — freshness provenance.** Documented the `current/` stamp convention in
  `CONVENTIONS.md` (§ `current/`: `Last updated: DATE` or `verified @ <sha>`). Built the staleness
  **comparison** in `docs_check.py` (warning-only, git-backed, best-effort — never breaks the audit):
  it maps a `current/` file to its code via the component README's existing `Path:` field and warns
  when the code changed after the stamp. **Proved itself on first run** — flagged `index/current`
  (stamped 2026-07-15, code changed 2026-07-17) as a 2-day-lagging cache; spun off to the index
  component to re-verify + re-stamp (rather than fake-bumping the date). This is the mechanical guard
  against the phantom-project bug the convention warns about.

- **2026-07-19** · **P5 — `work-check` detector (advisory).** Built [`bin/lib/work_check.py`](../../../bin/lib/work_check.py)
  + wired `rd work-check`. Detects a *silent premature pause*: the active stream (most-recently-
  modified `open` stream) has open executable todo items AND no open blocker AND no `.stop-reason`
  marker. Deliberately does **not** fire on "todo non-empty" (the normal resting state). Three prose
  false-matches during the build forced a redesign to structural/marker signals only (dropped the
  fork check; stop-reason = a gitignored `.stop-reason` file, not prose — see [`deviations.md`](deviations.md)
  D5). All paths verified: premature→exit 2 (`--enforce`), blocker-open→OK, marker→OK. **Blocking-wire
  into the Stop hook is deferred** to the F6 autonomy dial — recorded as an open [`blocker`](blockers.md);
  advisory `rd work-check` is live now.

- **2026-07-19** · **P5 wiring — continuation hook LIVE (F6 dial → default).** User: "wire with your
  default." Merged docs-check + work-check into one two-stage Stop hook
  [`bin/hooks/stop-check.sh`](../../../bin/hooks/stop-check.sh) (renamed from `stop-docs-check.sh`;
  the rename left broken links that `docs-check` immediately caught + I fixed — the audit earning its
  keep again). Stage 2 = `work-check --enforce`, **blocking-but-bounded**: added a **recency window**
  (`WORK_CHECK_WINDOW_MIN`, default 180) so a fresh/idle session never nags, a **progress guard** (no
  new `completed.md` entry since the last nudge → release), and `SKIP_WORK_CHECK=1` + `.stop-reason` +
  blocker escapes. **Full matrix tested:** green→exit 0; docs-broken→stage-1 block; premature→stage-2
  block; no-progress repeat→release; fresh nudge→block again (bounded, never infinite). F6 resolved
  (dial=default) in [`forks.md`](forks.md); blocker closed in [`blockers.md`](blockers.md).
