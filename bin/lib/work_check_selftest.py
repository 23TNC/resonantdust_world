#!/usr/bin/env python3
"""work_check_selftest — assert the classifier against the REAL corpus.

Every one of the eight classification bugs in this detector was invisible from the code and only
showed up by measuring `docs/work/`. So the corpus *is* the test fixture: this asserts the
properties that must hold across all streams, plus synthetic cases for the exact shapes that
previously fooled it. Run it after any change to `work_check.py`'s parsing.

    python3 bin/lib/work_check_selftest.py
"""
from __future__ import annotations
import os
import re
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import work_check as w  # noqa: E402

fails: list[str] = []


def check(name: str, cond: bool, detail: str = "") -> None:
    if cond:
        print(f"  ok   {name}")
    else:
        fails.append(f"{name} — {detail}")
        print(f"  FAIL {name}: {detail}")


def corpus_checks() -> None:
    print("corpus:")
    streams = w._all_streams()
    check("streams found", len(streams) > 10, f"only {len(streams)}")

    # 1 — no ticked box is ever reported as open work (the shard-tables phantom, 11 items).
    # Assert on the parsed box state, NOT on the item's text: a plan may legitimately *mention*
    # "[x]" in prose (this stream's own todo does), and matching that was a bug in this test.
    bad = []
    for s in streams:
        want = sum(1 for _sec, done, _t in w._scan_items(s) if not done)
        got = len(w._open_items(s, limit=10**6))
        if want != got:
            bad.append(f"{s}: scan says {want} open, _open_items returned {got}")
    check("open list == unticked items", not bad, "; ".join(bad[:3]))

    # 2 — every plan file is machine-readable (the prose-only fail-open hole).
    unreadable = [s for s in streams if w._has_unreadable_plan(s)]
    check("no unreadable plan files", not unreadable, ", ".join(unreadable))

    # 3 — counts agree with a naive independent recount (catches fold/continuation bugs).
    for s in streams:
        op, dn = w._item_counts(s)
        raw_o = raw_d = 0
        for name in ("todo.md", "remaining.md"):
            p = os.path.join(w.WORK, s, name)
            if not os.path.exists(p):
                continue
            for line in open(p, encoding="utf-8"):
                m = re.match(r"^\s*[-*]\s+\[([ xX])\]", line)
                if m:
                    if m.group(1).lower() == "x":
                        raw_d += 1
                    else:
                        raw_o += 1
        if (op, dn) != (raw_o, raw_d):
            check(f"counts match for {s}", False, f"parser={op}/{dn} naive={raw_o}/{raw_d}")
            return
    check("item counts match a naive recount", True)

    # 4 — a brief is always producible and bounded.
    big = [s for s in streams if len(w.brief(s)) > 6000]
    check("briefs stay compact (<6KB)", not big, ", ".join(big))


def synthetic_checks() -> None:
    """The exact shapes that fooled earlier versions, pinned so they can't regress."""
    print("synthetic (previously-fooling shapes):")
    with tempfile.TemporaryDirectory() as tmp:
        orig = w.WORK
        w.WORK = tmp
        try:
            def mk(name: str, blockers: str = "", todo: str = "") -> str:
                d = os.path.join(tmp, name)
                os.makedirs(d, exist_ok=True)
                if blockers:
                    open(os.path.join(d, "blockers.md"), "w", encoding="utf-8").write(blockers)
                if todo:
                    open(os.path.join(d, "todo.md"), "w", encoding="utf-8").write(todo)
                return name

            # Bug: lowercase "resolved" in a title closed a blocker whose header says OPEN.
            s = mk("b_open", blockers="# B\n\n## B3 — `resolved_zone` alongside resolved tile (2026-07-25, OPEN)\n\nProse, no bullet.\n")
            check("prose blocker marked OPEN is open", w._has_open_blocker(s), str(w._open_blockers(s)))

            s = mk("b_res", blockers="# B\n\n## B1 — layout ✅ RESOLVED by the user\n\nProse.\n")
            check("✅ RESOLVED blocker is closed", not w._has_open_blocker(s))

            # Bug: "None open." lives in the BODY, not the header.
            s = mk("b_none", blockers="# B\n\n## Open\n\nNone open.\n\n## Resolved\n\n- old thing\n")
            check("'None open.' body is not a blocker", not w._has_open_blocker(s))

            # Bug: [x] counted as open.
            s = mk("i_done", todo="# T\n\n## P1\n\n- [x] done thing\n- [ ] open thing\n")
            check("counts split on the box", w._item_counts(s) == (1, 1), str(w._item_counts(s)))

            # Bug: wrapped continuation lines dropped (81% of items).
            s = mk("i_wrap", todo="# T\n\n## P1\n\n- [ ] first line of the item\n      second line of it\n")
            got = w._open_items(s)[0]
            check("wrapped lines are folded in", "second line" in got, got)

            # A checkbox under a DONE-marked header is still governed by its box, not the header.
            s = mk("i_hdr", todo="# T\n\n## P1 — ✅ DONE\n\n- [ ] genuinely deferred item\n")
            check("box beats a DONE header", w._item_counts(s) == (1, 0), str(w._item_counts(s)))

            # Prose-only plan with bullets = unreadable (must be surfaced, not silently zero).
            s = mk("i_prose", todo="# T\n\n## P1\n\n- a plain bullet item\n")
            check("bullets without boxes flagged unreadable", w._has_unreadable_plan(s))

            # Pure prose (no bullets at all) is legitimately item-free, not a parse failure.
            s = mk("i_none", todo="# T\n\nAll phases complete; see completed.md.\n")
            check("pure prose is not flagged unreadable", not w._has_unreadable_plan(s))
        finally:
            w.WORK = orig


def main() -> int:
    corpus_checks()
    synthetic_checks()
    print()
    if fails:
        print(f"FAILED ({len(fails)}):")
        for f in fails:
            print(f"  - {f}")
        return 1
    print("all work_check self-tests passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
