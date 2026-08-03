# Issues — LOD aftermath

## I1 — render-performance coordination: idle, custody taken for ONE bug {#i1}

State read 2026-08-02 ~22:40: `render-performance` is ARMED with 5/22 done. Its I11
(targeted `invalidateStem`, commit `2c908012`) LANDED and measured well (one stem
dirties 916 squares vs 2310; queue drains in 3 ticks). Its P1 complete-on-arrival items
are OPEN and its last resolver-touching commit is ~5 h old — in-flight but idle.

Custody: lod-aftermath lands ONLY the complete-on-arrival re-pack (the black-flora
root) with the smallest possible diff in `ensureCoPack`, and leaves the rest of their
charter (await removal, decode caps, first-paint probes) untouched. A note is written
into their `issues.md` so their session verifies rather than re-implements.

## I2 — the black-flora mechanism, precisely {#i2}

`loadMapBytes` returns null on a 404, and `ensureCoPack` packs the co-pack WITH the
missing quadrant transparent, then sets `packedHash` — so ONE transient 404 (the edge
was down/half-deployed during several of today's boots) permanently bakes an
incomplete co-pack for the session. Nothing retries: the hash matches the manifest, so
even `onManifestChange` keeps it. Flora's layers quadrant read zero on BOTH twins with
healthy disk bytes — the pack simply ran during an outage window. The fix is
completeness bookkeeping + a bounded retry, not a blit bug.
