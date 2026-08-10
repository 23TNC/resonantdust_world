# Live edit — the `/edit` inspector on the selected object

A panel opened by the `/edit` chat command that shows **everything the shared eval knows about the
selected object**, in one place, live. Authored 2026-08-09 (work
[`2026-08-09-live-edit`](../../../../work/2026-08-09-live-edit/README.md)); the decisions behind
each clause are that stream's `forks.md` F1–F6.

It is a fifth selection surface, and deliberately a different kind from the other four. Details /
intentions / conditions each answer one question at a glance while you play. This one answers
*all* of them at once, on demand, for someone tuning the corpus — which is why it is a command
rather than a panel that is simply open.

## Shape

```
┌─ live_edit ──────────────────────────────┐
│            ┌──────────────┐              │   preview: the selected object,
│            │   preview    │              │   zoom + pan (F1)
│            └──────────────┘              │
├──────────────────────────────────────────┤
│ [traits] [needs] [conditions] [emotions] │   the panel's OWN tab strip, not
│                                          │   `DomPanel.addTab` — see below
│   … active tab's content …               │
└──────────────────────────────────────────┘
```

The tab strip is **this panel's own widget**, styled to match the built-in one. `DomPanel.addTab`
is mutually exclusive with `setBody`, and the preview has to persist across tab switches rather
than being duplicated into every tab's content — so the panel takes `setBody` and rolls the strip
itself.

## The four tabs

| tab | shows | source |
|---|---|---|
| **traits** | active traits as colour squares in a grid, name on hover | a `pawn_traits` accessor + `color` on the trait def |
| **needs** | per need: label, a bar filled from value against the **effective** clamp, and a **signed rate** — red negative, green positive | one accessor returning `(value, min, max, rate)` together |
| **conditions** | active conditions as pie squares in a grid, name on hover | `pawn_conditions` + `condition_emotions`, rendered as the conditions panel already does |
| **emotions** | labels with current values | `pawn_emotion`, `emotion_label`, `emotion_color` |

Two of these were nearly free and two were new engine surface, and the reason is worth keeping:
**emotions author `color` and conditions borrow theirs through the emotion pie**, while traits and
needs authored no colour at all. Colour being authored per def is what lets the corpus decide how
its own concepts look; adding it to traits and needs closed the gap.

## The rules

**ONE eval snapshot per refresh, sliced four ways.** Every tab describes the same pawn at the same
tic through the same wasm eval. Four tabs fetching independently would run the eval four times per
poll and could show four different instants inside one panel. The panel takes one snapshot and
hands slices out; only the **active** tab renders, so the heaviest tab costs nothing while hidden.

**A need's bounds and its rate come from the SAME evaluation.** The authored `min`/`max` are the
need's domain, but conditions and traits **narrow the effective clamp**, and the live rate is a
product over those same modifiers. Drawing the bar from authored bounds while taking the rate from
live modifiers yields a panel that is individually plausible and jointly wrong — a bar reading
full while the effective max is 60. Hence one accessor returning all four numbers, not four
accessors.

**The rate's sign means the change in SATISFACTION.** Down is red, up is green, for every need —
including inverted domains (`inventory`'s satisfaction is FREE SLOTS, and `min` may be negative).
Fixing the convention in the accessor lets the panel colour it blindly, and stops each new need
from becoming a special case in the UI.

**The panel FOLLOWS the selection.** `/edit` binds it to whatever is selected now, and it re-binds
as the selection changes rather than freezing at open — a stale inspector silently describing
something else is a trap. Non-pawn selections open it with empty tabs rather than refusing; a
command that appears to do nothing reads as broken.

**Read-only.** Everything here is a display. The name anticipates mutation and the layout leaves
room for it — a preview that would show the consequence, tabs that already address a specific
trait/need/condition — but a write surface built over a read surface that is subtly wrong produces
edits nobody intended, and this read surface contains two derived quantities (the rate, the
effective clamp) that are easy to get quietly wrong.

## The preview

A live view of the selected object with zoom and pan. Its implementation was chosen by
measurement, not by design: `Viewport` owns a `Renderer` — and therefore its **own WebGL2
context** — plus a `TextureResolver` bound to that context, a `MaterialRegistry` and texture
caches. A second instance is a second context with every atlas page resident twice, and browsers
cap live contexts and **evict the oldest** rather than failing loudly, so the failure mode is the
world viewport going blank on the eighth `/edit`. The alternative — a purpose-built object preview
— is cheaper but re-implements a slice of the renderer and will drift from it.

See the work stream for which way the spike went and the numbers behind it.

## Invariants a change must not break

1. One eval snapshot per panel refresh, however many tabs exist.
2. A need's value, bounds and rate are read from one evaluation, never assembled from separate
   calls.
3. The rate's sign is the change in satisfaction — the panel colours, it does not decide.
4. The panel re-binds on selection change; it never keeps describing a stale object.
5. Nothing in this panel writes.
