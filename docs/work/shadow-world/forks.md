# Forks — shadow-world

_Decision points + options + which we chose + why. All 2026-07-20; the two headline changes are the
user's spec._

---

## F1 · `shadow-a`/`shadow-b` in the world-space toroidal buffer — 2026-07-20

The bitfield RTs move from screen-space (shadow-cast) to the **same world-space toroidal layout as the
other composites** (`albedo-cold`, `normal-cold`, …) — same fixed buffer, window/slot geometry, pan, and
nearest reproject. **Why:** shadows are on the ground (world-anchored); storing them world-space means they
stick to the world under pan/zoom, which is exactly what the real `shadow-cold` requires. This makes them
directly `/overlayRT`-able (world-aligned) alongside every other channel.

## F2 · Colour display moves to `/overlayRT`; remove the built-in mesh — 2026-07-20

**Remove** shadow-cast's bespoke full-viewport `ShadowDisplayShader`. Instead the bit→colour decode becomes
an **overlay mode** (float-mod, 5 colours, additive) that `/overlayRT shadow-a` / `-b` uses — so the
coloured-shadow inspector lives in the same debug path as `albedo`/`normal`/`zdepth`, world-aligned. (This
re-adds the `OVERLAY_BITS` mode removed after bitfield-rt, now routed for `shadow-*`.) **Why:** it's a great
debug tool and belongs with the rest of the G-buffer overlays, not as a special always-on display.

## F3 · Toroidal write — reuse the cache wrap; visible-window-first for bring-up — 2026-07-20

A world shadow quad can straddle the toroidal buffer seam → it must write up to 4 wrapped pieces (the same
4-way wrap `shadow-cold`'s copy needs — [shadows F10](../shadows/forks.md#f10)). **Chose** to reuse the
cache's existing window→buffer wrap math rather than reinvent it. **Bring-up latitude:** write only the
on-screen window first (accept seam artifacts at the buffer edge), then add the wrap once casting +
combine + overlay read correctly — so a wrap bug can't be confused with a cast/combine bug.

## F4 · Keep everything else from shadow-cast — 2026-07-20

Unchanged: 5 lights, 5 bits (RED byte, **A never data**), billboard-quad shadows from in-radius
`standingPrims`, one light moves/second, re-cast only the dirty light (cast → mask, combine → dest,
ping-pong), float-mod (ES 1.00), and the light dot + radius-ring **markers** (still a screen-space overlay,
projecting world→screen). This is an **edit** of the shadow-cast code, not a rewrite.
