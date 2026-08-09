# Issues — improvement audit (anticipated; logged before they bite)

## I1 — deliberate postures are NOT findings {#i1}

The project carries documented dev-postures that look like debt but are
decisions: no ownership model (the edge door is a verb allowlist by design),
placeholder art on purpose, refusals logged not surfaced to chat (spawn-
authority I3), the u8|u8 trait split withdrawn by the user. The sweep lists
a posture ONLY under a "deliberate, re-decide when…" section — never ranked
as if it were neglect. Re-litigating the user's calls is the failure mode.

## I2 — archived streams stay archived {#i2}

Delivered streams live in /home/wolf/archive/ and their closed items are
history. The sweep reads OPEN folders only; a candidate that traces to an
archived stream must show CURRENT evidence (lane 4/5), not a resurrected
ticket.

## I3 — identify, never fix {#i3}

Even a one-line fix found mid-sweep gets RECORDED, not committed — a fix
changes the thing being audited and invites scope creep. The one exception:
a docs-check breakage caused by the audit's own files. Trivial fixes become
chips/successors like everything else.

## I4 — the audit's own reading is bounded {#i4}

Lane 4's greps and lane 5's doc reads can expand forever. The bound: greps
run repo-wide but candidates are opened only where the hit is in a LIVE lane
(server/, client/webgl, client/npc, shared/) — pixijs and archived tooling
get one summary line each, not per-hit entries.

## I5 — statuses decay {#i5}

findings.md is a dated snapshot. Its header says so, and each successor
stream that picks up an entry should mark it there — but this stream does
not promise the file stays current beyond its date.
