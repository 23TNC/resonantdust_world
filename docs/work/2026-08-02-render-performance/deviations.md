# Deviations — render performance

_Where execution departed from the plan, written AT THE MOMENT of deviating. "Less churn" is never a
reason._

None yet.

## 2026-08-02 — P1 packs once per SIZE, not once per MAP arrival

**Planned:** *"Let each map upgrade the co-pack independently as it lands … a stem re-packs on each
arrival rather than once at the end."*

**Built:** one pack per `(stem, size)`, when that size's maps have arrived. Progressive reveal comes
from the SIZE ladder (32 px preview → master), not from partial packs.

**Why.** `SpritePool`/`MaxRectsPacker` are **insert-only** — there is no free, release or evict. Every
pack allocates a new frame and the superseded one leaks until the atlas is rebuilt. Packing per map
arrival means up to 4 allocations per (stem, size), 8 per stem across both sizes, of which 6 leak.
At 256² for a master that is more leaked atlas than resident, which trades the load latency this
stream is fixing for the resident-bytes problem [I4](issues.md#i4) is about.

**The user-visible goal still lands**, because the barrier that mattered was *"nothing shows until
the MASTER's four maps arrive"*. The preview's four maps are 2.2–4.3 KB total
([F6](forks.md#f6)) — effectively one round trip — so art appears almost immediately and the master
streams behind it. What is not delivered is `layers` no longer delaying `albedo` **within** a size,
which is a much smaller effect at preview size and invisible at master size once the preview is up.

**To do it properly** wants either a pool free-list or an in-place quadrant re-blit into an already
allocated frame — recorded as [I7](issues.md#i7). Until one exists, per-arrival packing is a
regression dressed as an improvement.
