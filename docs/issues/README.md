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
