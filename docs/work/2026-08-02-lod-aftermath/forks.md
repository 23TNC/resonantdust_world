# Forks — LOD aftermath

## F1 — forward, not rollback {#f1}

The user, after the P6 diagnosis: "moving forward with fixes and mitigations instead
of rolling back". Evidence basis: the pre-stream in-place A/B FROZE the renderer under
tick storms HEAD survives (the invalidateAll-per-arrival stampede, fixed by
render-performance I11); rollback also resurrects duplicate atlas tiers and discards
the restored preview kick. Every new-world cost is a named, bounded bug; the old-world
cost was structural. Recorded so a future bad week does not re-litigate it without new
evidence.
