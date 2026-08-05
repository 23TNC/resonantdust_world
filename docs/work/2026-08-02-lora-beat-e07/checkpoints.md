# Checkpoints — what each saved LoRA actually is

_Provenance for every checkpoint kept out of a run. Filenames carry the distinguishing facts;
this file carries the rest. Live on the box at
`appdata/comfyui-nvidia/basedir/models/loras/`, so ComfyUI can load them by name._

## Naming

```
rd_<corpus>_<base>_r<run>_g<generation>.safetensors
    │        │      │       └─ generation (kohya's epoch number)
    │        │      └───────── run number, matching train_run<N>.sh
    │        └──────────────── base checkpoint
    └───────────────────────── training corpus
```

`g` for **generation**, the user's own term for these. Deliberately not `e`/`ep`: the old
`rd_quadruped_e07` / `rd_quad2_e15` / `rd_quad3_e15` names encode only an epoch, so nothing on the
filename says which corpus or base produced them — which is exactly how `e07`'s two-stage lineage
stayed invisible until 2026-08-03 ([I11](issues.md#i11)).

| segment | values so far |
|---|---|
| `<corpus>` | `full` = 701 images, 6 body plans · `quad` = 459 quadrupeds only |
| `<base>` | `anima` = animagine-xl-4.0 · `cyber` = cyberrealisticXL_v80 · `illus` = Illustrious-XL-v1.0 |

## Kept

### Run-10 — stage one, full corpus
Fresh (no warm start) · `animagine-xl-4.0` · 701 images / 6 body plans · 768 · `dim 64 / alpha 64`
(1.0× scale) · `LR 5e-5` unet, `2.5e-5` TE · batch 4 · bf16 · AdamW · 20 generations / 3520 steps ·
loss 0.025 → 0.0188. Script: `train_run10.sh`. **436 MB each.**

| file | why kept — the user's read of the sample grid, 2026-08-03 |
|---|---|
| `rd_full_anima_r10_g07.safetensors` | the **boundary**: duck and snake acceptable, but tiger's legs, wolf's chest and bear's hind legs all still wrong. First generation where the wolf's **tail splits in two** |
| `rd_full_anima_r10_g08.safetensors` | **best tiger**; also best of the five for duck and bear. Tiger legs and bear hindquarters correct here. Wolf tail already diverging; snake not yet coiled correctly |
| `rd_full_anima_r10_g10.safetensors` | **best wolf** (chest correct by here) and **best duck** |
| `rd_full_anima_r10_g11.safetensors` | **best bear** |
| `rd_full_anima_r10_g12.safetensors` | **best snake** |

**No single generation is best at everything** — the per-subject optima span g08–g12, and g07 is
consistently just-too-early on four of five subjects. Note `e07`'s own lineage warm-started from
**epoch 8** of its general stage, which is where the user independently placed three of five.

**What run-10 established** ([I12](issues.md#i12)): the body-plan contrast works — quadrupeds read
as low horizontal masses with unbroken bottom edges (the RimWorld convention, no articulated
limbs), while the biped stands upright and the legless coils. Runs 6–9 could not express that,
having never seen a non-quadruped.

**Open defect:** colour collapsed toward white/grey on a tinted plate — the tiger is white with
black stripes rather than orange-tan. New in run-10; not present in run-9. Untested whether a
quadruped specialist stage recovers it.

## Not kept

Runs 6–9 (`output_run6`…`output_run9`) remain on the box at ~8.6 GB per run but are **not**
promoted or renamed — all four are superseded, and their story is in
[`issues.md`](issues.md) I10/I11/I12. Delete them if the box needs space; nothing depends on them.
