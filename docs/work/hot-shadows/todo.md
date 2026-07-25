# Todo — hot-shadows (execution order)

_Planned, not started. Items move to [`completed.md`](completed.md) when done + verified. Builds on the
delivered [`cold-data-textures`](../cold-data-textures/README.md) cold tier. Packed layouts authoritative in
[`docs/VARIABLES.md`](../../VARIABLES.md). Verify in-browser at `?focus=100,50`. Resolve
[`forks.md`](forks.md) F1–F4 (they set the layouts + mechanism) before/at P0._

---

## P0 · Layouts — work items, bit index, hot format, maps — 2026-07-21

- [ ] In `VARIABLES.md`: (a) **`hot-light-work`** uniform = 2 `u32` (`light_temp|prim_temp|reserved|index` +
      `lut_index|lut_count`); (b) **`cold_light_data`** — remove `lut_index/lut_count` (A channel), add
      `u8 shadow_bit_index` there; (c) **hot light / hot prim** uniform layouts = the cold packed layouts
      (`cold_light_data` / `cold_billboard_data` shapes); (d) the **hot LUT** uniform ([F1](forks.md#f1)); (e) the
      **`*-hot`/`*-cold` shadow map** format (bitfield, [F4](forks.md#f4)) + space ([F3](forks.md#f3)).

## P1 · Work-item-driven cast (refactor, cold×cold, behaviour-preserving) — 2026-07-21

- [ ] Replace the per-light draw (which reads `lut_index/lut_count` from `cold_light_data`) with iterating a
      **work-item list**: each item → resolve the light (temp+index: cold texture for now) + its prim run, run
      the SAME fan. Build the cold×cold work items on the CPU (from the existing LUT). Still draws coloured
      triangles to screen (output change is P2).
- [ ] **Verify:** shadows identical to the current cold-data cast (a pure input-plumbing refactor).

## P2 · Bitfield output — the `*-cold` map + `shadow_bit_index` — 2026-07-21

- [ ] Cast into an integer `RGBA32UI` **`shadow-cold`** map: each light sets its `shadow_bit_index` bit where
      shadowed (the OR-write mechanism, [F2](forks.md#f2)) instead of blending a colour to screen. A display
      pass decodes the bitfield → per-light colours (reuse `OVERLAY_BITS` / the `shadows` decode).
- [ ] **Verify:** `/overlayRT shadow-cold` (or the decode display) shows the per-light coloured shadows,
      overlaps combining — matching the current look, now from a bitfield.

## P3 · Hot lights + hot prims (uniforms) — 2026-07-21

- [ ] Upload hot lights + hot prims as **uniforms** in the packed cold format; the shader decodes uniform-or-
      texel uniformly (per `light_temp`/`prim_temp`). Add the hot work items (cold-light×hot-prim, hot×hot,
      hot×cold) writing the **`*-hot`** map ([F3](forks.md#f3) space). A test hot mover (a wolf, or a debug
      hot prim) exercises it.
- [ ] **Verify:** a moving hot prim casts a moving shadow (rebuilt each frame) into the hot map; a hot light
      casts over cold prims — all landing in `*-hot`, cold×cold untouched in `*-cold`.

## P4 · Budgeting — 2026-07-21

- [ ] Always submit the hot work items; **round-robin the cold×cold** work items under a per-frame budget
      (N items/frame), so the cold map refreshes in slices. Reset/clear semantics per [F4](forks.md#f4).
- [ ] **Verify:** cold shadows stay correct while only a slice rebuilds per frame; frame cost bounded.

## P5 · Combine + display / lighting feed — 2026-07-21

- [ ] Combine `*-hot | *-cold` (per [F4](forks.md#f4): shared map or OR-at-read) for the shadow display /
      the eventual lit consumer. Keep the default-on debug decode.
- [ ] **Verify:** the combined shadow reads correctly; a hot mover's shadow + the static cold shadows compose.

## P6 · Verify the tiered system — 2026-07-21

- [ ] In-browser: static (cold×cold) shadows from the budgeted cold map + a moving mover's shadow from the hot
      map compose correctly; pan/zoom stable; the work-item + budget bound per-frame cost; read back a map
      texel + a work item to confirm packing vs [`VARIABLES.md`](../../VARIABLES.md). Reconcile the
      [`shadows`](../shadows/README.md) stream (this absorbs its output half).
