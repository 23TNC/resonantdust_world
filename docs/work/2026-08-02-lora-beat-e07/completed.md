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
