# Todo — emitter soft shadows (execution order)

_Gated on the corridor↔brute identity diff (`__corridor(false)` + `__gather.debugReadShadow()`,
**0 mismatches**) + visual verification (penumbra widens with emitter). Items move to
[`completed.md`](completed.md) when done **and** verified. Model in [`README.md`](README.md); the
approach fork is [`forks.md`](forks.md)._

_**ALL PHASES COMPLETE + VERIFIED 2026-07-23** → [`completed.md`](completed.md): P0 u9 format, P1 emitter penumbra + umbra (multi-tap, 0 mismatches, 10k partial-coverage texels), P2 perf (9-tap at display cap). F4 (explicit contact-darkening) not needed — physical falloff reads well._
