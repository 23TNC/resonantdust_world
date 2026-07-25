#!/usr/bin/env python3
"""docs_check — the docs-authority audit engine (invoked by `rd docs-check`).

Enforces the invariants that keep docs/ a trustworthy external memory, so the
convention in docs/CONVENTIONS.md stays *obeyed* rather than drifting. Every
check prints file:line on failure and the process exits non-zero if any ERROR
fires, so a Stop hook / git pre-commit can gate on it. WARNINGs never fail the
run.

Invariants (see docs/work/docs-authority/{todo,deviations}.md for the why):

  1 decay-markers   no markup-level decay in authoritative files. Strict tier
                    (design/** + VARIABLES/TABLES/ACTIONS): emphasis-wrapped decay
                    words (**deprecated** …), the phrase "superseded by", and bare
                    "deprecated" in the three shapes-only top files. Lenient tier
                    (intent/**): only emphasis-decay. `~~` strikethrough banned in
                    ALL docs. `☠` banned except in map README files. Matches inside
                    inline-code / fenced code are ignored (a doc must be able to
                    *name* the pattern it bans).
  2 ref-integrity   every  \\w+_reference  used in TABLES.md is defined in VARIABLES.md.
  3 link-integrity  every relative markdown link resolves to a real file.
  4 freshness       every current/** file + the component map + intent index carry a
                    parseable stamp (`verified @ <sha>` or `Last mapped/updated: DATE`).
  5 work-linkage    every work/<w>/ has a README and is linked from work/README.md,
                    and every work link in that index resolves. Bidirectional.
  6 map-bijection   every component folder (one with a README) is linked from the
                    component map, and every components/ link in the map resolves.
  7 work-items      a work/<w>/{todo,remaining}.md with plain bullets but NO checkbox
                    is an ERROR — its work is invisible to the continuation hook, the
                    fail-open mode that let that hook sit dead for six days. Items
                    over 250 chars (wrapped lines folded in) warn as likely phases
                    rather than actions; `--items` lists them individually instead of
                    the one aggregate line.
"""
from __future__ import annotations
import os
import re
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DOCS = os.path.join(REPO, "docs")

TOP_AUTH = {"VARIABLES.md", "TABLES.md", "ACTIONS.md"}  # shapes-only, strictest

# Set from argv: list every oversized item instead of one aggregate line.
VERBOSE_ITEMS = False

errors: list[str] = []
warnings: list[str] = []


def err(path: str, line: int, msg: str) -> None:
    errors.append(f"{rel(path)}:{line}: {msg}")


def warn(path: str, line: int, msg: str) -> None:
    warnings.append(f"{rel(path)}:{line}: {msg}")


def rel(path: str) -> str:
    return os.path.relpath(path, REPO)


def md_files() -> list[str]:
    out = []
    for root, _dirs, files in os.walk(DOCS):
        for f in files:
            if f.endswith(".md"):
                out.append(os.path.join(root, f))
    return sorted(out)


INLINE_CODE = re.compile(r"`[^`]*`")


def strip_inline_code(text: str) -> str:
    return INLINE_CODE.sub("", text)


def lines_no_code(path: str):
    """Yield (lineno, raw, prose) where prose has inline-code removed and lines
    inside ``` fences are blanked — so content checks never fire on examples."""
    in_fence = False
    with open(path, encoding="utf-8") as fh:
        for i, raw in enumerate(fh, 1):
            raw = raw.rstrip("\n")
            if raw.lstrip().startswith("```"):
                in_fence = not in_fence
                yield i, raw, ""
                continue
            prose = "" if in_fence else strip_inline_code(raw)
            yield i, raw, prose


# ── check 1 · decay markers ──────────────────────────────────────────────────
# Only a decay word that is *wholly* emphasized is a status label: `**stale**`,
# `**superseded.**`. `**Stale prune**` (a feature name) is prose — not flagged.
EMPH_DECAY = re.compile(r"\*\*(deprecated|superseded|stale|obsolete|legacy)[.!,]?\*\*", re.I)
SUPERSEDED_BY = re.compile(r"\bsuperseded by\b", re.I)
BARE_DEPRECATED = re.compile(r"\bdeprecated\b", re.I)
STRIKE = re.compile(r"~~")
TOMBSTONE = "☠"


def check_decay() -> None:
    for path in md_files():
        base = os.path.basename(path)
        r = rel(path)
        is_design = "/design/" in r.replace(os.sep, "/")
        is_top = base in TOP_AUTH and os.path.dirname(r) == "docs"
        is_intent = r.replace(os.sep, "/").startswith("docs/intent/")
        is_map_readme = base == "README.md"
        strict = is_design or is_top
        for ln, _raw, prose in lines_no_code(path):
            if not prose.strip():
                continue
            if STRIKE.search(prose):
                err(path, ln, "strikethrough `~~` — move the item between state-files, don't annotate")
            if TOMBSTONE in prose and not is_map_readme:
                err(path, ln, "`☠` tombstone outside a map README — delete it (git is the history)")
            if strict or is_intent:
                m = EMPH_DECAY.search(prose)
                if m:
                    err(path, ln, f"emphasis-wrapped decay label '**{m.group(1)}**' — authoritative docs hold current truth only")
            if strict:
                if SUPERSEDED_BY.search(prose):
                    err(path, ln, "'superseded by' in an authoritative doc — describe the current shape, not what it replaced")
            if is_top:
                if BARE_DEPRECATED.search(prose):
                    err(path, ln, "'deprecated' in a shapes-only authority file — state the current allocation (e.g. 'reserved')")


# ── check 2 · reference integrity ────────────────────────────────────────────
REF = re.compile(r"\b([a-z][a-z0-9_]*_reference)\b")


def check_refs() -> None:
    tpath = os.path.join(DOCS, "TABLES.md")
    vpath = os.path.join(DOCS, "VARIABLES.md")
    if not (os.path.exists(tpath) and os.path.exists(vpath)):
        return
    vtext = open(vpath, encoding="utf-8").read()
    defined = set(REF.findall(vtext))
    with open(tpath, encoding="utf-8") as fh:
        for ln, raw in enumerate(fh, 1):
            # A reference the line itself marks not-yet-real is intentionally undefined.
            if re.search(r"\b(deferred|reserved)\b", raw, re.I):
                continue
            for tok in REF.findall(raw):
                if tok not in defined:
                    err(tpath, ln, f"`{tok}` used in TABLES.md but not defined in VARIABLES.md")


# ── check 3 · link integrity ─────────────────────────────────────────────────
LINK = re.compile(r"\]\(([^)]+)\)")


def check_links() -> None:
    for path in md_files():
        d = os.path.dirname(path)
        for ln, _raw, prose in lines_no_code(path):
            for target in LINK.findall(prose):
                target = target.strip()
                if not target or target.startswith(("http://", "https://", "mailto:", "#")):
                    continue
                # strip a "title" and any #anchor
                target = target.split(" ", 1)[0].split("#", 1)[0]
                if not target:
                    continue
                dest = os.path.normpath(os.path.join(d, target))
                if not os.path.exists(dest):
                    err(path, ln, f"broken link → {target}")


# ── check 4 · freshness stamps ───────────────────────────────────────────────
STAMP = re.compile(r"verified\s*@\s*\S+|Last (mapped|updated)\s*:\s*\d{4}-\d{2}-\d{2}", re.I)
STAMP_DATE = re.compile(r"Last (?:mapped|updated)\s*:\s*(\d{4}-\d{2}-\d{2})", re.I)
VERIFIED_SHA = re.compile(r"verified\s*@\s*([0-9a-fA-F]{7,40})")
PATH_HINT = re.compile(r"Path:\s*`([^`]+)`")


def _git_date(args: list[str]) -> str | None:
    """Best-effort `git log -1 --date=short` → YYYY-MM-DD, or None on any failure.
    Warning-only feature: never let git trouble break the audit."""
    try:
        r = subprocess.run(["git", "log", "-1", "--format=%cd", "--date=short", *args],
                           cwd=REPO, capture_output=True, text=True, timeout=5)
        return r.stdout.strip() or None if r.returncode == 0 else None
    except Exception:
        return None


def _stamp_date(text: str) -> str | None:
    m = STAMP_DATE.search(text)
    if m:
        return m.group(1)
    m = VERIFIED_SHA.search(text)
    return _git_date([m.group(1)]) if m else None


def _component_root(path: str) -> str | None:
    parts = path.split(os.sep)
    if "current" not in parts:
        return None
    idx = len(parts) - 1 - parts[::-1].index("current")
    return os.sep.join(parts[:idx])


def check_freshness() -> None:
    targets = []
    for path in md_files():
        r = rel(path).replace(os.sep, "/")
        if "/current/" in r or r == "docs/components/README.md" or r == "docs/intent/README.md":
            targets.append(path)
    for path in targets:
        text = open(path, encoding="utf-8").read()
        if not STAMP.search(text):
            err(path, 1, "missing freshness stamp (`verified @ <sha>` or `Last updated: YYYY-MM-DD`) — a cache with no age is unsafe to plan from")
            continue
        # Staleness comparison (warning-only): if the component's code changed after
        # the stamp, the current/ cache may lag — re-verify before planning from it.
        # Reuses the component README's `Path: `<code>`` hint; skips silently if absent.
        root = _component_root(path)
        if not root:
            continue
        readme = os.path.join(root, "README.md")
        if not os.path.exists(readme):
            continue
        hint = PATH_HINT.search(open(readme, encoding="utf-8").read())
        if not hint:
            continue
        stamped = _stamp_date(text)
        changed = _git_date(["--", os.path.join(REPO, hint.group(1))])
        if stamped and changed and changed > stamped:
            warn(path, 1, f"current/ stamped {stamped} but code ({hint.group(1)}) changed {changed} — re-verify before planning from it")


# ── check 5 · work-index linkage ─────────────────────────────────────────────
def check_work() -> None:
    work = os.path.join(DOCS, "work")
    index = os.path.join(work, "README.md")
    if not os.path.isdir(work):
        return
    if not os.path.exists(index):
        err(index, 1, "no work index (work/README.md)")
        return
    itext = open(index, encoding="utf-8").read()
    linked = set(LINK.findall(strip_inline_code(itext)))
    for name in sorted(os.listdir(work)):
        d = os.path.join(work, name)
        if not os.path.isdir(d):
            continue
        if not os.path.exists(os.path.join(d, "README.md")):
            err(os.path.join(d, "README.md"), 1, f"work stream '{name}' has no README.md")
        if not any(t.split("#")[0].strip().startswith(name + "/") or t.strip() == name for t in linked):
            err(index, 1, f"work stream '{name}' not linked from the work index")


# ── check 7 · work items are machine-readable ────────────────────────────────
ITEM_RE = re.compile(r"^\s*[-*]\s+\[([ xX])\]\s*(.*)$")
PLAIN_BULLET_RE = re.compile(r"^\s*[-*]\s+(?!\[[ xX]\])\S")


def check_work_items() -> None:
    """A plan file's items must be checkboxes — the only thing the continuation hook can read.

    ERROR: a `todo.md`/`remaining.md` with plain bullets but no checkbox anywhere. That file's
    work is *invisible* to the hook, which then silently allows a stop — the fail-open mode that
    let the continuation check sit dead for six days. Prose-only files are fine (no items claimed).

    WARNING: an item too coarse to execute (>250 chars). A vague next step is itself a reason to
    check in — the session must re-plan before it can act, and planning invites ratification,
    which is a stop. Advisory only: the corpus has dozens today and blocking would help nobody.
    """
    work = os.path.join(DOCS, "work")
    if not os.path.isdir(work):
        return
    coarse = 0
    per_stream: dict[str, int] = {}
    for name in sorted(os.listdir(work)):
        d = os.path.join(work, name)
        if not os.path.isdir(d):
            continue
        for fname in ("todo.md", "remaining.md"):
            path = os.path.join(d, fname)
            if not os.path.exists(path):
                continue
            text = open(path, encoding="utf-8").read()
            lines = text.splitlines()
            has_item = any(ITEM_RE.match(l) for l in lines)
            if not has_item and any(PLAIN_BULLET_RE.match(l) for l in lines):
                err(path, 1, "plan items must be checkboxes (`- [ ]` / `- [x]`) — this file's "
                             "bullets are invisible to the continuation hook")
                continue
            # Measure the WHOLE item, folding its wrapped continuation lines: 81% of items wrap,
            # so measuring the first line alone found nothing at all on the first run.
            i = 0
            while i < len(lines):
                m = ITEM_RE.match(lines[i])
                if not m:
                    i += 1
                    continue
                start, parts = i + 1, [m.group(2)]
                i += 1
                while i < len(lines):
                    nxt = lines[i]
                    if (not nxt.strip() or nxt.startswith("#")
                            or re.match(r"^\s*[-*]\s", nxt) or not nxt[:1].isspace()):
                        break
                    parts.append(nxt.strip())
                    i += 1
                size = len(" ".join(parts))
                if size > 250:
                    coarse += 1
                    per_stream[name] = per_stream.get(name, 0) + 1
                    if VERBOSE_ITEMS:
                        warn(path, start, f"item is {size} chars — likely a phase, not an action; "
                                          f"decompose it (`/rd-plan`) so it can be executed")
    # ONE aggregate line by default. Listing all 102 individually buried the real warnings and
    # would have trained us to skim past them — which is how the last silent failure survived.
    # `--items` prints them all when you actually want to go decompose.
    if coarse and not VERBOSE_ITEMS:
        top = sorted(per_stream.items(), key=lambda kv: -kv[1])[:3]
        where = ", ".join(f"{s} ({n})" for s, n in top)
        warn(os.path.join(work, "README.md"), 1,
             f"{coarse} plan item(s) over 250 chars — likely phases, not actions; worst: {where}. "
             f"These make a resumed session re-plan instead of execute. "
             f"`rd docs-check --items` to list them; `/rd-plan` to decompose.")


# ── check 6 · map ↔ folder bijection ─────────────────────────────────────────
def check_map() -> None:
    comp = os.path.join(DOCS, "components")
    mapf = os.path.join(comp, "README.md")
    if not (os.path.isdir(comp) and os.path.exists(mapf)):
        return
    mtext = open(mapf, encoding="utf-8").read()  # raw: a component may be named in prose or code
    subfolders = {"design", "intent", "current", "plan"}
    for root, _dirs, files in os.walk(comp):
        if root == comp:
            continue
        if "README.md" not in files:
            continue
        relpath = os.path.relpath(root, comp).replace(os.sep, "/")
        # {design,intent,current,plan} dirs are *inside* a component, not components.
        if any(seg in subfolders for seg in relpath.split("/")):
            continue
        if relpath not in mtext:
            warn(mapf, 1, f"component folder '{relpath}' has a README but isn't named in the component map")


CHECKS = [
    ("decay-markers", check_decay),
    ("ref-integrity", check_refs),
    ("link-integrity", check_links),
    ("freshness", check_freshness),
    ("work-linkage", check_work),
    ("work-items", check_work_items),
    ("map-bijection", check_map),
]


def main(argv: list[str]) -> int:
    global VERBOSE_ITEMS
    VERBOSE_ITEMS = "--items" in argv
    if "--list" in argv:
        print("docs-check invariants:")
        for name, fn in CHECKS:
            doc = (fn.__doc__ or "").strip().splitlines()[0] if fn.__doc__ else ""
            print(f"  {name}")
        print("\nSee docs/work/docs-authority/todo.md (P2) for the full spec.")
        return 0
    quiet = "--quiet" in argv
    for _name, fn in CHECKS:
        fn()
    if errors:
        print(f"[docs-check] {len(errors)} error(s):", file=sys.stderr)
        for e in errors:
            print(f"  ✗ {e}", file=sys.stderr)
    if warnings and not quiet:
        print(f"[docs-check] {len(warnings)} warning(s):", file=sys.stderr)
        for w in warnings:
            print(f"  ! {w}", file=sys.stderr)
    if errors:
        return 1
    if not quiet:
        print(f"[docs-check] clean ✓  ({len(md_files())} files, {len(warnings)} warning(s))")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
