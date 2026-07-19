#!/usr/bin/env python3
"""bin/art meta — the per-variant texture METADATA sidecar (`meta.json`).

A co-located `meta.json` in each variant leaf, beside the pixel maps (`albedo.png`,
`normal.png`, `surface.png`, …). It holds the art/texture metadata the runtime needs that
is **not** a pixel map:

  - `channel_tints` — the base colour of each packed material channel (`split_layers`), so the
    client can reconstruct `albedo + Σ layersᵢ·tintᵢ` and re-tint per DSL.
  - `outline` — the shadow-cast silhouette (boundary polygons + earcut triangulation), projected
    through a light in the tiered-lighting scatter.
  - … whatever later stages need — this is the extensible home for per-sprite art metadata.

**Read-merge-write:** each generator contributes its own keys via `update()` without clobbering
the others', so the map generators can run in any order / independently. The server folds these
into the content manifest the same way it folds `atlas.json`.
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import texpath


def path_for(a_map_path):
    """The per-variant `meta.json` sidecar in the same variant leaf (one per variant,
    no dir/part — outline/bbox/tints)."""
    return texpath.leaf_file(a_map_path, "meta", "json")


def load(a_map_path):
    """The leaf's current metadata dict (empty if none / unreadable)."""
    p = path_for(a_map_path)
    if os.path.exists(p):
        try:
            with open(p) as f:
                return json.load(f)
        except (json.JSONDecodeError, OSError):
            return {}
    return {}


def update(a_map_path, **keys):
    """Merge `keys` into the leaf's `meta.json` (read-merge-write); return the merged dict.
    Only the passed keys change — every other generator's contribution is preserved."""
    m = load(a_map_path)
    m.update(keys)
    with open(path_for(a_map_path), "w") as f:
        json.dump(m, f, separators=(",", ":"), sort_keys=True)
        f.write("\n")
    return m
