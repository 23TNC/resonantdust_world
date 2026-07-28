# Issues — material system

## I1 — `bin/lib/noise_fields.py` does not exist (plan correction) {#i1}

The plan (and a `material.ts` comment) referenced it as the atlas source; it was aspirational.
The RUNTIME generator (`noiseAtlas.ts`) is the atlas's only source today — the `needle` field was
appended there (lattice [9,48] + a sharpening shape transform), honouring the append-only row
rule. If richer hand-tuned fields are ever wanted, the python bake can be created then, emitting
the SAME layout.

## I2 — the F1 verdict is the user's, pending {#i2}

The four placement modes are live behind `__material(0..3)` and the A/B set was produced
(completed.md); DETAIL-FIELD-KEYED (2) ships as the default for detail-carrying materials per the
F1 lean. The user's by-eye pick — and their stream-level "less plastic" verdict — are solicited in
the session report; record both here when given.
