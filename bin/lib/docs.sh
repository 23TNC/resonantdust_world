#!/usr/bin/env bash
# rd docs — the docs-authority audits. Thin wrappers over the python engines in
# bin/lib/{docs_check,work_check}.py, which do the real work (link resolution,
# code-span handling, regex invariants). Sourced by bin/rd; see
# docs/work/docs-authority/ for what these enforce and why.

# rd docs-check [--quiet] [--list]
#   --quiet : print only on failure (for the Stop hook)
#   --list  : print the invariant names and exit
rd_docs_check() {
  python3 "$RD_LIB_DIR/docs_check.py" "$@"
}

# rd work-check [--quiet]
#   Flags a *silent premature pause* — open executable work in the active stream
#   with no blocker / user-fork / recorded stop-reason. (Implemented in P5.)
rd_work_check() {
  if [[ -f "$RD_LIB_DIR/work_check.py" ]]; then
    python3 "$RD_LIB_DIR/work_check.py" "$@"
  else
    rd_warn "work-check not yet built (docs-authority P5)"
    return 0
  fi
}
