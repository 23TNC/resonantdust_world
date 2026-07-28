# Completed — docs-authority

_Executes: [`docs/CONVENTIONS.md`](../../CONVENTIONS.md). Done + verified, chronological._

- **2026-07-19** · **P0 — compliance sweep.** Brought the tree to green against the convention it
  already had:
  - `intent/sync.md` **moved + rescoped** (not deleted — see [`deviations.md`](deviations.md) D1) to
    [`components/client/core/intent/sync.md`](../../components/client/core/intent/sync.md); dead
    server framing dropped, header points the authoritative side at the shard rebuild. Inbound live
    link (`client/core/README.md`) repointed; `intent/README.md` sync entry replaced with a
    distributed-out pointer.
  - Strict-decay markers removed from authoritative files: `components/client/webgl/design/lighting.md`
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
  Added the missing `docs-migration/README.md` (invariant-5 gap; `docs-migration` archived out-of-repo 2026-07-19).

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

- **2026-07-25** · **P6 — the continuation hook actually fires (session-bound + unstuck).** The user
  reported the symptom the hook exists to prevent — "stops after each and every task" — so we
  audited it. It was **silently dead**: `_active_stream()` picked the most-recently-modified stream
  *filtered to index status `open`*, but the stream actually being driven
  (`2026-07-25-primitive-graph`) is indexed **`blocked`**, so selection fell through to an 8-hour-stale
  `open` stream, missed the recency window, and reported "no recently-active work stream — nothing to
  check." Four fixes, all verified against the real corpus:
  - **Session→stream binding (the missing question 3).** New PostToolUse hook
    [`bin/hooks/work-bind.sh`](../../../bin/hooks/work-bind.sh) → `work_check.py --bind` records
    session→stream from real file touches into `.git/rd-work/sessions/` (write = strong claim, read =
    weak). The Stop hook now checks *this session's* stream; mtime survives only as a fallback for the
    first turn after a fresh start. Verified: two sessions bound to different streams resolve
    independently.
  - **Selection no longer filters on index status** (only `done`/`closed` are skipped). A stream marked
    `blocked` is very often the one being worked — the index lags, and a stream can be blocked on one
    item while others execute.
  - **Blocker detection reads the section HEADER, not bullets.** Bullet-scanning missed the live B3 in
    primitive-graph (prose + bold, no `-` bullet) → the hook would have nudged straight past a genuine
    blocker. Markers are matched **case-sensitively** (`✅`/`RESOLVED`/`CONFIRMED`), because lowercase
    "resolved" is ordinary prose — B3's own title is "`resolved_zone` … alongside resolved tile/unit
    **(OPEN)**", which a case-insensitive match closed. An explicit `OPEN` overrides. All 15 blocker
    sections across the tree now classify correctly.
  - **Progress guard widened + deepened.** Was: cksum of `completed.md`, released after **one** nudge —
    so a phase spanning several turns before an entry lands read as "no progress" and the session was
    free to stop. Now progress = the stream's state files + `HEAD` + working tree, and the release is
    at `WORK_CHECK_MAX_NUDGES` (default **3**) *consecutive no-progress* stops. Also: `remaining.md`
    counts as open work (it's the in-flight tier), and DONE-marked sections are skipped.
  - **The nudge now says what to do** — it names the stream, lists the actual next items, and spells
    out the four legitimate exits (complete → `completed.md` · blocked → `blockers.md` · plan-error →
    `issues.md` · other → `.stop-reason`), which is the behaviour the dial was always meant to drive.
  Matrix re-verified: blocked stream→exit 0; unblocked open work→exit 2; nudge 1/2/3→block, 4th→release,
  progress→counter resets and re-arms; `.stop-reason`, `SKIP_WORK_CHECK=1`, and stale-window→exit 0.
  `docs-check` green throughout. Stage 2 of the Stop hook now delegates the whole decision to
  `work_check.py --nudge` (the payload is replayed on stdin so it can key on `session_id`).

- **2026-07-25** · **P6a — two live detector bugs, found by auditing the corpus instead of the code.**
  Asked to review P6's robustness, we measured what the parser actually sees across all 34 streams
  rather than reasoning about it, which surfaced two defects shipped an hour earlier:
  - **`- [x]` counted as open work.** 20 of 34 streams use checkboxes; ticked ones read as open →
    **13 phantom items**, 11 of them in `shard-tables` (its whole done P1). The hook would have nudged
    to redo finished work — the failure mode that destroys a forcing function's credibility, because
    the correct response to it is to bypass the hook.
  - **Wrapped bullets truncated at the first line.** **81%** (180/222) of bullets carry indented
    continuation lines, so the nudge was emitting half-sentences ("…stamp each leaf's absolute
    position + effective"). Continuations are now folded in; the width cap went 150 → 300.
  Both fixed + re-measured (phantom items 13 → 0). **The standing lesson:** this detector's inputs are
  hand-written prose in 4 different item dialects, so *test it against the corpus, not against
  intuition* — every one of the 6 classification bugs so far was invisible from the code alone.

- **2026-07-25** · **P7 — continuation half handed off.** P6/P6a made the hook fire and corrected two
  live defects; the remaining work (item contract, arming, brief, decision log, planning assist) is
  substantial and multi-phase, with a different failure model from the audit — it fails *open and
  silent*, where `docs-check` fails loud. Split to
  [`2026-07-25-continuation-hooks`](../2026-07-25-continuation-hooks/README.md), cross-linked both
  ways. `docs-authority` keeps the audit: `docs-check`, its invariants (now 7 — `work-items` added
  there), the front door, and the git pre-commit.
