#!/usr/bin/env bash
# rd content-check — the one rule about `content/`, enforced.
#
# **A tracked file under `content/` is a `*.toml` file a human wrote to describe the world.**
# Nothing a tool emits, and nothing describing where servers live, belongs there. That rule is
# the whole point of the `content-toml-only` stream; this is what keeps it true after the stream
# closes. Sourced by `bin/rd`; assumes common.sh is sourced.
#
# Why it checks the TRACKED SET and not the loader: `read_content_dir` is flat and TOML-only, so
# a stray file is INVISIBLE to it — it would never fail a build, it would just sit there
# accumulating company. Reading the git index also gives the pre-commit hook exactly the
# semantics it wants: a staged `content/x.json` is caught before it lands, and an untracked
# scratch file in `content/` is left alone.
#
# History (the cost of not having this): `content/` accreted three generated ART manifests, a
# generated R2 key index that went stale and broke the deployed content path, and four
# deployment-topology files. Every one of them looked reasonable the day it was added.
#
#   rd content-check            report offenders, exit 1 if any
#   rd content-check --quiet    print only on failure (for hooks)
rd_content_check() {
  local quiet=0
  [[ "${1:-}" == "--quiet" ]] && quiet=1

  local -a bad=()
  local f
  while IFS= read -r f; do
    [[ -n "$f" ]] && bad+=("$f")
  done < <(git -C "$REPO" ls-files content | grep -v '\.toml$' || true)

  if (( ${#bad[@]} > 0 )); then
    {
      echo "[content-check] ${#bad[@]} non-TOML file(s) tracked under content/:"
      for f in "${bad[@]}"; do echo "  ✗ $f"; done
      echo
      echo "content/ holds the authored TOML corpus and nothing else."
      echo "  generated index  → beside the tree it indexes (e.g. textures/manifest/)"
      echo "  deploy topology  → deploy/"
      echo "See docs/work/2026-08-04-content-toml-only/forks.md (F1/F3)."
    } >&2
    return 1
  fi

  local n; n=$(git -C "$REPO" ls-files content | wc -l)
  (( quiet )) || echo "[content-check] clean ✓  ($n corpus file(s), all *.toml)"
  return 0
}
