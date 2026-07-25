#!/usr/bin/env python3
"""work_check — the premature-pause detector (invoked by `rd work-check`).

Automates [[execute-dont-relitigate]] + [[decide-and-proceed]] + [[dont-checkpoint-and-wait]]:
catch a *silent premature pause* — the session handed control back while executable, unblocked
work remained and it gave no reason. The trap this deliberately AVOIDS: firing on "todo.md
non-empty" — a full todo is the normal resting state, and firing on it would forbid every
legitimate checkpoint.

The four questions, in order (the user's framing, 2026-07-25):

  1. Did a session pause?            → the Stop hook fires us.
  2. Is a work stream in flight?     → `--bind` records session→stream from real file touches.
  3. Was it THIS session's stream?   → the binding is session-keyed (was mtime-only; P6).
  4. Is there unblocked work left?   → open todo/remaining items, no open blocker, no stop-reason.

If all four hold, the pause is premature: exit 2 with the actual next items, so the session
continues the documented plan until it is **complete, interrupted, blocked, or the plan itself
is wrong** (the four legitimate exits, spelled out in the nudge).

Bounding (F6 dial, default = blocking-but-bounded). The nudge is never unsupervised:
  - a **progress guard** — the nudge repeats only while the session keeps making progress
    (completed/todo/remaining edits, commits, or working-tree changes). `WORK_CHECK_MAX_NUDGES`
    consecutive no-progress stops release the turn, because stuck is the cue to record a
    blocker, not to spin;
  - a **recency window** (`WORK_CHECK_WINDOW_MIN`, default 180) so an idle or fresh session
    is never nagged;
  - **escapes** — an open `blockers.md` row, a `work/<stream>/.stop-reason` marker, or
    `SKIP_WORK_CHECK=1`.

Note on forks vs blockers: per CONVENTIONS a *fork* is a decision the session resolves itself
(choosing an option is not a reason to pause), while a *blocker* is what needs human input. So a
legitimate pause is justified by an open blocker or a recorded stop-reason — never by an open
fork. (Fuzzy prose-matching on forks.md proved unreliable — it matched the condition's own
description — which is exactly why the signal must be explicit.)

Modes:
  --bind      read a PostToolUse hook payload on stdin; if it touched `docs/work/<stream>/`,
              record session→stream. Always exits 0 (never interferes with a tool call).
  --nudge     read a Stop hook payload on stdin; decide. Exit 2 = premature pause (block).
  --active    print the active stream for this session and exit.
  (default)   advisory report, exit 0. `--enforce` makes a premature pause exit 2.
"""
from __future__ import annotations
import json
import os
import re
import subprocess
import sys
import time

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
WORK = os.path.join(REPO, "docs", "work")
# Repo-scoped, durable, never committed (inside .git by construction).
STATE = os.path.join(REPO, ".git", "rd-work")

BULLET = re.compile(r"^\s*[-*]\s+\S")
HEADER = re.compile(r"^(#{1,6})\s+(.*)$")
DONE_HEADER = re.compile(r"✅|\bDONE\b|\bDELIVERED\b|~~", re.I)
MAX_NUDGES = int(os.environ.get("WORK_CHECK_MAX_NUDGES", "3"))


# ── state ────────────────────────────────────────────────────────────────────
def _state_path(kind: str, sid: str) -> str:
    d = os.path.join(STATE, kind)
    os.makedirs(d, exist_ok=True)
    return os.path.join(d, f"{re.sub(r'[^A-Za-z0-9_.-]', '_', sid)}.json")


def _read_json(path: str) -> dict:
    try:
        with open(path, encoding="utf-8") as fh:
            return json.load(fh)
    except Exception:
        return {}


def _write_json(path: str, obj: dict) -> None:
    try:
        with open(path, "w", encoding="utf-8") as fh:
            json.dump(obj, fh)
    except OSError:
        pass


def _hook_payload() -> dict:
    try:
        raw = sys.stdin.read()
    except Exception:
        return {}
    try:
        return json.loads(raw) if raw.strip() else {}
    except Exception:
        return {}


# ── stream facts ─────────────────────────────────────────────────────────────
def _mtime(folder: str) -> float:
    best = 0.0
    for root, _d, files in os.walk(folder):
        for f in files:
            try:
                best = max(best, os.path.getmtime(os.path.join(root, f)))
            except OSError:
                pass
    return best


def _all_streams() -> list[str]:
    if not os.path.isdir(WORK):
        return []
    return [d for d in os.listdir(WORK) if os.path.isdir(os.path.join(WORK, d))]


def _status(stream: str) -> str:
    """The stream's lifecycle per work/README.md — 'open'/'blocked'/'done'/'closed'/''.

    Selection must NOT filter on this. A stream marked `blocked` at the index level is very
    often the one being actively worked (the index lags, and a stream can be blocked on one
    item while others execute). Filtering selection by it is what made this check silently
    no-op: the real stream was `blocked`, so the mtime scan fell through to an 8-hour-stale
    `open` stream, missed the recency window, and reported "nothing to check".
    """
    index = os.path.join(WORK, "README.md")
    if not os.path.exists(index):
        return ""
    try:
        for line in open(index, encoding="utf-8"):
            m = re.match(r"\|\s*\[[^\]]+\]\(([^/)]+)/[^)]*\)\s*\|\s*([a-z]+)", line)
            if m and m.group(1) == stream:
                return m.group(2).lower()
    except OSError:
        pass
    return ""


CHECKED = re.compile(r"^\s*[-*]\s+\[[xX]\]")
UNCHECKED = re.compile(r"^\s*[-*]\s+\[\s*\]")


def _open_items(stream: str, limit: int = 8, width: int = 300) -> list[str]:
    """Open, executable items — bullets under a not-DONE header, across todo + remaining.

    `remaining.md` is the in-flight tier (CONVENTIONS): items being executed right now. It
    counts as open work — reading only todo.md missed exactly the streams mid-execution.

    Two corpus facts this has to respect (both were live bugs, 2026-07-25):
      - **`- [x]` means DONE.** 20 of 34 streams use checkboxes, and counting a ticked box as
        open work made shard-tables report 11 phantom open items — the hook would have nudged
        to redo finished work, which is how a forcing function loses its credibility.
      - **Bullets wrap.** 81% of them carry indented continuation lines; reading only the first
        line handed the nudge half-sentences ("…stamp each leaf's absolute position + effective"),
        so continuations are folded back in before truncating.
    """
    out: list[str] = []
    for name in ("todo.md", "remaining.md"):
        path = os.path.join(WORK, stream, name)
        if not os.path.exists(path):
            continue
        try:
            lines = open(path, encoding="utf-8").read().splitlines()
        except OSError:
            continue
        section, skip, i = "", False, 0
        while i < len(lines):
            line = lines[i]
            h = HEADER.match(line)
            if h:
                section = h.group(2).strip()
                skip = bool(DONE_HEADER.search(section))
                i += 1
                continue
            if skip or not BULLET.match(line) or CHECKED.match(line):
                i += 1
                continue
            parts = [line]
            i += 1
            while i < len(lines):
                nxt = lines[i]
                if not nxt.strip() or HEADER.match(nxt) or BULLET.match(nxt) or not nxt[:1].isspace():
                    break
                parts.append(nxt)
                i += 1
            item = " ".join(parts)
            item = re.sub(r"^\s*[-*]\s+(\[\s*\]\s*)?", "", item)
            item = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", item)
            item = re.sub(r"\s+", " ", item).strip()[:width]
            out.append(f"{section} · {item}" if section else item)
            if len(out) >= limit:
                return out
    return out


# Case-SENSITIVE by design. The corpus writes the state marker in caps ("✅ RESOLVED",
# "(RESOLVED 2026-07-24)", "✅ CONFIRMED") while lowercase "resolved" is ordinary prose — and
# ordinary prose is what bit us: B3's title is "`resolved_zone` is needed alongside resolved
# tile/unit … (OPEN)", so a case-insensitive match closed a blocker that says OPEN in its own
# header. An explicit OPEN overrides everything, for the same reason.
RESOLVED_MARK = re.compile(r"✅|\bRESOLVED\b|\bCLOSED\b|\bCONFIRMED\b|\bANSWERED\b")
OPEN_MARK = re.compile(r"\(\s*open\b|\bOPEN\b|\bopen\s*\)", re.I)
BLOCKER_ID = re.compile(r"^B[-\s]?\d", re.I)


def _open_blockers(stream: str) -> list[str]:
    """Open blocker sections — keyed on the SECTION HEADER, not on bullets.

    Bullet-scanning was wrong against the real corpus: the live B3 in primitive-graph
    ("`resolved_zone` … (2026-07-25, OPEN)") is prose + bold lines with no `-` bullet, so it
    read as *no blocker* and the hook nudged straight past a genuine blocker. Every stream's
    blockers.md keys state in the header instead — `## B3 — … (OPEN)` vs
    `## B2 — … ✅ RESOLVED` — so that is what we read.
    """
    f = os.path.join(WORK, stream, "blockers.md")
    if not os.path.exists(f):
        return []
    try:
        lines = open(f, encoding="utf-8").read().splitlines()
    except OSError:
        return []

    out: list[str] = []
    head, body, group_resolved = "", [], False

    def flush() -> None:
        if not head or not any(l.strip() for l in body):
            return
        bare = head.strip()
        low = bare.lower()
        if low.startswith("not blocker") or "none open" in low:
            return
        # An emptiness sentinel lives in the BODY, not the header — the `## Open` /
        # `## Resolved` grouping style writes "None open." under the header, which read as
        # a live blocker until we looked past the heading.
        if re.match(r"none\b", "\n".join(body).strip(), re.I):
            return
        if bare.lower() in ("resolved", "archive", "archived", "closed"):
            return
        if BLOCKER_ID.match(bare) or low.startswith("open"):
            if OPEN_MARK.search(bare) or not (RESOLVED_MARK.search(bare) or group_resolved):
                out.append(bare[:150])

    for line in lines:
        h = HEADER.match(line)
        if h and len(h.group(1)) <= 2:  # '#' title / '##' section
            flush()
            head, body = (h.group(2) if len(h.group(1)) == 2 else ""), []
            group_resolved = bool(head) and head.strip().lower() in ("resolved", "archive", "archived", "closed")
            continue
        body.append(line)
    flush()
    return out


def _has_open_blocker(stream: str) -> bool:
    return bool(_open_blockers(stream))


def _has_stop_reason(stream: str) -> bool:
    # An unambiguous marker file — existence, not prose. Prose-matching proved fragile (a
    # stream's own docs describe the convention → false match). The session writes
    # `work/<stream>/.stop-reason` (one line of why) to declare a deliberate pause.
    return os.path.exists(os.path.join(WORK, stream, ".stop-reason"))


# ── stream selection (question 2 + 3) ────────────────────────────────────────
def _window() -> float:
    return float(os.environ.get("WORK_CHECK_WINDOW_MIN", "180")) * 60


def _bound_stream(sid: str) -> tuple[str | None, str]:
    """The stream THIS session is driving, from its recorded file touches.

    Strength 'write' (it edited the work folder) beats 'read' (it only oriented on it).
    Returns (stream, how) — how ∈ {'write', 'read', ''}.
    """
    if not sid:
        return None, ""
    rec = _read_json(_state_path("sessions", sid))
    for kind in ("write", "read"):
        entry = rec.get(kind)
        if not entry:
            continue
        stream, ts = entry.get("stream"), entry.get("ts", 0)
        if not stream or not os.path.isdir(os.path.join(WORK, stream)):
            continue
        if ts < time.time() - _window():
            continue
        return stream, kind  # write wins outright
    return None, ""


def _active_stream(sid: str = "") -> tuple[str | None, str]:
    """Question 2+3: which stream, and is it this session's?

    Session binding first (authoritative). Falls back to the most-recently-touched stream
    within the recency window — a heuristic, kept so the check still works before a binding
    exists (e.g. the first turn after this lands), but no longer filtered by index status.
    """
    stream, how = _bound_stream(sid)
    if stream:
        return stream, how
    streams = [s for s in _all_streams() if _status(s) not in ("done", "closed")]
    if not streams:
        return None, ""
    cand = max(streams, key=lambda s: _mtime(os.path.join(WORK, s)))
    if _mtime(os.path.join(WORK, cand)) < time.time() - _window():
        return None, ""
    return cand, "mtime"


# ── progress signal (the guard) ──────────────────────────────────────────────
def _progress_sig(stream: str) -> str:
    """What "the session did something" looks like, cheaply.

    Was: the cksum of completed.md alone — so a phase that takes several turns before an entry
    lands read as *no progress*, and the guard released after a single nudge. That is precisely
    the "stops after every task" complaint, so the signal now includes the stream's own state
    files, HEAD, and the working tree.
    """
    parts = []
    for name in ("completed.md", "todo.md", "remaining.md", "issues.md", "deviations.md"):
        p = os.path.join(WORK, stream, name)
        try:
            st = os.stat(p)
            parts.append(f"{name}:{st.st_mtime_ns}:{st.st_size}")
        except OSError:
            parts.append(f"{name}:-")
    for cmd in (["git", "rev-parse", "HEAD"], ["git", "status", "--porcelain", "-uno"]):
        try:
            out = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True, timeout=10)
            parts.append(str(len(out.stdout)) + ":" + str(sum(map(ord, out.stdout[:4000]))))
        except Exception:
            parts.append("-")
    return "|".join(parts)


# ── the nudge ────────────────────────────────────────────────────────────────
def _nudge_text(stream: str, items: list[str], count: int) -> str:
    listing = "\n".join(f"    - {i}" for i in items) or "    (see the stream's todo.md)"
    return (
        f"[work-check] PREMATURE PAUSE — you are driving `docs/work/{stream}/`, it has open,\n"
        f"executable work, no open blocker, and no recorded stop-reason. Do not hand control back.\n"
        f"Next up:\n{listing}\n"
        f"\n"
        f"Continue the documented plan through phase boundaries. Stop only when one of these is\n"
        f"true, and record it before you stop:\n"
        f"  · COMPLETE   — move the items to docs/work/{stream}/completed.md\n"
        f"  · BLOCKED    — needs the user: add a row to docs/work/{stream}/blockers.md\n"
        f"  · PLAN ERROR — the plan is wrong and not trivially fixable: log it in\n"
        f"                 docs/work/{stream}/issues.md (or deviations.md) and raise it\n"
        f"  · OTHER      — write one line of why to docs/work/{stream}/.stop-reason\n"
        f"(nudge {count}/{MAX_NUDGES} · docs-authority continuation hook · SKIP_WORK_CHECK=1 bypasses)"
    )


def _do_nudge(payload: dict) -> int:
    sid = str(payload.get("session_id") or "nosession")
    stream, how = _active_stream(sid)
    if not stream:
        return 0
    if _status(stream) in ("done", "closed"):
        return 0

    items = _open_items(stream)
    if not items or _has_open_blocker(stream) or _has_stop_reason(stream):
        return 0

    path = _state_path("nudge", sid)
    prev = _read_json(path)
    sig = _progress_sig(stream)
    count = 1 if (prev.get("stream") != stream or prev.get("sig") != sig) else prev.get("count", 0) + 1

    if count > MAX_NUDGES:
        _write_json(path, {"stream": stream, "sig": sig, "count": 0})
        print(
            f"[work-check] '{stream}' still has open work but no progress across {MAX_NUDGES} "
            f"nudges — letting the turn end.\n"
            f"  Stuck is the cue to record a blocker (docs/work/{stream}/blockers.md) or write "
            f"docs/work/{stream}/.stop-reason.",
            file=sys.stderr,
        )
        return 0

    _write_json(path, {"stream": stream, "sig": sig, "count": count, "how": how})
    print(_nudge_text(stream, items, count), file=sys.stderr)
    return 2


# ── binding (question 3's input) ─────────────────────────────────────────────
def _do_bind(payload: dict) -> int:
    sid = str(payload.get("session_id") or "")
    if not sid:
        return 0
    tool = str(payload.get("tool_name") or "")
    inp = payload.get("tool_input") or {}
    path = inp.get("file_path") or inp.get("notebook_path") or ""
    if not isinstance(path, str) or not path:
        return 0
    try:
        rel = os.path.relpath(os.path.abspath(path), WORK)
    except ValueError:
        return 0
    if rel.startswith(".."):
        return 0
    stream = rel.split(os.sep)[0]
    if not stream or stream == "." or not os.path.isdir(os.path.join(WORK, stream)):
        return 0

    # Editing a work folder is a strong claim on the stream; reading one is a weak claim
    # (orienting). Strong always wins in _bound_stream.
    kind = "read" if tool == "Read" else "write"
    p = _state_path("sessions", sid)
    rec = _read_json(p)
    rec[kind] = {"stream": stream, "ts": time.time()}
    _write_json(p, rec)
    return 0


# ── entry ────────────────────────────────────────────────────────────────────
def main(argv: list[str]) -> int:
    if "--bind" in argv:
        try:
            _do_bind(_hook_payload())
        except Exception:
            pass  # a binding failure must never break a tool call
        return 0

    if "--nudge" in argv:
        if os.environ.get("SKIP_WORK_CHECK") == "1":
            return 0
        try:
            return _do_nudge(_hook_payload())
        except Exception as exc:  # never trap a turn on our own bug
            print(f"[work-check] internal error, allowing stop: {exc}", file=sys.stderr)
            return 0

    quiet = "--quiet" in argv
    enforce = "--enforce" in argv
    sid = ""
    if "--session" in argv:
        i = argv.index("--session")
        sid = argv[i + 1] if i + 1 < len(argv) else ""

    stream, how = _active_stream(sid)
    if "--active" in argv:
        if stream:
            print(stream)
        return 0
    if not stream:
        if not quiet:
            print("[work-check] no recently-active work stream — nothing to check.")
        return 0

    items = _open_items(stream)
    blockers = _open_blockers(stream)
    reason = _has_stop_reason(stream)
    premature = bool(items) and not (blockers or reason)

    if premature:
        print(_nudge_text(stream, items, 1), file=sys.stderr)
        return 2 if enforce else 0

    if not quiet:
        why = ("no open executable work" if not items else
               f"blocker open — {blockers[0]}" if blockers else "stop-reason recorded")
        print(f"[work-check] active stream '{stream}' (via {how}): OK to pause ({why}).")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
