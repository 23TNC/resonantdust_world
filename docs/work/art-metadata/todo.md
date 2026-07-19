# Todo — art-metadata

_Items move to `completed.md` as they land + verify. Design: [`README`](README.md)._

---

## P1 · Sidecar infrastructure + channel tints — ✅ DONE

- [x] `bin/lib/meta.py` — the extensible `meta.json` sidecar: `load`/`update(map_path, **keys)`,
      read-merge-write, co-located via `texpath.sibling(alb, "meta", ext="json")`.
- [x] `split_layers.py` emits `channel_tints` (the per-channel `Bcol·255` it already computed, was only
      logged). **Verified:** `wall.smooth` → `meta.json` = `{"channel_tints":[[244,244,243]]}`.

## P2 · Outline generation (the tiered-lighting dependency) — ✅ DONE

- [x] **Deps (F1):** toolchain had only numpy+PIL (no earcut), so **ported the Rust `geometry` crate**
      (`shared/geometry`, added to the workspace) + a CLI `src/bin/outline.rs` — `sprite PNG → Sidecar
      JSON`. Built in the rust:slim builder; runs natively on the host.
- [x] `bin/lib/outline.py` — runs the CLI per leaf on **`diffuse.png`** (its alpha is the silhouette —
      things are RGBA cutouts; opaque tiles are `L` → harmless square, and don't cast). Merges the
      `Sidecar` (polygons + earcut triangulation, content-box-normalized) into `meta.json` via
      `meta.update`. Skip-if-present + `--force`.
- [x] Wired into `cmd_maps` (after `_surface_kind`; guarded on the CLI being built → skip-with-hint,
      never fails maps) + a standalone `bin/art outline` dispatch.
- [x] **Verified:** conifer leaves → 163–183-tri silhouettes hugging the shape (contour `x[0.22,0.77]`,
      not a square); read-merge-write proven — wall meta has **both** `channel_tints` + `outline`.

## P3 · Serve + consume

- [ ] The edge/content path folds `meta.json` into the manifest (like `atlas.json`) so the client gets
      `channel_tints` + `outline` per sprite. (Confirm the current manifest fold picks up new sidecars.)
- [ ] Client reads `channel_tints` for reconstruction (replaces however tints are sourced today) and
      `outline` for the lighting scatter (the lighting stream consumes it).

---

**Done when:** `bin/art maps` writes a `meta.json` per leaf carrying `channel_tints` + `outline`, folded
into the manifest, ready for the tiered-lighting port to consume.
