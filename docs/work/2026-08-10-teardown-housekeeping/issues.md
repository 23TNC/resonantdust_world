# issues — teardown-housekeeping

Problems hit, candidate solutions, which we chose and why. Chronological append. A problem that is
really a *decision* belongs in [`forks.md`](forks.md); one that genuinely needs the user belongs in
[`blockers.md`](blockers.md).

---

### H-I1 — "Verified as a complete duplicate" was the wrong check · 2026-08-10 · open

**What happened.** During the 0.3.0 teardown, `../resonantdust_world_old` was confirmed to be a
faithful copy before ~42G was deleted from the working tree: same HEAD (`3345f06d`), clean status,
and the ignored payloads present at matching sizes (142M textures, 7.3G venv, 8.7G DB data). That
check was sound for the deletion it authorised.

It does **not** support the next deletion. The moment the backup copy is itself the target, "it's
duplicated in the old copy" inverts — and the audit found `bin/keys/*.env`, deliberately never
committed, living nowhere else. Sizes matching says two copies exist; it says nothing about which
files exist in only one place *once one of them goes*.

**Candidate solutions.**
1. Prose warning in the README. — Insufficient; it relies on the reader re-deriving the asymmetry.
2. **Order the phases so salvage gates deletion, and state the gate in `todo.md`.** — chosen.
3. A pre-delete script that diffs ignored files across both copies. — Over-built for two known
   files; worth revisiting if a third copy ever appears.

**Chosen: 2** — see [`forks.md`](forks.md) H1. Left **open** rather than resolved, because the
generalised lesson (a duplicate-copy check is directional) should end up in `CONVENTIONS.md` or a
memory, and it has not been written down anywhere durable yet.
