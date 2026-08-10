# blockers — teardown-housekeeping

Things needing human input: what blocks, *why* it needs a human, the options, a suggested path.
Open→resolved; resolved rows archive with a date.

A **fork is mine to resolve; a blocker needs the user.** Everything I could decide is in
[`forks.md`](forks.md) H1–H6.

---

## H-B1 — Keep or tear down the 0.2.x cloud resources? · opened 2026-08-10 · OPEN

**What blocks.** P6's last two items. Three remote resources survive the deletion, and I can report
on all three but should not decide any of them.

**What was found (2026-08-10).**

| Resource | Evidence | Status |
|---|---|---|
| AWS Lightsail instance | a `lightsail` docker context exists, endpoint `ssh://lightsail-deploy` | unknown — reachability not tested |
| Cloudflare R2 bucket | `r2.env` credentials exist; bucket/prefix hard-coded in `0.2.3:bin/art` | likely still holding uploaded textures |
| `gateway.resonantdust.com` | named in `0.2.3:deploy/servers/alpha` and `bin/lib/common.sh` | DNS state untested |

`deploy/servers/alpha` says the remote standup "isn't fully wired yet" and every routing row in it
is commented out — so the Lightsail box may have been provisioned and never finished, which is
exactly the shape of thing that bills quietly for months.

**Why it needs you, and not me.** All three are spending decisions on your accounts, and two are
externally visible. Tearing down a Lightsail instance destroys its volumes; emptying an R2 bucket
destroys the uploaded texture corpus (the *second* copy of it — the first is the 45G local copy this
stream is retiring, which is why these two decisions interact); and releasing a domain or its records
is not reversible on a whim. None of that is mine to choose, and none of it is urgent enough to
justify guessing.

**Options.**

1. **Keep all three, do nothing.** Costs whatever they cost; zero risk. Right answer if 0.3.0 is
   expected to deploy within weeks.
2. **Keep R2, tear down Lightsail + the DNS record.** R2 is cheap object storage and holds art;
   the instance is a compute charge for a server that no longer exists in any branch. This is my
   suggested path.
3. **Tear down all three.** Cheapest. Only correct if you are content that the texture corpus lives
   solely in the local copy — and then this stream must *not* retire that copy (H2 would need
   revisiting).

**Suggested path: option 2**, and if you choose it, P6 also needs to confirm the R2 bucket's
contents are intact *before* H2 lets the local copy go, so the art is never down to zero copies.

**What I need from you:** one line per resource — keep or tear down.

---

## Not blockers, but flagged

- **The credentials are currently one `rm -rf` from gone.** `r2.env` and `anthropic.env` exist only
  in `../resonantdust_world_old/bin/keys/`. P0's first item fixes this and everything else waits on
  it. Not a blocker — it needs no decision, just doing.
- **Four branches and 8 dirty files exist only on this machine.** `0.2`, `0.2.1`, `0.2.2`,
  `sync-experiment`, and the working tree of `/home/wolf/resonantdust`. P0 pushes/bundles them. If
  you would rather *not* push the old branches to a public remote, say so — that would turn this
  into a decision, and a bundle in `/home/wolf/archive/` is the alternative.
