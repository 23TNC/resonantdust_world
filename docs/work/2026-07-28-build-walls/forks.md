# Forks — build-walls

_Decisions resolved during execution, with reasoning. Plan-stage decisions D1–D6 live in the
[README](README.md); expected forks: the DSL tag lane for build-wall kinds, the packed-tile
encoding for the event args, and how the preview overlay draws (textured quads vs a mini
cache)._

## F1 · Blueprint stem by CONVENTION, not authorship

`blueprintStemFor` replaces the stem's MATERIAL segment with `blueprint`
(`…/smooth/wall` → `…/blueprint/wall`): one blueprint atlas per KIND, material-agnostic —
blueprints are colourless outlines regardless of what they'll be built from. Authoring a
per-material blueprint would be 100% duplication today; revisit only if a kind ever wants a
distinct blueprint look.

## F2 · The blueprint preview draws as a dedicated overlay pass

Textured world-space quads (`BlueprintOverlay`), NOT cold-cache prims: the rect resizes
every pointer move, and cache prims would re-bake slots per move for content that is
client-only and ephemeral. Clearing = dropping the list. Unstreamed atlas cells draw a flat
translucent blue so the preview never blanks.

## F3 · BUILD_WALL queues SETs instead of writing rows

The verb writes NOTHING; the worker expands the perimeter and queues a `PROMOTE SET` per
tile (the verb-that-queues-events pattern MOVE_TO established). Why: a rect can span zones,
and per-tile SETs route each cell to its own zone through the normal queue — no cross-shard
write problem, and the FUTURE blueprint phase swaps what gets queued without touching the
verb.
