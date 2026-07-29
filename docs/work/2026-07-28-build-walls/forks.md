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
