# Blockers — texture-generalization

_Rows: what blocks, why it needs the user, options, recommendation._

## B1 · The world doesn't RENDER — the art-128 master re-export is incomplete, and the
## outage flushed the caches that were masking it

Every visual/bit-identity acceptance in this stream needs a renderable world; there isn't
one. Chain of evidence: post-outage the world draws as scrambled dark fragments at all
scales; the P1 reshape is EXONERATED (identical scramble with it stashed, on the committed
build); the texture manifest was additionally corrupted by `textures/biome-thing` having
been moved INSIDE `textures/biome-tile/default/` (fixed — moved back, manifest keys clean);
a cold client cache + fresh derivations still scramble; and the masters themselves are the
smoking gun — `biome-thing/default/conifer/1/albedo.e.0.png` (rewritten by the art session
2026-07-28 15:07) is a NEAR-BLACK silhouette: the re-export moved real color into the split
lanes (`albedo_marigold`/`albedo_residual`) while the serving path still consumes the plain
`albedo`. Pre-outage this was masked by BOTH derivation caches (edge `/tmp` — wiped by the
reboot — and client IndexedDB — cleared during diagnosis); the wolf-invisible bug
(ns-shadows B1) was the same defect at smaller scope. WHY THE USER: the master re-export is
the art-128 migration's in-flight state — finishing or reverting it is that work's call,
and `textures/` is gitignored (no git history to restore from). OPTIONS: (1) finish the
migration so the pipeline composites the split lanes; (2) restore masters from a backup
(`/home/wolf/backup.gate/textures` and `/home/wolf/resonantdust/textures` both exist);
(3) bless me to copy from a backup myself. RECOMMENDATION: (2)/(3) to unblock quickly —
the migration can then proceed against a rendering world. THIS STREAM'S STATE: P0 done +
committed; the P1 reshape is CODE-COMPLETE and typechecks but is UNVERIFIED (the
bit-identity oracle needs the renderable world) — committed as explicit WIP.
