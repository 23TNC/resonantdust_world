#!/usr/bin/env python3
"""work_check — the premature-pause detector (invoked by `rd work-check`).

Automates [[execute-dont-relitigate]] + [[decide-and-proceed]]: catch a *silent
premature pause* — the agent handed control back while executable, unblocked work
remained and it gave no reason. The trap this deliberately AVOIDS: firing on
"todo.md non-empty" — a full todo is the normal resting state, and firing on it
would forbid every legitimate checkpoint.

Firing condition (all must hold) — the ACTIVE stream:
  - has open, executable items in todo.md (not just done-pointers), AND
  - blockers.md has no open row, AND
  - no stop-reason was recorded (an unambiguous `work/<stream>/.stop-reason`
    marker file — existence, not prose, to stay reliable).

Note on forks vs blockers: per CONVENTIONS a *fork* is a decision the agent
resolves itself (choosing an option is not a reason to pause), while a *blocker*
is what needs human input. So a legitimate pause is justified by an open blocker
or a recorded stop-reason — never by an open fork. (Fuzzy prose-matching on
forks.md proved unreliable — it matched the condition's own description — which is
exactly why the signal must be an explicit blocker/stop-reason, not inference.)

Active stream = the OPEN work stream (per work/README.md status) whose files were
modified most recently. NOTE: the *true* active stream is session-scoped; mtime is
a heuristic. This is advisory by default (exit 0). `--enforce` exits 2 when a
premature pause is detected — for the OPTIONAL Stop-hook wiring, which is gated on
the F6 autonomy dial (docs/work/docs-authority/forks.md) and NOT enabled by default.
"""
from __future__ import annotations
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
WORK = os.path.join(REPO, "docs", "work")

BULLET = re.compile(r"^\s*[-*]\s+\S")
DONE_HEADER = re.compile(r"✅\s*DONE|·\s*DONE\b", re.I)


def _mtime(folder: str) -> float:
    best = 0.0
    for root, _d, files in os.walk(folder):
        for f in files:
            try:
                best = max(best, os.path.getmtime(os.path.join(root, f)))
            except OSError:
                pass
    return best


def _open_streams() -> list[str]:
    """Streams the work index marks 'open'."""
    index = os.path.join(WORK, "README.md")
    if not os.path.exists(index):
        return []
    out = []
    for line in open(index, encoding="utf-8"):
        # table row: | [name](name/README.md) | open | … |
        m = re.match(r"\|\s*\[[^\]]+\]\(([^/)]+)/[^)]*\)\s*\|\s*([a-z]+)", line)
        if m and m.group(2).lower() == "open":
            out.append(m.group(1))
    return out


def _has_open_todo(stream: str) -> bool:
    todo = os.path.join(WORK, stream, "todo.md")
    if not os.path.exists(todo):
        return False
    for line in open(todo, encoding="utf-8"):
        if line.startswith("#"):
            continue  # headers (incl. done-pointers) are not open items
        if BULLET.match(line):
            return True
    return False


def _has_open_blocker(stream: str) -> bool:
    f = os.path.join(WORK, stream, "blockers.md")
    if not os.path.exists(f):
        return False
    text = open(f, encoding="utf-8").read()
    if re.search(r"none open", text, re.I):
        return False
    resolved = False
    for line in text.splitlines():
        if re.match(r"#{1,6}\s", line) and "resolved" in line.lower():
            resolved = True
            continue
        if re.match(r"#{1,6}\s", line):
            resolved = False
        if BULLET.match(line) and not resolved:
            return True
    return False


def _has_stop_reason(stream: str) -> bool:
    # An unambiguous marker file — existence, not prose. Prose-matching proved
    # fragile (a stream's own docs describe the convention → false match). The
    # session writes `work/<stream>/.stop-reason` (one line of why) to declare a
    # deliberate pause with open work.
    return os.path.exists(os.path.join(WORK, stream, ".stop-reason"))


def main(argv: list[str]) -> int:
    quiet = "--quiet" in argv
    enforce = "--enforce" in argv

    streams = _open_streams()
    if not streams:
        if not quiet:
            print("[work-check] no open work stream — nothing to check.")
        return 0
    active = max(streams, key=lambda s: _mtime(os.path.join(WORK, s)))

    open_todo = _has_open_todo(active)
    blocker = _has_open_blocker(active)
    reason = _has_stop_reason(active)
    premature = open_todo and not (blocker or reason)

    if premature:
        msg = (f"[work-check] active stream '{active}' has open, executable work and no "
               f"open blocker or recorded stop-reason.\n"
               f"  → Continue it, or record why you're stopping "
               f"(a blockers.md row, or write work/{active}/.stop-reason).")
        print(msg, file=sys.stderr)
        return 2 if enforce else 0

    if not quiet:
        why = ("no open executable work" if not open_todo else
               "blocker open" if blocker else
               "stop-reason recorded")
        print(f"[work-check] active stream '{active}': OK to pause ({why}).")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
