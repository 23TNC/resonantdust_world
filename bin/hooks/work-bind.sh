#!/usr/bin/env bash
# Claude Code **PostToolUse hook** — bind this session to the work stream it is driving.
#
# Answers question 3 of the continuation check ("was the session that paused the one
# working on that stream?"). Every Read/Write/Edit under `docs/work/<stream>/` records
# session→stream; an edit is a strong claim, a read a weak one. The Stop hook then
# checks THIS session's stream instead of guessing by directory mtime — the guess is
# what silently broke the check (it filtered to `open` streams, so a stream marked
# `blocked` while actively worked fell through to a stale one and no-op'd).
#
# Never blocks and never fails a tool call: always exit 0.
set -uo pipefail
REPO="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"
python3 "$REPO/bin/lib/work_check.py" --bind >/dev/null 2>&1
exit 0
