# Blockers — conditions

## B1 — a panel's content CANNOT exceed the panel's width {#b1}

_2026-08-04. Open — the user reserved this call ("If we cannot exceed a panel's width please let me
know and I'll determine another method to handle this")._

**What was asked.** Clicking a minimized condition card maximizes all of them, "which will cause
conditions to exceed the details width."

**What the code says.** It cannot. Two lines decide it:

- [`DomPanelStyles.ts:30`](../../../client/webgl/src/ui/dom/DomPanelStyles.ts) — `PANEL_CSS`
  sets `overflow: "hidden"` on the panel ROOT (the fixed-position container holding title bar,
  body, footer and resize handles). Every descendant is clipped at the panel's border.
- [`DomPanelStyles.ts:177`](../../../client/webgl/src/ui/dom/DomPanelStyles.ts) — `BODY_CSS`
  sets `overflow: "auto"`. Content wider than the body gets a horizontal scrollbar INSIDE the
  panel; it does not spill past the frame.

So a card strip parented anywhere inside `DetailsPanel` renders up to the panel edge and stops. The
clip is deliberate: it is what keeps overlapping panels reading as distinct rectangles, and
`PixiPanel` subclasses depend on the body region being the panel's own box.

**The four ways to get the behaviour anyway** (analysis, not a decision):

| # | Method | What the user sees | Cost |
|---|---|---|---|
| 1 | **Scroll the strip in place** — expanded cards live in an `overflow-x: auto` row inside the body | Panel geometry never changes; cards past the edge are reached by scrolling/dragging the strip | Smallest change. Doesn't match "exceed the details width" — the cards stay inside |
| 2 | **Widen the panel while expanded** — set `panel.style.width` to fit the strip, restore on collapse | The panel itself grows rightward; cards are all visible, always inside the frame | Fights the user's own stored geometry: width is user-owned, persisted per panel, and grid-snapped. Needs a rule for what happens if they resize while expanded, and a viewport-edge clamp |
| 3 | **Wrap to rows** — expanded strip becomes a wrapping flex, growing downward | Cards stack into 2–3 rows; the body scrolls vertically | Cheap and safe, but abandons the single horizontal row the user described |
| 4 | **Detached overlay strip** — expanded cards render into a `position: fixed` element that is a SIBLING of the panel, not a child, anchored to the panel's bottom-left | Cards genuinely paint past the panel's right edge, over the world | The only option that does literally what was asked. Needs anchoring on every drag/resize/anchor-flip (`rectChange` already fires for these), a z-index above the panel band, and a dismiss rule when the panel closes/minimizes/streams out |

**Suggested path: #4**, with #1 as the fallback inside it — the overlay is what the request
describes, and if the expanded strip is wider than the *viewport* it needs a scroll anyway, so the
overflow rule from #1 is the natural clamp. #2 is the cheapest thing that looks close, at the cost
of touching geometry the user owns.

**Why it blocks.** Only P6 — the overflow behaviour itself. P0–P5 (the rename, priority, the card
layout, the 4-maximized rule, the click toggle) are executable now and land the same either way.
