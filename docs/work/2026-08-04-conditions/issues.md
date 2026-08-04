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
