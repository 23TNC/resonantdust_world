# Deviations — z positioning

_Log any departure from [`todo.md`](todo.md) AT THE MOMENT of deviating, with the reason._

## 2026-08-01 · P0a item 1 sourced now, ticked at P2

`todo.md` P0a asks for the tilt as "one source, **read by BOTH the record writer and the shadow
transform**". The source now exists (`worldTilt.ts`) and the axis is named, but **nothing consumes it
yet** — the record writer changes in P2b and the shadow transform in P2.

**Left unticked rather than ticked on a partial.** Injecting `WORLD_TILT_GLSL` into shaders that do
not yet use it would satisfy the wording with dead code, which is the opposite of what the acceptance
is for. The item closes when P2 wires the second reader.

**Why the plan had it this way:** P0a was written to pin unknowns *before* any transform existed, so
its acceptance reached forward to consumers that its own phase cannot create. Not worth restructuring
mid-flight — noting it is enough.


## 2026-08-01 · P2's "screen→world transform for `occludes()`" was not needed

The item reads *"Give `occludes()` the screen→world transform it has never had"*. **Built nothing
there**, deliberately.

Once [F9](forks.md#f9) made `unit.z` mean one thing, every term in the height test is a **drawn**
quantity — `Lz`, `cElev`, `subHi`, `targetH` — so they already compare correctly. Adding a uniform
`sin(θ)` to all of them would scale both sides of `hBot ≤ h ≤ hTop` and change nothing, at the cost of
implying the terms were previously inconsistent *there* rather than at `N·L`.

The transform went where a height actually meets a horizontal distance: the two `ldir` expressions.
The item's intent — "stop comparing screen quantities to world ones" — is met; the location it named
was wrong because it was written before [I8](issues.md#i8) was resolved.


## 2026-08-01 · P3's corpus change deferred — RESOLVED, the premise was wrong

**Resolved the same day.** I wrote below that the DSL change was blocked for want of a cargo
toolchain. It was not: `bin/rd build shared` builds the wasm **in docker**, and its own header says
so — *"builds in docker (no host cargo)"*. I checked `PATH` for `cargo`, found none, and concluded
"cannot build" without reading the build script that exists for exactly that situation.

The change is made and verified (see [`completed.md`](completed.md)). **Kept below** because the
mistake is the interesting part: a missing tool on `PATH` is not the same as a missing capability,
and this repo builds everything Rust in containers by design.

### The original entry

## 2026-08-01 · P3's corpus change deferred — no cargo toolchain

`offset.z` is plumbed through `shared/dsl/src/loader.rs` (`VisualPart::elevation`) and exported by
`shared/wasm/src/lib.rs` (`moverParts`), but **neither can be compiled here** — there is no `cargo`
on this machine, and the client loads a prebuilt `shared/pkg/resonantdust_shared_bg.wasm`.

**So `content/visual/pawns.rd` still authors `-0.87 &head.offset.y`.** Changing it to `offset.z` now
would break the head's drawing: the shipped wasm does not know the field, so the head would lose its
shift and draw at the body's feet until a rebuild.

**This costs nothing today.** The carrier link ([P2b](completed.md)) derives a piece's elevation from
the geometry, so head and body already share one footprint with `offset.y` authored. `offset.z`
remains the right authoring model — it disambiguates *"further north on the ground"* from *"higher
up"*, which matters for a part that really is the former — but it is a refinement now, not the
mechanism.

**Post-rebuild, the change is two lines** in `content/visual/pawns.rd`:

```
-0.87 &head.offset.y set        ->        0.87 &head.offset.z set
```

and then re-verify that the head still draws where it does today.
