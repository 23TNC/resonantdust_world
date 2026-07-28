# material-system — todo

_Plan for the stream (see [README.md](README.md)). A/B = screenshot pair at area1
(`?user=Claude&focus=100,50&zoom=1&cb=area1`), before/after, recorded in completed.md. Identity =
no-material bake bit-identical (readback hash of the albedo + normal composites). Opened
2026-07-27._

## P0 — wake the machinery (the noise atlas stub)

- [ ] Port `noiseAtlas.ts` from pixijs (104 lines; fields `mottle`, `strand` from `bin/lib/noise_fields.py`) to an engine `Texture`, replacing the null stub. Acceptance: `makeNoiseAtlas()` returns a non-null tiling atlas; tsc green.
- [ ] Verify the EXISTING conifer strand jitter comes alive: A/B at area1 — foliage hue/chroma variation visible with the atlas in, absent with the stub. Acceptance: screenshot pair; no-material stems bit-identical (delta-form identity).

## P1 — registry v2 (normal detail params)

- [ ] Extend the `<material>` DSL def + wasm mirror + `material.ts` registry with normal-detail params: `detailField` (noise-field name), `detailAmp` (0 = off), `detailScale`. Acceptance: wasm rebuilt; a material without them parses unchanged.
- [ ] Thread the params to the bake as a third per-channel uniform vec4 (field row, amp, scale, placement mode — [F1](forks.md#f1)). Acceptance: tsc green; all-zero params ⇒ bake bit-identical (readback hash).

## P2 — the seed lane

- [ ] Ratify `u8 seed` in `billboard_data` B bits 0–7 (of the u14 reserved) in `docs/VARIABLES.md` FIRST ([F3](forks.md#f3)). Acceptance: VARIABLES row updated; `bin/rd docs-check` green.
- [ ] Stamp the lane in `coldShadowData`'s billboard record write from `cellSeed(tx, ty)` quantised to u8; the bake's `uSeed` reads the SAME value (one source). Acceptance: two adjacent same-kind trees show different variation; identical across reloads.

## P3 — normal detail in the bake

- [ ] Blend the per-channel detail normal in `MRT_FRAG` via RNM ([F2](forks.md#f2)): amplitude × layer weight, sample offset by seed, `oNormal.a` stays 1 (alpha is a BLEND FACTOR — lighting-feel F3). Acceptance: tsc green.
- [ ] Verify identity + shape: amp 0 ⇒ normal composite bit-identical; with amp up, `/overlayRT normal-cold` shows high-frequency structure ONLY where layer-0 weight is high (trunk untouched). Acceptance: overlay screenshots + hash.
- [ ] GPU-time the bake with detail on vs off (the standing-costs timer harness, bake draws). Acceptance: within the existing bake budget; numbers recorded.

## P4 — colour placement (the user's open question)

- [ ] Implement placement modes (a) UV / (b) world (exist) / (c) detail-field-keyed / (d) normal-keyed behind the per-channel mode uniform + a live `__material(mode)` override. Acceptance: switching modes re-bakes visibly differently at area1.
- [ ] Produce the A/B set: one screenshot per mode on the conifer, same seed, presented for the user's pick; ship default (c) meanwhile. Acceptance: 4 screenshots in completed.md; [F1](forks.md#f1) records the verdict when given.

## P5 — the conifer pine-needle material

- [ ] Append the `needle` field to `bin/lib/noise_fields.py` + regenerate the atlas ([F4](forks.md#f4)). Acceptance: atlas row 2 = needle; `mottle`/`strand` rows byte-unchanged (append-only).
- [ ] Author the conifer material in content: layer 0 (R, foliage) = needle detail + green hue variation; layer 1 (trunk) = mild bark mottle, no hue swing. Acceptance: content loads; trunk visually unchanged in the A/B.
- [ ] The stream-level A/B: conifers at area1 vs the pre-stream build — needles read, adjacent trees differ, silhouette unchanged, user verdict on "less plastic" solicited. Acceptance: screenshot pair + verdict recorded.

## P6 — wrap

- [ ] Record final evidence (identity hashes, bake timings, the F1 verdict or its pending state) in completed.md; update the work index row. Acceptance: `bin/rd docs-check` green.
