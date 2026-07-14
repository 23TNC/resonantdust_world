# Implementation issues log

Genuine problems hit while building the [spacetime rewrite](../spacetime-implementation/README.md),
one file each. Every entry: the **problem**, the **options** considered, the **choice**, and
**why**. This is the decision trail — if a choice turns out wrong, the entry says what the
alternatives were.

Each phase of the build is a **commit** with a `spacetime-rewrite Sn:` header, so a phase is a
rollback point. An issue names the phase it arose in.

## Index

| # | issue | phase | choice |
|---|-------|-------|--------|
| [001](001-multi-target-resolve.md) | committing N targets in one `resolve` | S2 | `Vec<TargetState>` arg |
| [002](002-worker-binding-regen.md) | regenerating worker/non-edge bindings | S3 | copy edge bindings (extend generator later) |
| [003](003-claim-same-worker-reclaim.md) | `claim` refused same-worker re-claim (found live) | S3 | allow owner + unowned + expired |
| [004](004-running-worker-master.md) | running worker/master binaries (no cargo/compose) | S3/S4 | rust+openssl container on host net; compose later |
| [005](005-find-or-mint-payload-decode.md) | cold find-or-mint can't decode into a generic payload | S6/S7 | worker decodes + payload-param `mint_cold` |
