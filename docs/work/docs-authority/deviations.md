# Deviations — docs-authority

_Where execution departed from the plan (`todo.md` / `forks.md`). Logged at the moment of
deviating. Each carries a **strong** reason ("less churn" is never one). Open→resolved._

## D1 — sync.md: moved + rescoped, not deleted (revises F4)

- **2026-07-19** · **Plan (F4):** delete `intent/sync.md`, fold the client half into
  `client/core/intent/`. **Did:** *moved* the whole doc to `client/core/intent/sync.md` and trimmed
  the dead server framing, rather than deleting and re-authoring a fragment.
  **Why:** the doc's client-projection model (facts-not-positions, speed-aware tween, determinism)
  is live intent captured **nowhere else** — deleting mid-sweep would lose it, violating
  [[preserve-future-intent]]. The server half is genuinely dead, but the doc is simply *mis-filed*
  now (a client concern), and the convention already says intent "distributes out to the owning
  component." Moving preserves the thinking and files it correctly. **Status: resolved.**

## D2 — decay-token invariant: markup-scoped + tree-tiered, not word-grep

- **2026-07-19** · **Plan (P2 invariant 1):** ban `deprecated|SUPERSEDED|stale|☠|~~` in
  authoritative files. **Discovered on real data:** a bare-word grep is a false-positive machine —
  `stale` is heavy *legitimate* prose ("reads a stale B", "prune stale versions", "scheduled stale
  server reaping"). **Refined the invariant to:**
  - **Strict tier** (`design/**` + `VARIABLES`/`TABLES`/`ACTIONS`): ban only **markup-level** decay
    markers — emphasis-wrapped decay words (`**deprecated**`, `**superseded**`, `⚠️ **stale**`), the
    phrase `superseded by`, `☠`, `~~…~~`, and (in the three shapes-only top files) the bare word
    `deprecated`. A decay word in plain prose is content, not a status label — allowed.
  - **Lenient tier** (`intent/**`): staging is *where* evolving, forward-looking status lives — a
    "re-fit to tics" note is legitimate; only `~~`/`☠`/emphasis-decay are flagged.
  - **Exemptions:** matches inside inline-code spans (`` `…` ``) are ignored (a doc *describing* the
    banned pattern — like this stream's own plan — must be able to name it); index/map READMEs may
    carry a bounded "Retired (why absent)" list per **F3**.
  **Why:** without this the check is unusable (noise buries signal) and would force deleting valid
  technical writing. **Status: resolved (spec for P2).**

## D4 — invariant precision, forced by the first real run (P2)

- **2026-07-19** · Building `docs_check.py` against the live tree surfaced three over-strict checks;
  tightened each so signal isn't buried (an audit that cries wolf gets ignored — the failure mode
  the whole stream exists to prevent):
  - **decay-markers:** emphasis-decay now matches only a **wholly-emphasized** word
    (`**stale**`, `**superseded.**`) — `**Stale prune**` (a `bin/art` step name) is prose, not a
    label. Was flagging the feature name.
  - **ref-integrity:** a `_reference` on a TABLES.md line that marks it `deferred`/`reserved` is
    intentionally undefined (e.g. `counterpart_reference`, cross-shard transfer, deferred) — exempt.
  - **map-bijection:** matches a component folder's path against the map's **full text** (a component
    is named in prose/code, not always linked) and skips `{design,intent,current,plan}` subfolders
    (they're *inside* a component, not components). Cleared 17 false warnings.
  Genuine finds the run caught + fixed: strikethrough in `cold-rework/blockers.md`; a dead
  code-pointer in `notes/tables.md` (edge has no `index.rs`); `route_reference` used in TABLES.md
  but absent from VARIABLES.md (see [`shard-tables/issues.md`](../shard-tables/issues.md)). **Resolved.**

## D5 — work-check signals must be structural markers, not prose inference (P5)

- **2026-07-19** · **Plan (P5):** detect open blocker / user-fork / stop-reason to decide if a pause
  is premature. **Building it, fuzzy prose-matching false-matched *three times*** — each time on
  docs-authority's own files, which *describe* the detection convention (so they contain every
  trigger phrase: "no user-facing fork", "session scope closed", "stop-reason"). The same
  "linter matches its own description" trap as the docs-check code-span case. **Refined the design to
  structural/explicit signals only:**
  - **Dropped the fork check** — per CONVENTIONS a *fork* is a decision the agent resolves itself;
    only a **blocker** (needs human) or a stop-reason justifies pausing. Simpler *and* removes the
    fuzziest signal. (F6's "awaiting user" is therefore correctly a **blocker**, now in
    [`blockers.md`](blockers.md), not a fork.)
  - **stop-reason = an unambiguous marker file** `work/<stream>/.stop-reason` (existence, not prose;
    gitignored, session-local — matching the session-scoped nature of "active stream").
  - **open-blocker = structural** (a bullet not under a "Resolved" heading, "None open" → false).
  All three detector paths now verified deterministic: premature→exit 2, blocker→OK, marker→OK.
  **Lesson (general):** a machine check over docs must key on **structure/markers it owns**, never on
  natural-language the docs also use to describe the check. **Resolved.**

## D3 — work todos keep compact "done" phase pointers

- **2026-07-19** · **Plan (P0):** purge DONE-annotations from `work/**/todo.md`. **Did:** removed
  `~~strikethrough~~` (the actual "partially-crossed-out mess" the row convention names), but **kept**
  compact `## Pn … ✅ DONE (see completed.md)` phase *pointers* in the large active streams
  (shard-tables, lighting, art-metadata, cold-rework). **Why:** a one-line pointer whose detail lives
  in `completed.md` is not the mess the convention forbids — it preserves the phase structure that
  makes a long multi-phase todo readable, at no drift cost. The P2 audit therefore bans **rendered**
  `~~` strikethrough (row-convention check, code-spans exempt), **not** `✅`. **Status: resolved.**
