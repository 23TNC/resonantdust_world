# Todo — torch as a real THING (execution order)

_Model in [`README.md`](README.md); decisions in [`forks.md`](forks.md); findings in
[`issues.md`](issues.md). Ordering principle: **the reducer before the caller, the caller before the retire**
— the biome scatter stays until seeded torches are proven, so the world is never dark mid-stream._

_Rust builds go through docker (`bin/rd build shared`, `rd-sim-builder`); `cargo` is not on the host PATH.
A schema change needs a LIVE check — subscription SQL is a string, so the build gates cannot catch it._

## P0 — Ratify placement + ownership (no code)
- [x] Record the overlay-vs-baseline choice and why in [F1](forks.md#f1).
      Acceptance: one named primitive, with the clobber risk of the other written down.
- [x] Record who resolves `kind_reference` and why the module cannot ([F2](forks.md#f2)).
      Acceptance: the DSL stays the single authority for name→id; no constant duplicated server-side.
- [x] Choose the seed cells and zone; write them down as the test fixture.
      Acceptance: exact cells, so a later assertion can name them.

## P1 — `place_things` reducer on the `thing` module
- [x] Add `place_things(macro_position, subtype_id, layer_id, cells, kind_reference, data)`.
      Acceptance: kind-agnostic and idempotent — calling twice leaves identical rows.
- [x] Route it through the OVERLAY so it composites over worldgen instead of replacing a baseline row.
      REVERSED by [I4](issues.md#i4) — the overlay relays `ColdState`, which builds no cold-thing prim.
      Init objects APPEND to worldgen's payload instead; `place_things` is kept for real per-cell overrides.
- [x] Live-check the schema against a running module (the build gates cannot see subscription SQL).
      Acceptance: `rd redeploy` clean, no SDK parse panic, edge connects.

## P2 — Edge resolves the kind and calls it
- [x] Resolve `thing_object_id("torch")` from the edge's live DSL bundle, per zone seed.
      Acceptance: no hardcoded kind id anywhere server-side; a DSL reorder cannot desync it.
- [x] Seed the init objects for the seed zone after worldgen seeds it.
      Verified: 3 torches at (100,51), (108,53), (104,59) — the cells offset by the sprite's bottom anchor.
- [x] Warn (not panic) if the corpus has no `torch` kind.
      Acceptance: a missing kind degrades to "no torches", never a failed zone seed.

## P3 — Verify the full stack end to end
- [x] Confirm the client receives the torches as cold things with the right `kindId`.
      Verified: 8 lights = 5 scatter + exactly the 3 seeded.
- [x] Confirm each carries a light leaf and appears in `light_presence`.
      Verified: `carriedLights` tracks the total; each seeded torch carries a light leaf.
- [ ] Assert lighting at a NAMED cell rather than counting (now trivial — the cells are deterministic).
      Acceptance: a readback showing the accumulator non-zero at the torch's own texel and falling off.
- [x] Confirm B-4 is dissolved — the spawn area is lit.
      Verified: spawn is LIT — the zone that had 0 torches across 3,429 prims now has the 3 seeded ones.

## P4 — Retire the biome scatter
- [x] Remove the `torch` draw from `forest` and `plains` in `content/biome/biomes.rd`.
      Verified after `redeploy --force`: **exactly 3** torches world-wide, at exactly the seeded cells.
- [x] Keep the `torch` KIND in `content/{data,visual}/things.rd` — only the scatter goes.
      Verified: the seeded torches still resolve their kind and light, so the contract is intact.
- [x] Already-seeded zones regenerated as a side effect of `redeploy --force` (module republish resets the
      cold shards), so no manual wipe was needed — [B-4](../2026-07-25-primitive-graph/blockers.md) is moot.

## P5 — Verify + close
- [x] Re-run the identity + zoom checks with seeded torches as the light source.
      Verified 2026-07-26 across zoom 0.25 / 0.5 / 1 / 2: **0 mismatches at every zoom**, on a genuinely
      non-zero population (coverage texels 723 / 2,736 / 10,946 / 10,946; max byte 255 = full occlusion).
      Coverage scales ~4× per lod step, and zoom 1 ≡ zoom 2 because both clamp to lod 0 — the expected shape.
      NOTE: every prior run of this check was **vacuous** — [I37](../2026-07-25-primitive-graph/issues.md#i37)
      had coverage pinned at zero, so corridor and brute agreed by both being empty. That is the D-2 trap
      exactly, and it hid a total shadow outage behind a passing test. A population assertion is not
      optional garnish on an identity check; it is the half that makes the other half mean anything.
- [x] Record the torch's kind id and cells in the stream so a future test can reuse them.
      Fixture: kind `torch`, `kind_reference` = `kind_id << 4 | variant`; 3 seeded at **(100,51), (108,53),
      (104,59)** in the seed zone (cells offset by the sprite's bottom anchor). Light leaf: reach 6 tiles
      (96 units), emitter radius 6, intensity 255, `cast` 1, class **0 (cold)** — read the cold RT with
      `__gather.debugReadShadow(0)`; the parameter defaults to **1 (hot)** and silently returns all zeros
      for these torches, which cost real time during I37.
      Height is **2.5 tiles = 40 units** and is shadow-critical — see [I38](../2026-07-25-primitive-graph/issues.md#i38).
