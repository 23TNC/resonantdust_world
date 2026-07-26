# Todo — torch as a real THING (execution order)

_Model in [`README.md`](README.md); decisions in [`forks.md`](forks.md); findings in
[`issues.md`](issues.md). Ordering principle: **the reducer before the caller, the caller before the retire**
— the biome scatter stays until seeded torches are proven, so the world is never dark mid-stream._

_Rust builds go through docker (`bin/rd build shared`, `rd-sim-builder`); `cargo` is not on the host PATH.
A schema change needs a LIVE check — subscription SQL is a string, so the build gates cannot catch it._

## P0 — Ratify placement + ownership (no code)
- [ ] Record the overlay-vs-baseline choice and why in [F1](forks.md#f1).
      Acceptance: one named primitive, with the clobber risk of the other written down.
- [ ] Record who resolves `kind_reference` and why the module cannot ([F2](forks.md#f2)).
      Acceptance: the DSL stays the single authority for name→id; no constant duplicated server-side.
- [ ] Choose the seed cells and zone; write them down as the test fixture.
      Acceptance: exact cells, so a later assertion can name them.

## P1 — `place_things` reducer on the `thing` module
- [ ] Add `place_things(macro_position, subtype_id, layer_id, cells, kind_reference, data)`.
      Acceptance: kind-agnostic and idempotent — calling twice leaves identical rows.
- [ ] Route it through the OVERLAY so it composites over worldgen instead of replacing a baseline row.
      Acceptance: seeding torches leaves the zone's trees/grass untouched.
- [ ] Live-check the schema against a running module (the build gates cannot see subscription SQL).
      Acceptance: `rd redeploy` clean, no SDK parse panic, edge connects.

## P2 — Edge resolves the kind and calls it
- [ ] Resolve `thing_object_id("torch")` from the edge's live DSL bundle, per zone seed.
      Acceptance: no hardcoded kind id anywhere server-side; a DSL reorder cannot desync it.
- [ ] Call `place_things` for the seed zone after worldgen seeds it.
      Acceptance: torches exist in zone 0 on a cold start, including zones generated earlier.
- [ ] Warn (not panic) if the corpus has no `torch` kind.
      Acceptance: a missing kind degrades to "no torches", never a failed zone seed.

## P3 — Verify the full stack end to end
- [ ] Confirm the client receives the torches as cold things with the right `kindId`.
      Acceptance: `primsWithLight` equals the seeded cell count exactly — not "approximately".
- [ ] Confirm each carries a light leaf and appears in `light_presence`.
      Acceptance: the seeded cells' tiles list a light id; `carriedLights` matches.
- [ ] Assert lighting at a NAMED cell rather than counting.
      Acceptance: a readback showing the accumulator non-zero at the torch's own texel and falling off.
- [ ] Confirm B-4 is dissolved — the spawn area is lit.
      Acceptance: `focus=100,50` shows torches without visiting virgin territory.

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
