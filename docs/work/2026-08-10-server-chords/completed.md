# Completed — server-chords

_Opened 2026-08-10._

## 2026-08-10 — P0: the two verbs exist on the wire

`MOVE_CHORDS = 20` and `CANCEL = 21` in `shared/codec::action`, with palette rows in
`docs/ACTIONS.md` — the authoritative table, which outranks the code.

- **`CANCEL` is `&[ReadWrite]`.** The pawn must be a WRITE or the verb neither joins its write
  group nor serialises against the hops it is cancelling, and a Read so the worker sees the row it
  resolves from. This is why it cannot fold into `CANCEL_INTENT` ([F2](forks.md#f2)).
- **`MOVE_CHORDS` is the fifth variable-arity verb**, added to the framing list, the
  `collect_operands` skip list, and `target_routes`' write-less arm.

**Four tests, and one of them earns its keep:**
`move_chords_frames_and_a_verb_after_it_survives` puts a fixed-arity verb AFTER the variable one,
because a mis-framed variable verb has no re-sync point — everything downstream is garbage with no
error. Omitting the skip-list entry instead panics inside `collect_operands` at its
`signature().expect("parsed, so known")`, which the third test covers.

**Found while writing that test:** `SET_NEED`'s arity is **3 in code** while `ACTIONS.md` says
"Arity 3→2 with the stat-model reshape". My first draft used it as the canary and got
`Truncated { action: 10, want: 3, got: 2 }` — I read it as a framing bug in my own change for
several minutes. Docs outrank code, so one of them is wrong; logged as [I3](issues.md#i3) rather
than fixed in passing, since changing a verb's arity is a wire change.

**Also added the three palette rows the survey found missing** — `INV_ADD` 15, `INV_REMOVE` 16 and
`ACTIVATE_TRAIT` 19 were shipped verbs absent from the authoritative table, which is how 20 and 21
came to need checking by hand.

76 codec tests pass.
