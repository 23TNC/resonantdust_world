#!/usr/bin/env bash
# Claude Code **Stop hook** — gate the end of a turn on `rd docs-check`.
#
# When docs/ is out of compliance it blocks the stop (exit 2) with the audit
# failures on stderr, so the agent fixes drift *before* ending the turn — the
# forcing function that keeps docs/ a trustworthy external memory.
#
# Progress-aware loop guard (there is no `stop_hook_active` field to lean on):
# it records a hash of the failure set. If the *same* failures recur — the agent
# tried and didn't converge — it lets the turn end rather than trap the session.
# Any change to the failure set (progress) blocks again on the new set. Green
# clears the state. Escape hatch: SKIP_DOCS_CHECK=1 in the hook env.
set -uo pipefail

[[ "${SKIP_DOCS_CHECK:-}" == "1" ]] && exit 0

REPO="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
input="$(cat)"
sid="$(printf '%s' "$input" | python3 -c 'import sys,json;
try: print(json.load(sys.stdin).get("session_id","nosession"))
except Exception: print("nosession")' 2>/dev/null || echo nosession)"
state="/tmp/rd-docs-check.${sid}.last"

out="$("$REPO/bin/rd" docs-check --quiet 2>&1)"; rc=$?

if [[ $rc -eq 0 ]]; then
  rm -f "$state"
  exit 0
fi

hash="$(printf '%s' "$out" | cksum | awk '{print $1}')"
if [[ -f "$state" && "$(cat "$state" 2>/dev/null)" == "$hash" ]]; then
  # Identical to the last blocked stop — no progress; don't loop the session.
  rm -f "$state"
  {
    echo "[docs-check] still failing, unchanged — letting the turn end. Run \`rd docs-check\`:"
    echo "$out"
  } >&2
  exit 0
fi

printf '%s' "$hash" > "$state"
{
  echo "[docs-check] docs/ is out of compliance — fix before ending the turn"
  echo "  (this is the docs-authority Stop hook; bypass a known-noisy case with SKIP_DOCS_CHECK=1):"
  echo "$out"
} >&2
exit 2
