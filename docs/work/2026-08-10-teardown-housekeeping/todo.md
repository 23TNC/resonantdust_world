# todo — teardown-housekeeping

Items are checkboxes; `[x]` **is** the move — tick in place, never cut the line out. One action +
one acceptance criterion each. Phases read in execution order.

**P0 is a hard gate on P2 and P3.** Do not retire a copy or prune docker before the salvage items
are ticked — that ordering is the whole point of the stream.

## P0 — Salvage what exists in only one place

- [ ] Copy `../resonantdust_world_old/bin/keys/{r2.env,anthropic.env}` to `/home/wolf/keys/` at mode 600. Acceptance: both files readable there, `cut -d= -f1` shows the same 3 var names, and neither path is inside a git repo.
- [ ] Record in `/home/wolf/keys/README.md` what each credential is for and which tool read it. Acceptance: the file names R2 (`art upload`) and Anthropic (`art generate`) and links `0.2.3:bin/keys/README.md`.
- [ ] Confirm the credentials are not recoverable from git, so the copy above is load-bearing. Acceptance: `git log --all --diff-filter=A -- 'bin/keys/*.env'` returns nothing.
- [ ] Push `0.2`, `0.2.1`, `0.2.2`, and `sync-experiment` to `origin`. Acceptance: `git ls-remote --heads origin` lists all four.
- [ ] Push `claude/loving-feistel-0bda1d` to `origin`. Acceptance: `git ls-remote --heads origin claude/loving-feistel-0bda1d` prints a sha matching `122e1b8a`.
- [ ] Push `0.3.0` to `origin`. Acceptance: `git status -sb` shows no `ahead` count.
- [ ] Inspect the 8 dirty files in `/home/wolf/resonantdust` and report what they are. Acceptance: a `completed.md` entry lists each path and whether it holds unique work.
- [ ] Commit or bundle whatever those 8 files hold, per the finding above. Acceptance: `git -C /home/wolf/resonantdust status --short` is empty, or a `.bundle` exists in `/home/wolf/archive/`.
- [ ] Verify the 5 split repos are fully pushed before they are considered retirable. Acceptance: each of `resonantdust_{client,content,gateway,shared,wasm}` reports no unpushed commits on any branch.

## P1 — Repo-internal correctness

- [ ] Fix the `AGENTS.md` line claiming the docs-authority system is gone. Acceptance: it states that `CONVENTIONS.md` is restored and authoritative, and that `docs-check` is not yet ported.
- [ ] Point `AGENTS.md` at `docs/work/README.md` as the index of active streams. Acceptance: the link resolves and both open streams are reachable in two clicks from `AGENTS.md`.
- [ ] Delete `.claude/skills/rd-execute` and `.claude/skills/rd-plan`. Acceptance: `grep -rl 'bin/rd' .claude/` returns nothing.
- [ ] Re-add a planning skill that matches 0.3.0, or record the decision not to. Acceptance: either `.claude/skills/*/SKILL.md` exists with no `bin/rd` reference, or `forks.md` carries the row.
- [ ] Update `.vscode/launch.json` to the port 0.3.0 actually serves, or delete it. Acceptance: no `:5173` reference survives unless something listens there.
- [ ] Audit `.gitignore` for rules whose paths no longer exist. Acceptance: every remaining rule matches either a live path or a path 0.3.0 will create; `git check-ignore -v` confirms each.
- [ ] Confirm no tracked file references a deleted 0.2.3 path. Acceptance: `git grep -nE 'bin/(rd|art|sim|content)|shared/codec|client/(webgl|pixijs|npc)|docs/(components|VARIABLES|TABLES|ACTIONS)'` returns only intentional history references.

## P2 — Retire the redundant local copies

_Gated on P0._

- [ ] Remove the `loving-feistel-0bda1d` worktree with `git worktree remove`. Acceptance: `git worktree list` shows one entry and `.claude/worktrees/` is gone (90M freed).
- [ ] Run `git worktree prune` and confirm no stale registrations remain. Acceptance: `git worktree list --porcelain` lists only the main tree.
- [ ] Write the retention list for `../resonantdust_world_old` — what it uniquely holds. Acceptance: the list names the 142M texture corpus and nothing else that isn't in git or salvaged.
- [ ] Mark the old copy read-only so nothing edits it by accident. Acceptance: `chmod -R a-w` applied, and a `RETIRING.md` at its root names the stream that may delete it.
- [ ] Retire `pixijs_archived` (1.8G), `backup` (494M), and `master_backup` (35M) after confirming each is superseded. Acceptance: for each, a `completed.md` line saying what supersedes it; then removed.
- [ ] Retire the tiny stale dirs `config`, `data`, `goof`, `jobs`, `content-recovered` after inspection. Acceptance: each is either empty/superseded and removed, or kept with a one-line reason.
- [ ] Retire the 5 pre-monorepo split repos. Acceptance: gated on the P0 push check; then removed, with their origin URLs recorded in `completed.md`.
- [ ] Decide the fate of `/home/wolf/resonantdust` (14G). Acceptance: gated on P0; then removed or moved under `/home/wolf/archive/`, with the decision in `forks.md`.
- [ ] Leave `/home/wolf/archive/` alone. Acceptance: a `completed.md` line recording that it is the intentional work-stream archive, not debris.

## P3 — Docker reclamation

_Gated on P0. Prune by name — never `system prune -a`._

- [ ] Remove the 0.2.x build images `rd-sim-builder`, `gateway-build`, `server-build`, `edge-build`, `simulation-build`, `simulation-run`. Acceptance: `docker images` lists none of the six.
- [ ] Remove `clockworklabs/spacetime:latest` and `:v2.3.0`. Acceptance: `docker images | grep -c spacetime` prints `0`.
- [ ] Remove `resonantdust-laigter:1.13.1` and `dpokidov/imagemagick`. Acceptance: neither appears in `docker images`; `0.2.3:bin/laigter` is noted as the way back.
- [ ] Remove the 19 unused volumes, all at 0 containers. Acceptance: `docker volume ls -q | wc -l` prints `0` and 2.384G is reclaimed.
- [ ] Remove the `resonantdust` network. Acceptance: `docker network ls --filter name=resonantdust` lists nothing.
- [ ] Prune the build cache. Acceptance: `docker system df` shows build cache at 0B.
- [ ] Keep `busybox`, `alpine`, `rust:slim`, `debian:bookworm-slim`, `python:3-slim`. Acceptance: all five still listed — 0.3.0 re-pulls them otherwise.
- [ ] Record the before/after `docker system df`. Acceptance: a `completed.md` entry with both tables.

## P4 — Port the docs checker

_Owns F7 from the login stream._

- [ ] Port `0.2.3:bin/lib/docs_check.py` to `bin/docs-check`, dropping the `bin/rd` profile machinery. Acceptance: `bin/docs-check` runs with no `RD_*` env var set.
- [ ] Keep the checkbox-vs-bullet ERROR — a plan file with bullets and no checkbox must fail. Acceptance: a fixture with only bullets exits non-zero naming the file.
- [ ] Keep the oversized-item warning at ~250 chars. Acceptance: a fixture item of 300 chars warns; one of 200 does not.
- [ ] Keep the work-index link check — an unlinked `work/<w>/` folder fails. Acceptance: a fixture folder absent from `docs/work/README.md` exits non-zero.
- [ ] Drop the `current/` freshness-stamp check until `docs/components/` exists. Acceptance: `bin/docs-check` passes on today's tree, which has no `components/`.
- [ ] Run it green over both open streams. Acceptance: `bin/docs-check` exits 0 with `docs/work/2026-08-10-*` present.
- [ ] Update the login stream's F7 to point at this phase. Acceptance: that row reads resolved-here, not deferred.

## P5 — Memory hygiene

- [ ] Run a `/consolidate-memory` pass over the ~60 0.2.3 memories. Acceptance: every remaining entry is either true of 0.3.0 or explicitly marked historical.
- [ ] Keep the design-reasoning memories that outlived their code, marked as history. Acceptance: `client-sync`, `art-style`, and `object-model-redesign` survive with a historical marker rather than being deleted.
- [ ] Delete memories that only describe deleted mechanics. Acceptance: no memory names a `*_tables!` macro, a verb number, or a `bin/rd` subcommand as current.
- [ ] Prune `MEMORY.md` to match. Acceptance: every index line resolves to a file that exists.

## P6 — Cloud and remote audit

_Has the stream's one blocker — see [`blockers.md`](blockers.md) H-B1._

- [ ] Check whether the `lightsail` docker context still reaches a host. Acceptance: `docker --context lightsail ps` either errors as unreachable or lists containers; the result is recorded.
- [ ] Report what an R2 bucket would still be holding and costing. Acceptance: a `completed.md` entry naming the bucket/prefix from `0.2.3:bin/art` and the texture count.
- [ ] Report the DNS state of `gateway.resonantdust.com`. Acceptance: a `dig` result recorded, saying whether it resolves and to what.
- [ ] Ask the user to decide keep-or-tear-down for Lightsail, R2, and the domain. Acceptance: H-B1 carries their answer, dated.
- [ ] Execute the user's decision. Acceptance: for each of the three, an action taken or an explicit keep recorded.
- [ ] Remove the `lightsail` docker context if the host is gone. Acceptance: `docker context ls` no longer lists it.
