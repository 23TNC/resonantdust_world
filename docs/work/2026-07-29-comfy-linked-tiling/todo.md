# Todo — comfy-linked-tiling

_Numbers gate (P0 baseline → every later phase), the user's eyes ratify (P4).
Design + forks: [`README`](README.md). ComfyUI box required from P2 (`COMFYUI_URL`)._

---

## P0 — the albedo seam metric (extend the existing oracle)

- [ ] `atlas_check.py`: an ALBEDO section — the same D1 in-world adjacency strip pairs,
      measured on `diffuse.l.0.png` RGB (mean/max per-channel distance, E|W + S|N tables,
      worst pair named). Acceptance: runs on the smooth wall; numbers print.
- [ ] Record the smooth wall's albedo-seam baseline + captures of the two worst seam
      pairs (assembled as they'd abut) in `completed.md`. Acceptance: the baseline entry
      exists with numbers + the pair images in the stream folder.

## P1 — the layout compositor (deterministic, no ComfyUI yet)

- [ ] `bin/lib/retile_linked.py`: assemble in-world LAYOUTS from the atlas via the D1
      table (horizontal run, vertical run, 4 corners, 4 Ts, cross — each cell in real
      context), with a slice-back map (every layout px ↔ exactly one atlas px).
      Acceptance: assemble→slice round-trips the atlas bit-identically.
- [ ] Seam-band masks per layout (the abutting edge bands, ~2 units wide) + the FROZEN
      canonical-edge ledger (first finalisation wins; later layouts mask only unfrozen
      sides). Acceptance: mask PNGs emitted per layout; the ledger covers every D1-valid
      edge class exactly once.
- [ ] `bin/art retile-linked <kind> --dry` wiring: emits layouts + masks into the
      session scratchpad for eyeballing, touches nothing. Acceptance: the command runs
      on the smooth wall and lists what a live run WOULD inpaint.

## P2 — the ComfyUI seam-inpaint pass

- [ ] The inpaint graph in `retile_linked.py`: upload layout + mask, VAEEncode +
      SetLatentNoiseMask + KSampler at `--dn` (default 0.35), fetch, slice back with the
      frozen-edge discipline. Reuses `generate.py`'s client helpers. Acceptance: a live
      run completes on the smooth wall; pixels OUTSIDE the seam bands are bit-identical.
- [ ] Gate the pass with the P0 metric: albedo seams drop vs baseline; a second round
      runs only if round 1 didn't stabilise (ledger-frozen edges make it convergent).
      Acceptance: before/after numbers in `completed.md`; outside-mask identity re-checked.

## P3 — regenerate downstream + re-gate the normals

- [ ] Re-run the map chain on the new diffuse (`bin/art` delight → split_layers →
      `normal --marigold` → surface for the wall kind). Acceptance: all maps regenerate;
      `atlas_check` relief ≥ 20 % and normal seams ≤ the marigold stream's shipped
      numbers.
- [ ] Decide stamping strength on the NEW source (fork to record): if source consistency
      alone brings arm spread under ~2°, weaken/disable donor stamping to recover
      per-cell character. Acceptance: the chosen setting + its numbers in `forks.md`.

## P4 — the in-game drill

- [ ] The torch-side wall compound at zoom 1 + zoom 2: runs read as one continuous
      TEXTURED surface (albedo seams gone, relief intact, shading coherent with texture).
      Acceptance: captures in `completed.md`; the user's eyes are the final oracle.
