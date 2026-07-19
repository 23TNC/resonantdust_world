# Completed — texture-restructure

_Done + verified. Items move here from [`todo.md`](todo.md) (`todo → completed`). Append-only history;
authoritative for what's actually shipped. Timestamp each row with the date it landed._

---

- **2026-07-19** · **P1 (part) — `texpath.py` leaf reshape.** Rewrote [`bin/lib/texpath.py`](../../../bin/lib/texpath.py)
  to the new leaf `<variant>/<map>.<dir>.<part>.<ext>` — drop `<id>` + `<subkind>`; `<part>` replaces the
  old `<layer>` filename segment; added `parse_map` + `leaf_file` (per-variant `meta.json`/`atlas.json`,
  which carry no dir/part, distinct from directional `sibling`). Updated consumers: `generate.py`
  (`variant_leaf`/`map_name`), `meta.py` (`meta.json` via `leaf_file`). Deleted the dead
  `bin/lib/migrate_texpaths.py` (0.1→0.2 tool, done 2026-07-04; git holds it; precedent mentions →
  git history). **Unit-tested** the new API (round-trips, `find_maps`, `sibling` vs `leaf_file`); all
  five consumers import clean.

- **2026-07-19** · **P1 (part) — `marigold/delight.py` mirror.** Mirrored the new leaf natively in the
  marigold venv: `find_diffuse` globs `diffuse.<dir>.<part>.png`, `out_path` emits `<target>.<dir>.<part>.png`
  at the same dir/part (+ `_diffuse_dir_part`/`_is_diffuse` helpers). `find_diffuse`/`out_path` signatures
  unchanged, so `normals.py`/`depth.py`/`normal_depth.py` are untouched. Syntax-checked all four; verified
  the path logic **agrees with `texpath.py`**. **Remaining in P1:** only the biome-tile fold in texpath
  (sequenced after the P0 registry).
