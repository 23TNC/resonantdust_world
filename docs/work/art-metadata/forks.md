# Forks — art-metadata

---

## F1 · Outline generation: port the Rust `geometry` crate, not a Python reimpl (2026-07-18)

**Context.** P2 needs `alpha → contours → simplify → earcut` to write the shadow outline. The art
pipeline is Python (`bin/lib/*.py`), but the toolchain has **only numpy + PIL** — no `cv2`, `scipy`,
`shapely`, `mapbox_earcut`, or `triangle`.

**Options.** (a) reimplement contours + Visvalingam + **earcut** in pure numpy/Python (earcut is
non-trivial and error-prone); (b) add heavy Python deps (opencv + an earcut) to the art toolchain;
(c) **port the old game's `resonantdust-geometry` Rust crate** (`../resonantdust/shared/geometry`) —
pure Rust, minimal deps (`serde` + `image` png-only + `earcutr`, all build in `rust:slim`),
self-contained (`mask`/`contour`/`simplify`/`triangulate`, ~22 KB), with a clean `generate(&png,
&Options) -> Sidecar` and types (`Sidecar`/`Polygon`) already designed to cross gate→wasm→view as JSON.

**Decision: (c).** It's the proven implementation, dependency-light, and the **types are reusable by
the wasm client** (the outline's runtime consumer) — the same crate serves generation (feature-gated)
and the view. `bin/art` shells out to a small CLI binary (like it already shells out to `laigter`); a
thin Python wrapper (`outline.py`) merges the emitted JSON into `meta.json` via `meta.update`, keeping
sidecar ownership in one place. New home: `shared/geometry` (mirrors the old layout).
