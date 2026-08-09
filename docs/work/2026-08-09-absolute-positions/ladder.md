# The ladder — the world's migration to absolute positions

_The rungs AFTER this stream's foundation + pilot. Each rung is a successor stream sized
like the ones this repo finishes; each retires a MEANING of "zone" (F4) and says so in its
docs. Order respects dependencies; re-order only with cause written here._

| # | rung | scope | depends on |
|---|---|---|---|
| 1 | **cold-tiles-absolute** | tile shard re-keyed to absolute ranges; ColdTiles rows → range-served bands; autotile math off nibbles | the pilot |
| 2 | **cold-things-absolute** | thing shard likewise; the composed THING view + tombstone fan on ranges | rung 1 |
| 3 | **worldgen-handoff** | generation keeps 16×16 batch cells but SEEDS/serves by absolute position; identical-world diff (I6) | rungs 1–2 |
| 4 | **anchors-become-viewports** | the client/edge anchor machinery → pure range viewports (nested radii = nested ranges); npc host module areas ride along | rungs 1–3 |
| 5 | **events-absolute** | `event_uid` re-shaped under the law (the F1 violator); the settle fan + orchestrator grouping by range overlap (I5) | rung 4 |
| 6 | **chords-native** | the chord eval + re-anchor + clamp law on unit-sixteenth math end to end; movement drills re-run (I4) | the pilot |
| 7 | **npc-world-model** | the Bot's zone maps → range maps; scans become pure coordinate math | rungs 1–2, 4 |
| 8 | **the un-zoning** | the legacy packers/lanes DELETE; "zone" leaves the vocabulary except as worldgen's batch cell; VARIABLES.md's hierarchy section rewritten | ALL rungs |
