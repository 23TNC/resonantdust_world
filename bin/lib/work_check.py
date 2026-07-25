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
MAX_NUDGES = int(os.environ.get("WORK_CHECK_MAX_NUDGES", "3"))

# ── the item contract (P0) ───────────────────────────────────────────────────
# An ITEM is a checkbox line in todo.md / remaining.md. Nothing else is an item: a plain
# bullet is detail under an item, and prose is prose. This single rule replaces the whole
# inference layer (DONE-header scanning, bullet-vs-prose, header casing) that produced eight
# classification bugs — the state is now written by the human, not guessed from their prose.
# Top-level vs nested is deliberately NOT part of the rule: a nested checkbox is still an item.
ITEM = re.compile(r"^\s*[-*]\s+\[([ xX])\]\s*(.*)$", re.M)


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


def _scan_items(stream: str) -> list[tuple[str, bool, str]]:
    """Every item in the stream's plan files, as (section, done, text).

    The ONLY rule is the checkbox (`ITEM`). No DONE-header inference: where a ticked-looking
    header disagreed with an unticked box the corpus was wrong, not the rule — 7 boxes under
    "DONE (moved to completed.md)" had simply never been ticked, and 8 more were real deferred
    work parked under a delivered phase (see forks F5). The heuristic was hiding both.

    Wrapped continuation lines are folded in: 81% of items carry them, and reading only the
    first line handed the nudge half-sentences.
    """
    out: list[tuple[str, bool, str]] = []
    for name in ("todo.md", "remaining.md"):
        path = os.path.join(WORK, stream, name)
        if not os.path.exists(path):
            continue
        try:
            lines = open(path, encoding="utf-8").read().splitlines()
        except OSError:
            continue
        section, i = "", 0
        while i < len(lines):
            h = HEADER.match(lines[i])
            if h:
                section = h.group(2).strip()
                i += 1
                continue
            m = ITEM.match(lines[i])
            if not m:
                i += 1
                continue
            done, parts = m.group(1).lower() == "x", [m.group(2)]
            i += 1
            while i < len(lines):
                nxt = lines[i]
                if not nxt.strip() or HEADER.match(nxt) or BULLET.match(nxt) or not nxt[:1].isspace():
                    break
                parts.append(nxt)
                i += 1
            text = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", " ".join(parts))
            out.append((section, done, re.sub(r"\s+", " ", text).strip()))
    return out


def _item_counts(stream: str) -> tuple[int, int]:
    """(open, done) item counts — the exact progress signal the box contract buys us."""
    items = _scan_items(stream)
    return sum(1 for _s, d, _t in items if not d), sum(1 for _s, d, _t in items if d)


def _open_items(stream: str, limit: int = 8, width: int = 300) -> list[str]:
    out = []
    for section, done, text in _scan_items(stream):
        if done:
            continue
        out.append(f"{section} · {text[:width]}" if section else text[:width])
        if len(out) >= limit:
            break
    return out


def _has_unreadable_plan(stream: str) -> bool:
    """A plan file with bullets but no checkbox at all — work the detector cannot see.

    This is the fail-open hole the contract closes: before P0, two `open` streams wrote
    prose-only todos, so the parser reported zero work and could never nudge on them. Silence
    is the failure mode that cost six days, so this is surfaced by --doctor and docs-check.
    """
    for name in ("todo.md", "remaining.md"):
        path = os.path.join(WORK, stream, name)
        if not os.path.exists(path):
            continue
        try:
            text = open(path, encoding="utf-8").read()
        except OSError:
            continue
        if not ITEM.search(text) and re.search(r"^\s*[-*]\s+(?!\[)\S", text, re.M):
            return True
    return False


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

    Arming first (a declaration), then the touch binding, then mtime (a guess). Each fallback
    is weaker than the last; the mtime tier exists only so a session that never named a stream
    still resolves one, and it can no longer nudge on its own (see `_do_nudge`).
    """
    armed = _armed(sid)
    if armed:
        return armed, "armed"
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


# ── arming (P2 / F6) ─────────────────────────────────────────────────────────
# The hook is OFF until the user says go. Arming replaces the planned "did the session edit a
# file this turn?" heuristic, which guessed at intent from side effects — the same mistake as
# inferring item state from prose. Disarmed sessions end their turn silently, always, so a
# question asked mid-stream can never drag the asker back into stream work.
def _armed(sid: str) -> str | None:
    if not sid:
        return None
    rec = _read_json(_state_path("armed", sid))
    stream = rec.get("stream")
    if stream and os.path.isdir(os.path.join(WORK, stream)):
        return stream
    return None


def _arm(sid: str, stream: str) -> None:
    _write_json(_state_path("armed", sid), {"stream": stream, "ts": time.time()})


def _disarm(sid: str, why: str = "") -> None:
    path = _state_path("armed", sid)
    if os.path.exists(path):
        try:
            os.remove(path)
        except OSError:
            pass
    _log_decision({"session": sid, "verdict": "disarm", "why": why})


# ── decision log (P1) ────────────────────────────────────────────────────────
def _log_decision(rec: dict) -> None:
    """Append every Stop verdict, so the next silent death is visible in a file.

    The six-day outage was invisible precisely because a no-op looks identical to a healthy
    'nothing to do'. A log makes "why did it let me stop 40 times?" an answerable question.
    """
    try:
        os.makedirs(STATE, exist_ok=True)
        rec = {"ts": time.strftime("%Y-%m-%dT%H:%M:%S"), **rec}
        with open(os.path.join(STATE, "decisions.jsonl"), "a", encoding="utf-8") as fh:
            fh.write(json.dumps(rec) + "\n")
    except Exception:
        pass  # telemetry must never break the hook


# ── progress signal (the guard) ──────────────────────────────────────────────
def _progress_sig(stream: str) -> str:
    """What "the session did something" looks like, cheaply.

    Was: the cksum of completed.md alone — so a phase that takes several turns before an entry
    lands read as *no progress*, and the guard released after a single nudge. That is precisely
    the "stops after every task" complaint, so the signal now includes the stream's own state
    files, HEAD, and the working tree.

    Leading the signature with the `(open, done)` box counts is the point of the P0 contract:
    ticking a box is now *unambiguous* progress, where before we could only infer it from file
    mtimes. The rest stays as a secondary signal for turns that are all code and no bookkeeping.
    """
    parts = ["boxes:%d/%d" % _item_counts(stream)]
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


# ── the brief (P3) ───────────────────────────────────────────────────────────
def _tail_entries(path: str, n: int, width: int = 220) -> list[str]:
    """The last n top-level bullets of an append-only log (completed/deviations)."""
    if not os.path.exists(path):
        return []
    try:
        lines = open(path, encoding="utf-8").read().splitlines()
    except OSError:
        return []
    out = []
    for i, line in enumerate(lines):
        if re.match(r"^[-*]\s+\S", line):
            parts = [line]
            for nxt in lines[i + 1:]:
                if not nxt.strip() or HEADER.match(nxt) or re.match(r"^[-*]\s", nxt):
                    break
                parts.append(nxt)
            t = re.sub(r"^[-*]\s+", "", " ".join(parts))
            t = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", t)
            out.append(re.sub(r"\s+", " ", t).strip()[:width])
    return out[-n:]


def brief(stream: str, next_n: int = 3) -> str:
    """The compact resume payload — what a session needs to continue without re-deriving.

    Resuming a stream by hand means reading its whole folder (45KB across 6 files for
    primitive-graph). That cost is paid on every resume and is lossy, which is why the nudge
    used to produce re-planning rather than execution. This assembles the same conclusion in
    a few hundred bytes: where we are, what is next, what is decided, what is in the way.
    """
    d = os.path.join(WORK, stream)
    items = _scan_items(stream)
    open_items = [(s, t) for s, done, t in items if not done]
    done_n = len(items) - len(open_items)
    phase = open_items[0][0] if open_items else ""
    L = [f"STREAM  docs/work/{stream}/   ({done_n}/{len(items)} items done · status={_status(stream) or '?'})"]
    if phase:
        L.append(f"PHASE   {phase}")
    L.append("")
    L.append(f"NEXT ({min(next_n, len(open_items))} of {len(open_items)} open):")
    for _s, t in open_items[:next_n]:
        L.append(f"  □ {t[:400]}")
    if len(open_items) > next_n:
        L.append(f"  … {len(open_items) - next_n} more open item(s)")

    blockers = _open_blockers(stream)
    if blockers:
        L.append("")
        L.append("OPEN BLOCKERS (needs the user — do not work around):")
        L += [f"  ! {b}" for b in blockers]

    recent = _tail_entries(os.path.join(d, "completed.md"), 2)
    if recent:
        L.append("")
        L.append("JUST LANDED:")
        L += [f"  ✓ {r}" for r in recent]

    devs = _tail_entries(os.path.join(d, "deviations.md"), 2)
    if devs:
        L.append("")
        L.append("RECENT DEVIATIONS:")
        L += [f"  ~ {r}" for r in devs]

    refs = [f for f in ("README.md", "forks.md", "issues.md") if os.path.exists(os.path.join(d, f))]
    L.append("")
    L.append("READ FOR CONTEXT: " + " · ".join(f"docs/work/{stream}/{f}" for f in refs))
    return "\n".join(L)


# ── the nudge ────────────────────────────────────────────────────────────────
def _nudge_text(stream: str, items: list[str], count: int) -> str:
    return (
        f"[work-check] PREMATURE PAUSE — you are ARMED on `docs/work/{stream}/` and it has open,\n"
        f"unblocked work. Do not hand control back; continue the plan through phase boundaries.\n"
        f"\n{brief(stream)}\n"
        f"\n"
        f"Stop only when one of these is true, and record it before you stop:\n"
        f"  · COMPLETE   — tick the item's box `- [x]` in todo.md; log the verification in\n"
        f"                 docs/work/{stream}/completed.md (items stay put — the box IS the move)\n"
        f"  · BLOCKED    — needs the user: add a row to docs/work/{stream}/blockers.md\n"
        f"  · PLAN ERROR — the plan is wrong and not trivially fixable: log it in\n"
        f"                 docs/work/{stream}/issues.md (or deviations.md) and raise it\n"
        f"  · OTHER      — write one line of why to docs/work/{stream}/.stop-reason\n"
        f"Any of those auto-disarms. (nudge {count}/{MAX_NUDGES} · `rd work disarm` to stop; "
        f"SKIP_WORK_CHECK=1 bypasses)"
    )


def _do_nudge(payload: dict) -> int:
    sid = str(payload.get("session_id") or "nosession")

    # Gate 0 — armed? An unarmed session is having a conversation, and a conversation must
    # always be allowed to end. This is the single biggest false-positive class removed.
    stream = _armed(sid)
    if not stream:
        _log_decision({"session": sid, "verdict": "allow", "why": "not armed"})
        return 0
    if _status(stream) in ("done", "closed"):
        _disarm(sid, f"stream status={_status(stream)}")
        return 0

    open_n, done_n = _item_counts(stream)
    blockers = _open_blockers(stream)
    reason = _has_stop_reason(stream)
    base = {"session": sid, "stream": stream, "open": open_n, "done": done_n}

    # The three real exits auto-disarm: finished, blocked, or a declared stop.
    if not open_n:
        _disarm(sid, "all items complete")
        _log_decision({**base, "verdict": "allow", "why": "complete"})
        return 0
    if blockers:
        _disarm(sid, f"blocker: {blockers[0][:80]}")
        _log_decision({**base, "verdict": "allow", "why": "blocked"})
        return 0
    if reason:
        _disarm(sid, "stop-reason recorded")
        _log_decision({**base, "verdict": "allow", "why": "stop-reason"})
        return 0

    path = _state_path("nudge", sid)
    prev = _read_json(path)
    sig = _progress_sig(stream)
    count = 1 if (prev.get("stream") != stream or prev.get("sig") != sig) else prev.get("count", 0) + 1

    if count > MAX_NUDGES:
        # Stalled. Record it where the NEXT session will see it, rather than releasing silently
        # and letting the same stall be rediscovered from scratch.
        _write_json(path, {"stream": stream, "sig": sig, "count": 0})
        note = os.path.join(WORK, stream, ".stop-reason")
        try:
            with open(note, "w", encoding="utf-8") as fh:
                fh.write(f"Auto-recorded {time.strftime('%Y-%m-%d %H:%M')}: released after "
                         f"{MAX_NUDGES} no-progress nudges with {open_n} item(s) still open. "
                         f"Investigate the stall before re-arming; delete this file to resume.\n")
        except OSError:
            pass
        _disarm(sid, "stalled")
        _log_decision({**base, "verdict": "release", "why": f"{MAX_NUDGES} no-progress nudges"})
        print(
            f"[work-check] '{stream}' still has open work but no progress across {MAX_NUDGES} "
            f"nudges — letting the turn end and DISARMING.\n"
            f"  Recorded the stall in docs/work/{stream}/.stop-reason. Record a blocker if this "
            f"needs the user, then re-arm with `rd work arm {stream}`.",
            file=sys.stderr,
        )
        return 0

    _write_json(path, {"stream": stream, "sig": sig, "count": count, "how": "armed"})
    _log_decision({**base, "verdict": "nudge", "why": f"{open_n} open", "nudge": count})
    print(_nudge_text(stream, _open_items(stream), count), file=sys.stderr)
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


# ── doctor (P1) ──────────────────────────────────────────────────────────────
def doctor(quiet: bool = False) -> int:
    """What the detector actually sees, per stream — the drift detector.

    Every one of the eight classification bugs was invisible from the code and only showed up
    by measuring the corpus. This makes that measurement a command, so the next dialect drift
    is a visible row rather than six weeks of silence.
    """
    rows, problems = [], 0
    for s in sorted(_all_streams()):
        st = _status(s)
        op, dn = _item_counts(s)
        bl = len(_open_blockers(s))
        sr = "yes" if _has_stop_reason(s) else "-"
        flag = ""
        if _has_unreadable_plan(s):
            flag, problems = "PARSE?", problems + 1
        elif st not in ("done", "closed", "delivered") and op == 0 and dn == 0:
            flag = "no-items"
        rows.append((s, st or "?", op, dn, bl, sr, flag))
    if not quiet:
        print(f"{'stream':34} {'status':9} {'open':>4} {'done':>4} {'blk':>3} {'stop':>4}  flag")
        for r in rows:
            print(f"{r[0]:34} {r[1]:9} {r[2]:4} {r[3]:4} {r[4]:3} {r[5]:>4}  {r[6]}")
        print(f"\n{len(rows)} streams · {problems} unreadable plan(s)")
    return problems


# ── entry ────────────────────────────────────────────────────────────────────
def _cli_work(argv: list[str], sid: str) -> int:
    """`rd work <arm|disarm|status|brief|doctor>` — the user-facing surface."""
    sub = argv[0] if argv else "status"
    rest = argv[1:]

    if sub == "arm":
        if not rest:
            print("usage: rd work arm <stream>", file=sys.stderr)
            return 1
        stream = rest[0].strip("/").split("/")[-1] if "/" in rest[0] else rest[0]
        if not os.path.isdir(os.path.join(WORK, stream)):
            print(f"no such work stream: {stream}", file=sys.stderr)
            return 1
        _arm(sid, stream)
        _log_decision({"session": sid, "stream": stream, "verdict": "arm"})
        op, dn = _item_counts(stream)
        print(f"ARMED on {stream} ({op} open / {dn} done). The Stop hook will now keep this "
              f"session going until complete, blocked, or stalled.\n")
        print(brief(stream))
        return 0

    if sub == "disarm":
        cur = _armed(sid)
        _disarm(sid, "manual")
        print(f"disarmed{f' (was {cur})' if cur else ''}.")
        return 0

    if sub == "brief":
        stream = rest[0] if rest else (_armed(sid) or _active_stream(sid)[0])
        if not stream or not os.path.isdir(os.path.join(WORK, stream)):
            print("usage: rd work brief <stream>", file=sys.stderr)
            return 1
        print(brief(stream))
        return 0

    if sub == "doctor":
        return 0 if doctor() == 0 else 0  # advisory: report, never fail the shell

    # status
    cur = _armed(sid)
    if cur:
        op, dn = _item_counts(cur)
        n = _read_json(_state_path("nudge", sid)).get("count", 0)
        print(f"ARMED on {cur} — {op} open / {dn} done, nudge {n}/{MAX_NUDGES}")
    else:
        stream, how = _active_stream(sid)
        print(f"disarmed (conversation mode). Nearest stream: {stream or 'none'}"
              f"{f' via {how}' if stream else ''}.")
    return 0


def main(argv: list[str]) -> int:
    if argv and argv[0] in ("arm", "disarm", "status", "brief", "doctor"):
        sid = os.environ.get("CLAUDE_SESSION_ID", "cli")
        if "--session" in argv:
            i = argv.index("--session")
            sid = argv[i + 1] if i + 1 < len(argv) else sid
            argv = argv[:i] + argv[i + 2:]
        return _cli_work(argv, sid)

    if "--doctor" in argv:
        doctor()
        return 0

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
