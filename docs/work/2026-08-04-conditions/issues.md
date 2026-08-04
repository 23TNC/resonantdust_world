# Issues — conditions

_Problems hit, candidate solutions, which we chose and why. Chronological._

## I1 — the rename crosses a deployed schema {#i1}

_2026-08-04. Open (scheduled — P1)._ `grant_moodlet` is a **SpacetimeDB reducer**, not an internal
function: renaming it changes the pawn module's published schema, invalidating
`server/st-bindings/src/pawn/grant_moodlet_reducer.rs` and the edge's mirrored copy under
`server/edge/src/bindings/pawn/`. A stale deployed module against fresh bindings is the SDK parse
panic already recorded in memory (`docker-cargo-mtime-miss`).

Order that avoids it: publish the module first (`bin/rd deploy module pawn`), regenerate both
binding trees, then `cargo check -p resonantdust-edge`, then `bin/rd redeploy --run`. Subscription
SQL is a string, so the live check is not optional — a green build proves nothing about it
(`build-gates-dont-gate`).

## I2 — the golden fixture could no longer be re-blessed {#i2}

_2026-08-04. **Fixed** in P0._ `BLESS_GOLDEN=1` did nothing. The bless branch lived inside
`the_golden_fixture_matches_the_corpus`, which loaded the **`.rd`** corpus — and `toml-content` P6
deleted the DSL, so `corpus()` found no `.rd` sources, returned `None`, and the test early-returned
before ever reaching the bless. The live guard was the *other* test
(`the_toml_corpus_matches_the_same_fixture`), which had no bless path of its own. Net effect: the
oracle still failed correctly on a corpus change, but the documented way to accept an intended
change was dead, and the failure message still blamed "the .rd fixture".

Not caught earlier because both halves read green: the dead test *passes* by returning early, so
`cargo test` showed 3 passing golden tests, one of which did nothing.

**Fix**: `corpus()` now loads the TOML corpus (the only corpus there is), the two `.rd`-only tests
are deleted, and the bless branch lives in the surviving comparison test. Verified by blessing (the
diff was the 8 expected rename lines) and re-running un-blessed to green. This is `toml-content`'s
leftover, found here — recorded in this stream because this is where it was fixed.

## I3 — `rd build core --check` had been red since 2026-08-03 {#i3}

_2026-08-04. **Fixed** in P1._ `client/core/src/bin/headless.rs:136` destructures
`Event::PawnParts { macro_position, entity_reference, tic, parts }` — but `74bdbe58`
(needs-moodlets P4) added a fifth field, `payload: Vec<u32>`, and never updated the binary.
`rustc` E0027, "pattern does not mention field `payload`".

It hid because the two gates disagree about scope: `rd build core` (release, default) builds the
**lib** and passed, while `rd build core --check` runs `--all-targets` and is the only one that
compiles `src/bin/`. The stream that added the field ran the former.

**Fix**: destructure `payload` and log it — headless is the wire-observation harness, so the raw
opcode stream is exactly what it exists to print. `rd build core --check` green. Found while
verifying this stream's npc/core rename; unrelated to the rename itself.
