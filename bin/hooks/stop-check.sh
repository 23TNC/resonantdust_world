#!/usr/bin/env bash
# Claude Code **Stop hook** — gate the end of a turn on the docs-authority checks.
#
# Stage 1 · docs-check — docs/ must be compliant. Broken → block (exit 2) with the
#   failures. Progress-aware loop guard: an identical failure set twice in a row
#   (agent tried, no change) releases, so a non-convergent case can't trap the turn.
# Stage 2 · work-check — if docs are clean, a *silent premature pause* (open,
#   unblocked, executable work in the active stream + no recorded stop-reason)
#   blocks too, pushing the agent to continue. Bounded by a progress guard: if the
#   block recurs with no new completed.md entry, release — two no-progress stops
#   mean stuck, which is the cue to record a blocker, not to spin.
#
# Escapes: SKIP_DOCS_CHECK=1 (stage 1), SKIP_WORK_CHECK=1 (stage 2). work-check's
# reach is tunable via WORK_CHECK_WINDOW_MIN (recency window, default 180).
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
[[ "${SKIP_WORK_CHECK:-}" == "1" ]] && exit 0
wout="$("$REPO/bin/rd" work-check --enforce --quiet 2>&1)"; wrc=$?
[[ $wrc -ne 2 ]] && exit 0   # not premature (or no recently-active stream) → ok to stop

active="$("$REPO/bin/rd" work-check --active 2>/dev/null)"
wstate="/tmp/rd-work-check.${sid}.last"
prog="$(cksum "$REPO/docs/work/${active}/completed.md" 2>/dev/null | awk '{print $1}')"; prog="${prog:-none}"
if [[ -f "$wstate" && "$(cat "$wstate" 2>/dev/null)" == "$prog" ]]; then
  rm -f "$wstate"
  { echo "[work-check] '$active' still has open work but no progress since the last nudge — letting the turn end."
    echo "  If stuck: record a blocker (docs/work/$active/blockers.md) or write docs/work/$active/.stop-reason."; } >&2
  exit 0
fi
printf '%s' "$prog" > "$wstate"
{ echo "$wout"; echo "  (docs-authority continuation hook; SKIP_WORK_CHECK=1 to bypass)"; } >&2
exit 2
