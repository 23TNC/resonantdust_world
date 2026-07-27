# Forks — decisions taken, with what was rejected

## F1 — Phase order: fix the gate before touching the training data · RESOLVED 2026-07-25

**Gate first (P0), data second (P2).**

The tempting order is data-first: it is the biggest lever ([I1](issues.md#i1)), and the fixes are
already understood. Rejected because **the gate is the instrument every later comparison is scored
with**, and it currently both passes blobs and rejects antlers ([I3](issues.md#i3)). Retrain first
and the A/B result — "new LoRA scores better" — would be measured with a ruler we already know is
wrong in both directions. Fixing the ruler is one CPU-only phase and makes every subsequent number
trustworthy.

Also rejected: *interleaving* (fix the gate while the retrain runs). Tempting for wall-clock, but the
P3 A/B must be scored by a **frozen** gate — tuning the instrument while the experiment runs is how
you get an unreproducible result.

## F2 — Screening metric: silhouette IoU vs more bbox statistics · RESOLVED 2026-07-25

**Add `iou_control`** — IoU of the generated silhouette against the control silhouette that was fed
to ControlNet.

Rejected: **more bounding-box statistics** (perimeter ratio, moments, Hu invariants). They are all
the same *class* of measurement that already failed — global summaries that a blob can satisfy.

Rejected for now: **a learned/semantic scorer** (CLIP similarity to "a wolf", or a small classifier).
Strictly more powerful and worth revisiting, but it needs a labelled corpus we do not have, and it
would make the gate a black box at exactly the moment we need to trust it. IoU-against-intent is
cheap, explainable, and uses information the pipeline already holds.

**The gate never becomes the arbiter of taste** (README design stance): its job is to cut the pile
down to something worth a human glance. The anteater passing on proportions is the standing proof
that a number cannot replace the eye.

## F3 — Gate composition, calibrated · RESOLVED 2026-07-26

Measured on `.staging/gate-cal/labels.csv` (67 hand-labelled sprites). Errors = false positives
(bad passed) + false negatives (good rejected):

| gate | FP | FN | total |
|---|---|---|---|
| OLD — `blobs + bg_uni + d_aspect + solidity` | 2 | 2 | **4** |
| **CHOSEN — `blobs + bg_uni + d_aspect`** | 2 | 1 | **3** |
| + `d_fill <= 60` | 2 | 6 | 8 |
| + `d_fill <= 50` | 2 | 7 | 9 |
| + `d_aspect <= 40` + `d_fill <= 50` | 2 | 9 | 11 |

**`solidity` removed.** Marginal separation on the calibration set is **0.19 sd** — nearly no signal
— yet it cost a false negative (the horned oryx, rejected by 0.001). Verified fixed: that sprite now
passes at `d_aspect 9.1%`.

**`d_fill` rejected**, and this is the useful surprise: `fill` has the *second-best* marginal
separation (0.83 sd, better than `bg_uni`), so it looks like an obvious addition — but as a **hard
gate** it rejects good sprites faster than bad ones and doubles-to-triples total error. Marginal
separation does not imply gate value; only the FP/FN count decides.

**`blobs` retained despite 0.00 separation** on this set (every sprite scored 1). It is not measuring
nothing — it is measuring a failure mode (multi-subject sprite sheets) that the LoRA and negatives
have since eliminated. It costs nothing and is the regression alarm if that failure returns.

**Residual errors accepted, not tuned away:** 1 FN (a templated fox-south at `d_aspect` 61.4% that
is visually fine — the reference fox is unusually narrow) and 2 FP (the anteater south/north, which
are structurally unreachable — see [I7](issues.md#i7)). Tightening `d_aspect` to catch the anteater
would reject far more good art, as the table shows.

## F4 — Keep `FAMILY_REP`; `auto` is an addition, not a replacement · RESOLVED 2026-07-26

The plan's premise was that a shape-similarity search would beat the hand-written table and retire
it. **Measured, it does not.**

| | family table | `--control auto` |
|---|---|---|
| wolf | `canine -> Wolf_Timber` — correct | picked **`AEXP_Hedgehog`** ([I8](issues.md#i8)) |
| anteater | Elephant — bad, s/n collapse | Gorilla — **worse**, snout gone ([I9](issues.md#i9)) |

`auto` lost both cases it was built to win. The table encodes *semantic* knowledge (a wolf is a
canine) that a 32×32 occupancy grid cannot recover, and no threshold fixes that — the hedgehog was a
*confident* wrong answer at 0.79.

**What `auto` is genuinely worth keeping for is the part that was not planned: the DECLINE.**
Species with a real body-plan twin in the corpus score **0.705–0.967** against it; the anteater, whose
body plan exists nowhere in a quadruped corpus, tops out at **0.532**. So a confidence floor
(`AUTO_MIN_MATCH = 0.65`, below the 0.705 real-match floor with margin) detects "nothing here fits"
and falls back to **no control** — which for the anteater is the visually best of all three modes.

Shipping shape: **`template` stays the default**, `family:` remains the recommended explicit choice,
`auto` is available and is the right call when you do not know the body plan — chiefly because it
knows when to give up. Rejected: retiring the table (measurably worse), and raising the threshold to
force better picks (the hedgehog scored 0.79 — high confidence, wrong answer, so no threshold helps).

## F5 — Shipping checkpoint after run-4 · RESOLVED 2026-07-26

**`rd_quadruped_e07` stays.** A/B at each LoRA's own training resolution, 6 species × e/s × 3 seeds:

| config | gate | mean d_aspect | east | south |
|---|---|---|---|---|
| **e07 @768 (current)** | **26/36** | **32.5%** | 12/18 | **14/18** |
| run4 e10 @1024 | 24/36 | 45.6% | 14/18 | 10/18 |
| run4 e15 @1024 | 25/36 | 42.8% | **15/18** | 10/18 |

Run-4 loses overall, but the loss is **entirely** the south regression from
[I13](issues.md#i13) — it **beats e07 on east**, where the P2 data fixes show up as visibly richer
art. So this is not a refutation of sharper data / 1024 / dim 48; it is one prep bug that happens to
hurt one direction badly.

Rejected: shipping run-4 anyway for its better east (a broken direction is worse than a flatter one,
and the pipeline needs all three). Rejected: shipping a run-4/e07 blend per direction (real, but it
doubles inference complexity to work around a bug we can just fix).

**Next attempt** should re-prep with jittered fill and retrain; the east result says the payoff is
there once the frame artefact is gone.
