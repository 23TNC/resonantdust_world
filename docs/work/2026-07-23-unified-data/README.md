# Unified data texture + command-buffer scatter — 2026-07-23

_Component: [`client/webgl`](../../components/client/) · `game/viewport/coldShadowData.ts`,
`shadowGather.ts`, `gl/`. Graduates [def-frame-anchors F5](../2026-07-23-def-frame-anchors/forks.md#f5)
into a stream. Phases in [`todo.md`](todo.md)._

ONE 1024×1024 RGBA32UI **data texture** holds every index table in 64-row bands, plus a constants
row; ONE small **command buffer** (64×64) carries all updates as absolute whole-texel writes,
applied by ONE **point-scatter draw** per frame. CPU never uploads the data texture directly.

## Layout (authoritative in VARIABLES.md once P0 lands)

```
data texture — 1024×1024 RGBA32UI (16 MB, provisioned once; linear index i → (i & 1023, i >> 10))
  rows   0–63    billboard_definition_data   65,536 defs, 1 px each (band base 0)
  rows  64–127   billboard_data              65,536 prims, 1 px each (band base 65,536) — F1: retire 2/px
  rows 128–191   light_data             65,536 light slots (band base 131,072; N_LIGHTS bitfield
                                        stays the shadow-cold cap — the band is address headroom)
  rows 192–1022  reserved               future tables (materials era)
  row  1023      constants              window mapping (cols/rows/winCol/winRow), counts, flags —
                                        updatable through the SAME command path

command buffer — 64×64 RGBA32UI (64 KB; 1 KB/row), 2 px per command:
  px0  header    u20 target linear index | u12 reserved
  px1  payload   the target texel's full RGBA32UI value
  → 2,048 commands per flush; count via uniform/draw-count (NO header px, no clearing)
```

## Why each property holds

- **Sequential in, random out.** The frame's commands upload as ONE contiguous row-span
  `texSubImage2D` (`uploadRows`); the scatter draw (`gl_VertexID` → command `texelFetch` → point at
  the target texel → integer fragment write) lands them anywhere in the data texture.
- **Replay-idempotent, no clearing.** Commands are ABSOLUTE whole-texel writes: re-executing a
  stale entry rewrites identical data — wasteful, never corrupt. Only the draw count matters.
- **No ping-pong.** The scatter pass reads ONLY the command buffer and writes whole texels; every
  later pass reads the updated data. Feedback-loop rule: the data texture is read-XOR-write per
  draw — a standing constraint on future passes.
- **No ghost coupling.** The data texture takes zero CPU uploads, so `texSubImage2D` ghosting
  vanishes for it; the only CPU upload is the tiny command span.
- **Bindings.** Gather binds ONE data texture instead of three (7 → 5 samplers).
- **Out of scope, on purpose:** the CPU-rebuilt tile-grid maps (presence/buckets/dirty — wholesale
  clustered rebuilds, `texSubImage` territory, R8UI among them) and the shadow-cold RT. Never mix
  writers on one resource.

## Cost accepted

16 MB VRAM provisioned up front (vs 1.5 MB of live tables) — the user's explicit capacity choice:
no realloc/copy/def-invalidation churn ever, and 830 reserved rows for the materials era.
