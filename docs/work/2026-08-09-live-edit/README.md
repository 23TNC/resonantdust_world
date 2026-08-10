# live-edit — `/edit` opens a live inspector on the selected object

_User (2026-08-09): "We will implement a new chat command `/edit`. This will open a new panel our
live_edit panel. We will open the live_edit panel against the selected object. There will be a
live preview in the top center of our edit panel that will function as a viewport with zoom and
pan. In the bottom half of our edit panel we will have a sub panel with tabs. The first tab will
be traits, which will contain the active traits on the selected object… displayed as squares in a
grid. Each traits color will be defined in the toml, and we will see the name of the trait when we
hover over it. The next tab will have needs… a text label, and to the right of it a bar… filled
based on the current maximum/minimum and value of the need. To the right of the bar we will
display a number with a sign so +/- that represents the current rate of gain or decay. - will be
red, + will be green. The color of the bar will be defined in the need toml. The third tab will be
the conditions on our active object… colored squares in a grid… drawn as they are in the
conditions bar with pie chart coloring. The fourth tab will be emotions, labels with current
values. I believe this is a decent start."_

## What already exists, and what does not

This matters more than usual here, because the four tabs are **not** equally expensive. Surveyed
against the delivered code:

| piece | state |
|---|---|
| `/edit` command | **free** — `ChatPanel.registerCommand(name, handler)` already exists; `/edit` is one registration |
| panel with tabs | **free** — `DomPanel.addTab(id, icon, content)` is built and in use |
| **conditions** tab | **mostly reuse** — `pawn_conditions` + `condition_emotions` feed the existing `ConditionCards`, whose whole job is already "coloured square, pie slices, name on hover" |
| **emotions** tab | **nearly free** — `pawn_emotion` returns `[index, sum₀..sum₁₅]`, and `emotion_label` / `emotion_color` exist |
| **traits** tab | **new engine surface** — there is no accessor listing a pawn's active traits, and **no `color` on a trait def** |
| **needs** tab | **the heaviest** — no `need_value`, no `need_min`, **no current-rate accessor at all**, and **no `color` on a need def** |
| live **preview** | **the unknown** — see below |

So two tabs are an afternoon and two are real work, and the preview is a question rather than a
task. The plan is ordered accordingly: cheap-and-certain first, so the panel exists and is useful
before the expensive parts land.

## The stance

- **The panel is an INSPECTOR first.** Everything the user described is read-only — squares,
  bars, labels, numbers. `/edit` and "live_edit" imply mutation later, and the design leaves room
  for it (F6), but nothing in this stream writes. Shipping a correct read surface is the
  precondition for ever trusting a write one.
- **The preview gets a SPIKE, not a guess** (F1). `Viewport` owns a `Renderer`, its own GL
  context, a `TextureResolver`, a `MaterialRegistry` and texture caches — a second instance is a
  second WebGL2 context with its own copy of every atlas page. That may be perfectly fine or may
  be unacceptable; the honest answer is a measurement, and the fallback (a purpose-built
  object-preview surface) is a different amount of work. Deciding before measuring would be
  guessing with a week of consequences.
- **Colour is authored, and that means schema work** (F2). Traits and needs carry no colour
  today. Emotions do (`color`, required) and conditions borrow theirs through the emotion pie —
  which is exactly why tabs 3 and 4 are cheap and 1 and 2 are not. Each needs a loader field, a
  wasm accessor, and a corpus pass.
- **The need RATE is a derived quantity, not a stored one** (F3). `deplete` in the TOML is the
  BASE (tics max→min); the live rate is that base scaled by the product of `rate` multipliers
  from active conditions and traits. Nothing exposes it. It is the single most valuable number on
  the panel — it is what tells you a pawn is starving *faster* — and it is the one that cannot be
  faked from data the client already has.
- **The tabs render from ONE snapshot per refresh** (F5). Four tabs each independently asking the
  wasm eval for the same pawn at the same tic would run the eval four times a poll; one snapshot
  per refresh, sliced four ways, keeps the panel honest about being a view.

## Watch

The needs tab is where this stream will actually be spent. `min`/`max` are authored per need and
`deplete` is a base rate, but the *effective* clamp narrows under conditions and the *effective*
rate is a product — so a bar drawn from authored bounds while the rate comes from live modifiers
would be quietly inconsistent, showing a full bar that is secretly capped
([I3](issues.md#i3)). And the preview spike may come back saying "second GL context is fine" or
"build a sprite preview" — those are different weeks, which is why nothing downstream of it is
scheduled until it reports ([F1](forks.md#f1)).

## Not in this stream (named, not built)

Editing anything (F6); a preview for non-pawn selections beyond whatever falls out of the spike;
re-authoring the conditions bar to share code with tab 3 beyond what reuse gives for free; any
change to what a trait, need, condition or emotion *is*.

## Exit

`/edit` with a pawn selected opens the live_edit panel bound to it. The preview shows that pawn
and responds to zoom and pan. Traits render as colour-authored squares naming themselves on
hover; needs render label + bar + a signed, coloured rate that visibly changes when the pawn
drinks or a condition lands; conditions render as pie squares matching the conditions bar;
emotions list with live values. Selecting a different object re-binds the whole panel. The user's
eyes close the stream.
