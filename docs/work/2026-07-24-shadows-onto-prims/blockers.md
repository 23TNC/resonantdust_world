# Blockers — cast shadows onto prims

_Things that stop progress and need input/resolution. Chronological._

---

## B-1 · The receiver/caster vertical OFFSET model — needs the user {#b1}
**2026-07-24 — OPEN, awaiting user.** P0–P2 landed and the headline win holds: the on-prim shadow is
**zoom-stable** (verified in + out, corridor↔brute 0 mismatches at two zooms — the in-family receiver mask
fixed what reverted attempt #2). What is NOT settled is the **vertical alignment** between the on-prim
shadow and the drawn sprite: the shadow's base seats ~2–3 units off, landing in the GAP below a receiver
instead of on it ("shadows clip the tops" / "shadows shifted down 2–3, or sprites up 2–3", user image).

I added a live knob `uRecvOff` / `__recvoff` that shifts the receiver rect's `Ac.y`, and dialed it by eye —
but I have the OFFSET MODEL wrong (the user: "I don't think you're understanding offsets"). `+3` (= the
ground `SHADOW_LIFT`) made it worse: it pushes `baseRow` south, so the shadow's `z=0` seat lands ~3 units
below the sprite's visible base (in the gap). Default reset to **0** pending the answer.

**The question put to the user:** what is the offset BETWEEN, and in which direction/frame? Candidates:
(1) each prim's stored anchor (`prim.y+height`, full-box bottom) vs where its SPRITE is drawn — one
anchor→sprite constant applied to BOTH the caster shadow origin and the receiver base; or (2) a second
offset I'm missing (e.g. the 65° ground↔screen tilt relating ground-space shadow to screen-space sprite).
`SHADOW_LIFT = 3` already compensates the anchor→sprite gap for the GROUND shadow; the open question is how
the SAME (or a different) gap applies to the caster origin and the receiver base for on-prim casting.

**Do not dial `recvoff` further until the model is clarified** — resume P2/P3 (align, then the zoom-sweep
close) once the user answers. The rest of the pipeline is verified and unaffected.
</content>
