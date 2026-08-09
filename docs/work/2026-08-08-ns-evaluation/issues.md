# Issues — north/south evaluation

_A problem hit, with what it cost and how it was found. Findings live here; decisions live in
[`forks.md`](forks.md)._

## I1 — the metrics measure the one thing the pipeline already guarantees {#i1}
_2026-08-08 · carried from the predecessor, stated as the structural cause_

At `cn 0.5` ControlNet **hands** the sprite its silhouette. `iou_ref`, `aspect`, `fill`, `solidity`
and `hull_solidity` all describe that silhouette. So on north and south they measure the constant and
say nothing about the variable.

Recast as distance from the real corpus wolf, on **south**:

```
d_aspect    good 0.014   bad 0.010     the REJECTS are closer to the real wolf
iou_ref     good 0.087   bad 0.065     the REJECTS are closer
d_solidity  good 0.022   bad 0.022     identical
```

On **north**, `d_hull` is 0.022 in both buckets and `d_lum` is 32.1 vs 32.7. These are not weak
signals; they are *no* signal, because the property being judged is not in the outline.

**East escapes it** only because its failures are partly geometric — `d_aspect` separates good (0.021
from the corpus) from bad (0.289), a 14× gap — and because its head is a large side profile that
survives at 512 while a frontal face does not.

## I2 — the structural gate never fired on a single real reject {#i2}
_2026-08-08 · found while counting misclassifications, and it invalidated a result of mine first_

`blobs` — the connected-component count that quarantines fragmented sprites — is **1 on all 63
labelled images**, good and bad alike. It has never caught anything the user rejected.

**It also produced a false result in my own analysis.** My first misclassification count reported
`blobs` as getting **0 wrong in every direction**, which I nearly reported as a perfect classifier. A
constant metric admits no threshold, so my code returned "no prediction" and I was counting *no
predictions* as *no errors*. Fixed: a metric that cannot produce a prediction now counts as wrong.

The honest counts, once fixed:

```
63 images    always-say-"good"   15 wrong (24%)
             iou_ref             20 wrong (32%)     <- WORSE than not looking
             best metric per dir  7 wrong (11%)     <- three different metrics, one per direction
```

## I3 — the user's rejections are three different failures, not one {#i3}
_2026-08-08 · from the user's own account of three east rejects_

- *"I failed the top right for having a **white background**."*
- *"I failed the one to the left of it for having **improper line art**, however if pressed I'd take
  it as acceptable."*
- *"I failed the one to the left of that, because it drew **an extra line in the mane**, very minor
  detail — if pressed I'd take it as accepted."*

**The background case is now solved and was invisible to us.** A corner-alpha check finds exactly one
image in 63 with an unkeyed plate — corner alpha 0.307, **95.7% of the sprite opaque** against the
~25% a cut sprite should be — and it is precisely the flagged one. `bg_uni` missed it because it asks
*"is the plate uniform"*, which a fully-attached white plate passes perfectly.

**Two of the three are borderline by the user's own account**, which means the east metric's "3
wrong" is really **1 clear miss plus 2 boundary calls**. A binary label cannot express that, and
scoring a judge as wrong for agreeing with a "would accept if pressed" is a measurement error.
Hence [P0](todo.md)'s reason codes and `borderline` label.

## I4 — asymmetry is the best north/south signal found, and my explanation of it is wrong {#i4}
_2026-08-08 · measured against the labels_

Left-right asymmetry of the sprite interior gets **3 of 19 wrong on south**, against 7 for the best
geometric metric and 5 for always-guessing. It is the only thing that has separated good south
sprites from bad ones.

**The sign is the opposite of my hypothesis.** I predicted broken faces would be *lopsided*.
Measured: good sprites average **20.6** asymmetry, rejects **13.7** — the rejects are *more*
symmetric. The plausible story is that a splayed white mask is symmetric by default while a resolved
face carries markings and a slight turn, so detail creates asymmetry and mush does not. That is a
story told after the fact.

**On north it does nothing** — best there is `face_regions` at 4 wrong against a naive 5, which is one
image on n=13.

**Recorded as unexplained rather than adopted.** This is the second mechanism I proposed this stream
that the data falsified (the first: that south's control carried more interior edge for ControlNet to
trace — measured at 20%, identical to east's). Two wrong theories in a row is the argument for
[P1](todo.md)'s mechanism-free judge over a third.

## I5 — sample sizes cannot support any of these conclusions {#i5}
_2026-08-08_

```
east   31 images   26 good /  5 bad
south  19 images    5 good / 14 bad
north  13 images    8 good /  5 bad
```

At n=13, one image is 8 percentage points. `sat` scoring 95% on south is two images from being
ordinary, has no mechanism connecting saturation to whether a face resolved, and **inverts sign
against east** where good sprites are *more* saturated. Every "winner" in this stream so far is
within noise of being a coincidence.

**And south's 19 were all generated at 512**, where the frontal face lands in ~100px and reliably
breaks. They describe a resolution setting, not a direction, and are re-collected in [P0](todo.md)
before anything is fitted to them.

## I6 — a concurrent session stubbed this file mid-write, for the second time today {#i6}
_2026-08-08 · caught in the same turn, content rewritten_

While this stream was being authored, another session overwrote `issues.md` with a placeholder —
this time one that names the hazard itself: *"CAUTION: this file may have existed for a few seconds
before this write landed — if the owning session had just written content here, it was lost."*

That is exactly what happened, and it is the **second occurrence today**; the first is recorded as
[east-pipeline I4](../2026-08-08-east-pipeline/issues.md#i4). Both times the loss was visible in the
same turn and nothing had been committed, so the cost was one rewrite.

**Worth fixing rather than tolerating.** A docs-green helper that *writes content* into a folder
another session is authoring will silently destroy work, and it produces no conflict to notice. A
session that did not happen to re-read the file after writing it would commit the stub over its own
findings. The safe form is to create the file only if it does not exist, or to write nothing at all
and let `docs-check` report the gap.
