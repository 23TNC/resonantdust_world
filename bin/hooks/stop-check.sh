#!/usr/bin/env bash
# Claude Code **Stop hook** — gate the end of a turn on the docs-authority checks.
#
# Stage 1 · docs-check — docs/ must be compliant. Broken → block (exit 2) with the
#   failures. Progress-aware loop guard: an identical failure set twice in a row
#   (agent tried, no change) releases, so a non-convergent case can't trap the turn.
# Stage 2 · work-check — if docs are clean, a *silent premature pause* (open,
#   unblocked, executable work in THIS SESSION's stream + no recorded stop-reason)
#   blocks too, pushing the session to continue the documented plan. All of the
#   decision — session→stream binding, open-work scan, blocker/stop-reason escapes,
#   and the progress guard — lives in work_check.py's `--nudge` mode, which is
#   handed the raw hook payload on stdin (it needs session_id).
#
# Escapes: SKIP_DOCS_CHECK=1 (stage 1), SKIP_WORK_CHECK=1 (stage 2). work-check's
# reach is tunable via WORK_CHECK_WINDOW_MIN (recency window, default 180) and
# WORK_CHECK_MAX_NUDGES (no-progress stops before release, default 3).
set -uo pipefail

REPO="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
input="$(cat)"
sid="$(printf '%s' "$input" | python3 -c 'import sys,json
try: print(json.load(sys.stdin).get("session_id","nosession"))
except Exception: print("nosession")' 2>/dev/null || echo nosession)"

# ── stage 1 · docs-check (hard gate) ─────────────────────────────────────────
if [[ "${SKIP_DOCS_CHECK:-}" != "1" ]]; then
  dstate="/tmp/rd-docs-check.${sid}.last"
  dout="$("$REPO/bin/rd" docs-check --quiet 2>&1)"; drc=$?
  if [[ $drc -ne 0 ]]; then
    dhash="$(printf '%s' "$dout" | cksum | awk '{print $1}')"
    if [[ -f "$dstate" && "$(cat "$dstate" 2>/dev/null)" == "$dhash" ]]; then
      rm -f "$dstate"
      { echo "[docs-check] still failing, unchanged — letting the turn end. Run \`rd docs-check\`:"; echo "$dout"; } >&2
      exit 0
    fi
    printf '%s' "$dhash" > "$dstate"
    { echo "[docs-check] docs/ is out of compliance — fix before ending the turn (SKIP_DOCS_CHECK=1 to bypass):"; echo "$dout"; } >&2
    exit 2
  fi
  rm -f "$dstate"
fi

# ── stage 2 · work-check (blocking, bounded) ─────────────────────────────────
# The payload is replayed on stdin so the detector can key on session_id — that is
# what answers "was the session that paused the one driving this stream?".
[[ "${SKIP_WORK_CHECK:-}" == "1" ]] && exit 0
printf '%s' "$input" | python3 "$REPO/bin/lib/work_check.py" --nudge
exit $?
