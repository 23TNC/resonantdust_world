# Forks — texture-restructure

_Execution decisions. The **shape** is Decided upstream in
[`texture-layout/`](../../components/dev/textures/design/texture-layout/README.md) — not re-litigated
here. These are the calls that come up **carrying it out**._

---

## F1 · Variant folder = named OR numeric (decided, user) — 2026-07-19

**Decision.** `<variant>` accepts **both**: numeric (`0..15`, art variations straight off a sprite
sheet — flora etc. have many) and **named** (a linked object's **form**: `wall`/`fence`/`rock`). Both
resolve to `variant_id`; the `type/subtype` registry ([todo P0](todo.md)) maps names → index. **≥16
truncates** out of the manifest (`u4`) — extra on-disk variations exist but go unused, and the manifest
walk must `log()` the drop, not silently cap.

## F2 · `form → variant_id` numbering (decided) — 2026-07-19

**Decided.** Assign `variant_id` in the `type/subtype` registry by an **explicit list** (never
`readdir` order, which is unstable): `wall=0, fence=1, rock=2, …`, low indices for the common forms.
The registry file is the source of truth ([B1](blockers.md)); `texpath.py` never hardcodes it.

## F3 · Linked variants were the SUPERSEDED per-cell split → held-whole atlas (resolved, user) — 2026-07-19

**Resolved (user, 2026-07-19).** The old `linked/<form>.<material>/1.l.0/1..16/` numeric `<variant>`
folders are the **superseded per-cell autotile split**, *not* art variations. The live edge + client
already use a **held-whole autotile atlas**: one master texture holding a `cols×rows` cell grid + an
`atlas.json` (`grid`,`pad`) sidecar, the client sampling a cell by UV
([`tex_manifest.rs`](../../../server/edge/src/tex_manifest.rs) `grid`/`pad`,
[`textureManifest.ts`](../../../client/pixijs/src/textures/textureManifest.ts),
[`SquareCache.ts`](../../../client/pixijs/src/game/viewport/SquareCache.ts); texture-paths.md:
"held whole… Superseded the earlier per-cell `1.l.0/<1..16>` split").

So there is **no per-variant remap**. A linked kind is **one atlas per form**. Target leaf:
`biome-tile/<biome>/<material>/<form>/{albedo,normal,…}.l.0.<ext>` + a sibling **`atlas.json`**; the
per-cell folders are **dropped**. The disk is **half-migrated** — `wall.smooth` is already an atlas,
`wall.blueprint` still the 16-split, `fence.*`/`rock.*` empty — so P3 must **re-master** the
still-split/empty kinds to an atlas, *not* uniformly rename. Full write-up: [`issues.md`](issues.md) I1.

## F4 · Storage needs no change — the dense tile vector is already `Vec<u16>` kind_reference — 2026-07-19

Folding `linked/`→`biome-tile` requires **zero server-storage work**. The dense tile vector already holds
a `u16` `kind_reference` per cell (`kind_id:12 | variant_id:4`) — `shared/codec/src/action.rs` ("tile =
dense `kind_reference` by index") + `server/st-bindings/src/tile/*.rs` (`tiles: Vec::<u16>`). A linked
object is a `kind_reference` with `kind_id ≥ 0x800`; it drops into the existing vector. This stream is
**purely file structure**.
