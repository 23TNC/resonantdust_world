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
- [ ] Assert lighting at a NAMED cell rather than counting.
      Acceptance: a readback showing the accumulator non-zero at the torch's own texel and falling off.
- [x] Confirm B-4 is dissolved — the spawn area is lit.
      Verified: spawn is LIT — the zone that had 0 torches across 3,429 prims now has the 3 seeded ones.

## P4 — Retire the biome scatter
- [ ] Remove the `torch` draw from `forest` and `plains` in `content/biome/biomes.rd`.
      Acceptance: torch count is exactly the seeded count, with no probabilistic contribution.
- [ ] Keep the `torch` KIND in `content/{data,visual}/things.rd` — only the scatter goes.
      Acceptance: `thing_object_id("torch")` still resolves; the light attrs are untouched.

## P5 — Verify + close
- [ ] Re-run the identity + zoom checks with seeded torches as the light source.
      Acceptance: 0 mismatches with a non-zero population on both sides (the D-2 rule).
- [ ] Record the torch's kind id and cells in the stream so a future test can reuse them.
      Acceptance: a fixture another stream can cite.
