# completed — teardown-housekeeping

The verification log: dated entries saying what landed and **how it was checked**. Append-only;
authoritative for what's done and why we believe it. Records evidence, not item text — the items
live in [`todo.md`](todo.md) with their boxes ticked.

---

## 2026-08-10 — the audit that opened the stream

Not a plan item, but the evidence the plan rests on, recorded so no phase has to re-derive it.

- **Disk:** `df -h /home/wolf` → 1007G total, 94G used, **863G available (10%)**. This is why no
  phase is framed as urgent.
- **Docker:** `docker system df` → images 20 / 16.88GB (5.775GB reclaimable); containers 5 /
  64.74MB; local volumes 19 / 2.384GB (**100% reclaimable, 0 in use**); build cache 151 / 5.077GB.
- **Unpushed branches:** per-branch `git ls-remote --exit-code --heads origin` → `0.2` (9c93bf38),
  `0.2.1` (53b76ad3), `0.2.2` (81bb1997), `sync-experiment` (73a01c2d),
  `claude/loving-feistel-0bda1d` (122e1b8a), `0.3.0` (8335acbd) all absent from origin.
- **Worktrees:** `git worktree list` → two entries; the second is
  `.claude/worktrees/loving-feistel-0bda1d` at 90M.
- **Single-copy credentials:** `../resonantdust_world_old/bin/keys/` holds `r2.env` (mode 600,
  `R2_ACCESS_KEY_ID` + `R2_SECRET_ACCESS_KEY`) and `anthropic.env` (`ANTHROPIC_API_KEY`). Its local
  `.gitignore` ignores `*`, so these were never tracked — values were **not** read or printed
  during the audit, only the variable names via `cut -d= -f1`.
- **Home-directory copies:** `/home/wolf/resonantdust` 14G, git, 3 branches, origin, **8 dirty
  files**; `resonantdust_{client,content,gateway,shared,wasm}` 204K–87M, git, clean;
  `pixijs_archived` 1.8G, `backup` 494M, `master_backup` 35M, `archive` 1.4M — none git.
- **Cloud:** `docker context ls` → a `lightsail` context at `ssh://lightsail-deploy`.
  `0.2.3:deploy/servers/alpha` names `gateway.resonantdust.com` and states the remote standup was
  never finished. Reachability deliberately **not** tested — that is P6, gated on H-B1.

_No plan items ticked yet — the stream was opened and planned on 2026-08-10._
