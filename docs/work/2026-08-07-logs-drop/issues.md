# Issues — logs-drop (anticipated inventory)

## I1 — the binding accessor's shape changes under three consumers {#i1}

`thing_interactions` (and its tile sibling) return `(name, magnitude)` tuples consumed
by the worker's offer probe, the wasm menu filter, and golden. Adding `yields` means
either a third tuple field everywhere or a small binding-params struct. Pick ONE shape
and sweep all consumers in the same change — a half-converted accessor is the drift
class the shared crate exists to prevent.

## I2 — the registry appends a kind; every sim binary must relearn it {#i2}

A new `[[thing]]` shifts nothing (append-only) but the LOGS def only resolves after the
master re-seeds the registry AND worker/npc reload their bundles — lumberjack I11:
rebuild + restart master, orchestrator, worker, npc after the corpus change, or the
yield SET composes a kind the world can't name.

## I3 — the yielded override must not read as a tombstone anywhere {#i3}

The client paths branch on `kind == 0` in several places (suppression in the offer
probe, `recordThingKind` deletion, the menu map). A logs override is a NORMAL
kind-carrying override — verify the felled-then-yielded cell reports the LOGS kind in
`thingDefAt` (not 0, not the tree) and that a reload replays it (the lumberjack edge
fixes should carry this for free; verify, don't assume).

## I4 — golden + placeholder-visual churn {#i4}

The logs def, its binding row on the tree, and the `yields` field all move golden;
re-bless with the diff audited. The placeholder tint renders as a flat box — that is
DELIBERATE (F3), not a texture-race bug; don't chase it in drills.

## I5 — carried successor: hauling / picking logs up {#i5}

The user's arc points at logs being GATHERED eventually. Nothing here builds inventory;
this stream leaves logs as inert scatter and records the successor by name.
