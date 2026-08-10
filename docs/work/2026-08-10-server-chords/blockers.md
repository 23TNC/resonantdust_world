# Blockers — server-chords

_Needs human input: what blocks, why it needs them, options, recommendation. Terse — a
blocker is a line, not an essay; analysis belongs in `issues.md`._

## B1 — is this stream still the plan? (2026-08-10)

**Blocks:** everything past P2b. User, on seeing [I5](issues.md#i5): *"I don't think we can salvage
this."* Asked to scope what gets torn out; declined to answer yet.

**Why it needs them:** the next phases either patch the client position stack or delete it, and
those are opposite directions. P6 is irreversible.

**Options put to them:** (a) tear out the client position stack, bring P5+P6 forward — the gate and
the chord survey are done, which is what licensed deletions; (b) fix the clock only, keep the stack,
continue the plan in order; (c) rethink who owns position at all; (d) close the stream.

**Recommendation:** (a).

**State is safe:** nothing deleted, nothing started. Live world still running 24 debug movers; the
worker carries dev-only `chord arrival` gate logging that can be stripped on request.
