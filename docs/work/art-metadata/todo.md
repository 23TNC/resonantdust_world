# Todo — art-metadata

_Items move to `completed.md` as they land + verify. Design: [`README`](README.md)._

---

## P1 · Sidecar infrastructure + channel tints — ✅ DONE

- [x] `bin/lib/meta.py` — the extensible `meta.json` sidecar: `load`/`update(map_path, **keys)`,
      read-merge-write, co-located via `texpath.sibling(alb, "meta", ext="json")`.
- [x] `split_layers.py` emits `channel_tints` (the per-channel `Bcol·255` it already computed, was only
      logged). **Verified:** `wall.smooth` → `meta.json` = `{"channel_tints":[[244,244,243]]}`.

## P2 · Outline generation (the tiered-lighting dependency)

- [ ] **Deps** — pick the Python path (`cv2.findContours` / `scipy` / pure-numpy for contours; a
      simplify; an earcut — `mapbox_earcut` / `triangle` / pure-py). Check what's available in the art
      toolchain before committing; keep it dependency-light if possible.
- [ ] `bin/lib/outline.py` — from the sprite's **silhouette** (surface `A` / diffuse alpha): threshold →
      contours (incl. holes) → Visvalingam–Whyatt (or Douglas–Peucker) simplify → group holes into
      outers → earcut. Normalize to the content box (so it's resolution-independent, like the old
      sidecars). Write `outline: { polygons, triangles }` via `meta.update`.
- [ ] Wire into `cmd_maps` (run after surface is baked, so the silhouette exists). Skip-if-present +
      `--force`, matching the other generators.
- [ ] Verify on a tree/rock: reasonable triangle count (Visvalingam target), holes excluded, outline
      hugs the silhouette. Port reference: `../resonantdust/shared/geometry/src/lib.rs`.

## P3 · Serve + consume

- [ ] The edge/content path folds `meta.json` into the manifest (like `atlas.json`) so the client gets
      `channel_tints` + `outline` per sprite. (Confirm the current manifest fold picks up new sidecars.)
- [ ] Client reads `channel_tints` for reconstruction (replaces however tints are sourced today) and
      `outline` for the lighting scatter (the lighting stream consumes it).

---

**Done when:** `bin/art maps` writes a `meta.json` per leaf carrying `channel_tints` + `outline`, folded
into the manifest, ready for the tiered-lighting port to consume.
