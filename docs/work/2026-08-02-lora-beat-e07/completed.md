# Completed — beat e07

_The verification log: dated entries saying what landed and **how it was checked**. Append-only;
authoritative for what's done and why we believe it._

_Nothing delivered yet. Items land here with their measured result when ticked in
[`todo.md`](todo.md)._

The `e07` / run-4 / run-5 baseline this stream measures against is carried in from the predecessor
streams and lives in [`README.md`](README.md) — deliberately NOT here. `completed.md` is what THIS
stream verified, and `rd work brief` reads its entries back to a resuming session as work that
landed; baseline facts sitting here get reported as deliveries. [P0](todo.md) re-runs the `e07` A/B
to confirm the baseline still reproduces, and that result is the first real entry.

## P0 — Rig, and the bar to clear

- **2026-08-02 · P0.1 · ComfyUI is up on the 3090 and needed no fiddling.** The user started the
  container; it took ~10 min to serve, all of it a recursive `chown` of the mounted `ComfyUI`/`HF`/
  `venv` dirs plus custom-node loading — not a GPU problem. `GET /system_stats` on `:8188` returns
  `cuda:0 NVIDIA GeForce RTX 3090 : cudaMallocAsync`, `vram_total` 25,294,995,456 B = **23.56 GiB**,
  ComfyUI 0.18.1 on **torch 2.11.0+cu128**. The existing venv carried over from the 2080 Ti
  untouched — the prebuilt wheel covers both sm_75 and sm_86, and driver 575.51.02 (CUDA 12.9) runs
  a cu128 build forward-compatibly.
- **2026-08-02 · bf16 verified on the real device, ahead of [P3](todo.md).** In the live venv:
  `torch.cuda.is_bf16_supported()` → **True**; device reports `sm_86`, **82 SMs**, 23.56 GiB. A
  4096² bf16 matmul against an fp32 reference gives **rel err 1.66e-3**, the expected bf16
  mantissa error — so the path is real, not a silent fp32 fallback. **P3's bf16 item is therefore
  a config change, not a risk.** Noted for that phase: `allow_tf32` is currently **False**, so
  fp32 matmuls are not using the tensor cores either — worth turning on for the non-bf16 parts of
  the graph.
- **2026-08-02 · P0.2 · ComfyUI stays OFF autostart** ([F5](forks.md#f5)). Autostart was insurance
  against [I2](issues.md#i2), but [P0.3](todo.md) removes that failure properly — a down box is now
  an immediate non-zero exit rather than a quietly-different corpus, which holds whether or not the
  service is running. Against it: the container takes **~10 min** to serve (measured today, nearly
  all recursive `chown`), the box's standing job is home automation, and its boot config is the
  user's existing choice. Revisit only if the pipeline becomes an unattended/cron job.
- **2026-08-02 · P0.3 · The silent LANCZOS fallback is gone — BOTH of them.** The known one was
  the pre-flight in `main()`. Reading the code found a **second, nastier** path: `normalise()`
  caught per-image ESRGAN failures and printed a line, so one interrupted run could produce a set
  that is **part ESRGAN and part LANCZOS**, with nothing on disk recording which images got which.
  Both now abort; `--allow-degraded` opts back in, and `--upscale lanczos` remains the deliberate
  whole-set choice. Verified all five paths:

  | case | expected | result |
  |---|---|---|
  | box unreachable, default | exit ≠ 0 | **exit 1**, message names both opt-outs |
  | box unreachable, `--allow-degraded` | proceeds | exit 0, `upscale=lanczos`, 2/2 images |
  | box unreachable, `--upscale lanczos` | proceeds | exit 0, `upscale=lanczos`, 2/2 images |
  | box UP, default | ESRGAN happy path intact | exit 0, `upscale=esrgan`, 2/2 in 33 s |
  | per-image ESRGAN miss (injected) | abort by default, tolerate with flag | **aborted**; `--allow-degraded` proceeded |

  The per-image case was checked by monkeypatching `esrgan()` to raise, since a real miss cannot be
  provoked on demand.
- **2026-08-02 · P0.4 · The evaluation set is pinned in `bin/lib/eval_set.json`, and three live
  defects fell out of pinning it.** The file holds subjects, directions, seeds, both prompts, the
  base checkpoint and the VAE; `lora_eval.py` now reads it instead of its own constants
  (`RD_EVAL_SET` overrides for smoke runs). It reproduces the historical matrix exactly — 6
  subjects (wolf/tiger/bear/cat/fox/pig) × e/s × seeds 7700-7702 = **36 cells** — with `n` carried
  as review-only so adding a direction cannot silently move the bar.

  What pinning exposed, none of it anticipated:

  1. **[I6](issues.md#i6) — the shipping A/B has no code.** `lora_eval.py` hard-coded a *different*
     matrix (4 subjects, 3 dirs, seeds 1001-1004); nothing in the repo mentions 7700. The verdict
     that ships `e07` is unreproducible from source. This is why [P0.5](todo.md) has to rebuild the
     matrix rather than "re-run" it.
  2. **The main `--loras` path was broken outright.** `measure()` returns `iou_control=""` when
     there is no control, and the row builder called `round()` over every value →
     `TypeError: type str doesn't define __round__`. Dead since `iou_control` landed, unnoticed
     because the real A/B never went through this path. Fixed to round only numerics.
  3. **A cold checkpoint load outran the 240 s generation timeout**, aborting the first cell of a
     matrix. Raised to 600 s and made tunable via `RD_COMFY_TIMEOUT`.

- **2026-08-02 · P0.4 acceptance · two invocations now produce byte-identical sheets — after a
  warm-up was added.** First attempt: `tiger_e` matched bit-for-bit but `wolf_e` did not — **max
  |Δ| 34/255 across 23% of pixels**, mean 0.138, while `results.csv` metrics were identical. The
  differing cell was the FIRST generation of each pass, so this is backend/allocator selection
  settling on the cold path, not a seed problem. A discarded warm-up generation now precedes the
  matrix; re-run twice, **both cells byte-identical**. Verified on a trimmed 2-cell set
  (`RD_EVAL_SET`) rather than the full 36 to keep it to four generations — the mechanism is what
  the criterion tests, and it is per-cell.
- **2026-08-02 · P1.1 · Base probe run; it overturned my pick.** 4 subjects × e/s × 1 seed, no
  LoRA, identical prompts and seeds on both candidates. cyberrealisticXL was NOT probed — it is
  being replaced, not evaluated ([D1](deviations.md)). Two code changes were needed first:
  `graph()` gained a **bare** path (`--loras none` wires the checkpoint straight through — an
  identity LoRA node is not the same thing), and the probe uses a **plain-language style prompt**
  instead of the pinned one, whose `rd_style`/`rd_quadruped`/`rd_east` triggers mean nothing
  without a LoRA.

  **Result: Animagine 4/4 east cells wider than tall (mean aspect 1.89); Illustrious 0/4 (1.02).**
  East aspect is the body-vs-bust discriminator, and the sheet agrees with the number — Animagine
  draws full-body side-profile quadrupeds, Illustrious draws emblems and faces. See
  [F4 measured](forks.md#f4).

  Incidental but important: **both bases fail on front views** ([I7](issues.md#i7)) — recorded
  because south has been blamed on the dataset for three runs and here it appears with no LoRA at
  all.

  Also learned operationally: the first cold read of a 6.6 GB checkpoint off the array exceeded even
  the 600 s timeout; once page-cached, generations run **~4.4 s**. `RD_COMFY_TIMEOUT` covers it.

## P2 — Rebuild the dataset from the lessons

- **2026-08-02 · P2.1 · v1-range scale variance restored via `--fill 0` (NATURAL).** Rather than
  widening the jitter — which cannot reach v1's spread without pushing subjects past the frame edge
  — `--fill 0` skips normalisation entirely and keeps the source framing verbatim, which *is* what
  v1 did. Measured over 120 corpus images, geometry only:

  | mode | mean | sd |
  |---|---|---|
  | v2 pinned (`--fill 0.85 --jitter 0`) | 0.850 | 0.0001 |
  | run-5 jitter (`--jitter 0.05`) | 0.852 | 0.0286 |
  | **NATURAL (`--fill 0`)** | **0.832** | **0.1405** |

  Against v1's measured **sd 0.1478**, that is a match on the property that matters (the spread);
  it also keeps ESRGAN, so this is the combination no run has had — v1's framing with v2's
  outlines. Acceptance asked for "near 0.148, not 0.027": **0.1405**.
- **2026-08-02 · P2.3 · Training resolution 1024²** ([F6](forks.md#f6)) — SDXL-native for both
  candidate bases. 768 was a VRAM accommodation from the 11 GB ceiling ([I4](issues.md#i4)), not a
  decision, and that ceiling is gone.
- **2026-08-02 · P2.4 IN FLIGHT · full rebuild running** — `prep_train.py --fill 0 --size 1024
  --dst .staging/animal-lora-train-v3`, ESRGAN enforced (no silent fallback possible now).
  Measured rate ~3.8 s/image → ~29 min for the 459-image corpus. Log: `.staging/prep_v3.log`.
  P2.2 (outline sharpness) and P2.5 (eyeball spot-check) are measured off the finished set.
