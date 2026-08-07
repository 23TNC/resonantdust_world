# Deviations — lumberjack

## D1 — the P3 code rode a foreign commit (2026-08-07)

A CONCURRENT session (the 2026-08-06-lora-style-not-species art stream) ran `git add -A`
while this session's P3 working tree was dirty: commit `4b1c570c` ("fix(art): run-16
permissions…") carries the intent-queue worker/wasm/npc/content changes alongside the art
stream's docs. Content is correct and in history; the MESSAGE is wrong for those files.
Not rewritten — history rewrite under a live concurrent session risks more than a
mislabeled commit. Mitigation from here: this session commits by EXPLICIT path, not
`add -A`, while the repo is shared.
