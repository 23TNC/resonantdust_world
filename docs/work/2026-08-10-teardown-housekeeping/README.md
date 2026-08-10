# teardown-housekeeping

**Status:** open · **Opened:** 2026-08-10 · **Components:** repo-wide, `dev/`

Close out what the 0.2.3 deletion left dangling. The `0.3.0` teardown removed 4554 tracked files
and ~42G of ignored payload from the working tree, but a deletion that size leaves a tail: copies
that are now the *only* copy of something, tooling that points at paths which no longer exist,
docker state nothing will ever use again, and ~60 memories describing code that is gone.

## Framing: this is about ambiguity and risk, not disk

Measured 2026-08-10: the disk is **94G used of 1007G, 863G free (10%)**. Nothing here is urgent for
space, and the plan should not pretend otherwise. The two things that actually justify the stream:

1. **One genuine risk of irreversible loss.** `bin/keys/r2.env` and `bin/keys/anthropic.env` were
   gitignored — never committed, on purpose — so they now exist in exactly one place:
   `../resonantdust_world_old/bin/keys/`. That directory is the one everybody thinks of as "the
   throwaway backup copy". Deleting it destroys the only copy of the R2 and Anthropic credentials.
   Same shape for four unpushed local branches and 8 uncommitted files in `/home/wolf/resonantdust`.
2. **Nobody can currently answer "which copy is authoritative?"** There are two full checkouts of
   this game (45G + the live repo), a registered git worktree holding a third partial one, five
   pre-monorepo split repos, and a 14G pre-`_world` iteration with dirty files. Every one of them
   answers to "resonantdust". That ambiguity is what makes a wrong deletion likely later.

So: **salvage first, retire second, reclaim third.** P0 exists entirely so that a mistake in P2 or
P3 stops being expensive.

## What was found

| Thing | State | Where |
|---|---|---|
| `r2.env`, `anthropic.env` | **single copy**, gitignored, never committed | `../resonantdust_world_old/bin/keys/` |
| branches `0.2`, `0.2.1`, `0.2.2`, `sync-experiment` | **local-only**, never pushed | this repo |
| branch `claude/loving-feistel-0bda1d` | local-only, live worktree, 90M | `.claude/worktrees/` |
| `/home/wolf/resonantdust` | 14G git repo, **8 dirty files**, has origin | prior iteration |
| `resonantdust_{client,content,gateway,shared,wasm}` | 5 repos, clean, pushed | pre-monorepo split |
| `pixijs_archived`, `backup`, `master_backup` | 1.8G + 494M + 35M, not git | prior iterations |
| docker images / volumes / build cache | 16.9G / 2.4G (0 in use) / 5.1G | local daemon |
| `resonantdust` network, `spacetime_cargo-cache` volume | still present | local daemon |
| `.claude/skills/rd-{plan,execute}` | tracked, drive the deleted `bin/rd` | this repo |
| `AGENTS.md` | claims the docs-authority system is gone — now false | this repo |
| `.vscode/launch.json` | points at vite `:5173`; webgl ran on `:5174` | this repo |
| `bin/rd docs-check` | deleted, but `CONVENTIONS.md` cites it throughout | this repo |
| ~60 memories | describe deleted 0.2.3 code | agent memory |
| Lightsail context, R2 bucket, `gateway.resonantdust.com` | possibly live, possibly billing | **needs the user** |

## Design stance

- **Nothing is deleted before its unique content is somewhere else.** P0 is a hard gate on P2/P3.
  Every later phase names what it depends on having been salvaged.
- **`git show 0.2.3:<path>` is the retention story for tracked files.** They are on `origin` now, so
  no local copy needs to exist to preserve them. Only *gitignored* content justifies keeping a
  working copy — which narrows "what do we keep the 45G copy for" to a short, checkable list.
- **The old copy survives this stream.** It holds the only copy of the 142M texture corpus, and
  0.3.0 cannot yet render anything. It gets retired by whichever stream re-establishes texture
  serving, not by this one — see [`forks.md`](forks.md) H2.
- **Docker gets pruned by name, not by `system prune -a`.** A blanket prune also destroys the base
  images (`rust:slim`, `busybox`, `alpine`) that 0.3.0 will immediately re-pull, and on a 10%-full
  disk that trade is backwards.
- **Cloud resources are the user's call.** Money and DNS are not mine to decide; that phase reports
  and asks rather than acts. It is the one blocker in the stream.

## Relationship to the other open stream

[`2026-08-10-login-without-spacetime`](../2026-08-10-login-without-spacetime/README.md) owns
deleting `server/spacetime/` + `bin/st` and the `spacetime_cargo-cache` volume (its P0). This stream
does **not** duplicate that. It does take ownership of that stream's **F7** — porting
`docs_check.py` — because a checker for a convention is housekeeping, not login work.

## Done when

The credentials exist outside any deletable working copy; every local-only branch is on `origin` or
in a verified bundle; no tracked file or tooling reference points at a deleted path; docker holds
only what 0.3.0 uses; the memory index no longer presents 0.2.3 code as current; and the cloud
question has an answer from the user recorded in [`blockers.md`](blockers.md).

## Files

[`todo.md`](todo.md) · [`forks.md`](forks.md) · [`blockers.md`](blockers.md) ·
[`completed.md`](completed.md) · [`issues.md`](issues.md) · [`deviations.md`](deviations.md)

Convention: [`../../CONVENTIONS.md`](../../CONVENTIONS.md).
