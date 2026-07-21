# Completed — cold-data-textures

_Done + verified. Items move here from [`todo.md`](todo.md) (append-only history; authoritative for what's
done). Nothing yet — the stream opened 2026-07-21._

## P0 · The four layouts, authored + byte-checked — 2026-07-21

The design pass with the user, formalized. All five forks resolved ([`forks.md`](forks.md)) and the four
`RGBA32UI` layouts + `position_anchor_reference` authored in
[`docs/VARIABLES.md` §Cold shadow data textures](../../VARIABLES.md), byte-checked to 128-bit pixels:
`cold_light_data` (1 px/light) · `cold_light_prim_data` (the LUT, 4 entries/px, indices only) ·
`prim_definition_data` (1 px/sprite variant, generic; `frame_page` + a spare `u32` for materials) ·
`cold_prim_data` (2 entries/px: `position_anchor_reference` + `u8 z | u2 rotation | u22 reserved`). The
normalization — a caster's position lives only in `cold_prim_data` — is the load-bearing choice: move a caster
= one texel, not every light's run.
