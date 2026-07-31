# Blockers — lighting + shader rework

## B1 — Three capabilities the rework does not carry forward (needs a yes/no, does not stop P0–P4)

**Filed open so the loss is a decision rather than a discovery.** Nothing before P5 depends on the
answer, so the stream can start today.

The strip removed emissive, ambient × AO, and the decay/flicker glow. The rework design does not
mention any of them, and the record layouts are **exactly 128 bits with no spare lanes**, so adding
one later is a layout change rather than a field.

| capability | what it did | cost to carry |
|---|---|---|
| **Emissive** | self-lit pixels — a wolf's eyes glow in pitch dark; mask rode the zdepth composite's R lane | needs a per-prim lane or a definition flag; the R lane still exists in the G-buffer |
| **Ambient × AO** | omnidirectional floor attenuated by baked occlusion, so crevices read as deep | needs the surface composite's G lane at read time; cheap, but only if the blit keeps sampling surface |
| **Decay / flicker** | ephemeral particle glow in its own coarse map, bilinear-sampled | a whole extra map and pass; the most expensive of the three |

**Recommendation: drop emissive and decay, keep ambient × AO.** AO is a single multiply against a lane
the blit already has and it is what stops flat-lit geometry looking like plastic. The other two are
independent features that a later stream can add deliberately, and neither is load-bearing for the
lighting model the rework is actually about.

**If you want any of them,** say so before P3 fixes the slot layout — after that the blit's inputs are
settled and it is a rework of the rework.

## B2 — Does per-light separation change the light COUNT you want? (not urgent)

Eight lights per tile came from `128 bits / u16`. In the rework the cap is set by the slot map instead
— 8 slots is 8 px per texel at 64 MiB, and 16 would be 128 MiB.

Worth a deliberate answer at P7 rather than now, with the measured cost in hand: **8 was an artefact of
the old bit budget, and it should not be inherited just because it is familiar.** The design says
"dense AUTHORED point lights, not a sun", which argues for more per tile if the frame can afford them.
