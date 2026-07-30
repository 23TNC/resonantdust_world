# Completed — human-pawns

## 2026-07-30 — P5 complete: graph conformance — the pawn is ONE prim_data. STREAM COMPLETE.

The user's steer mid-phase ("You should have used prim_data to store a pawn, because it
can hold up to 4 primitives … two billboards one for head one for body") is exactly what
landed: the pawn is ONE `prim_data` carrier whose piece slots hold BOTH billboards.

**The mechanics** (`coldShadowData.ts` + `Primitive.carrierOf`/`layer`, MoverLayer wiring):
a slot ≥ 1 warm prim names its owner (`carrierOf` = the body prim's id; explicit-copy
gotcha honored in `addPrim`); `billboardDataFor` takes the CHILD path — no root minted;
the leaf parents on the body's carrier (own drawn anchor stays the resolved position; the
GPU never walks), claims a piece slot via the new generalized `claimPieceSlot` (the light
path's logic), and stamps `layer` = the part slot + bias-8 authored offsets
(round-to-nearest-tile so the unit nibble can't saturate). THE BUG THIS FLUSHED OUT: the
root-prim rewrite used to CLOBBER slots b..d on every re-bake — it now merges the mirror's
piece slots, which also protects carried lights on movers. Frees partition children first
(clear the slot + leaf, no subtree — a shared carrier is never subtree'd through a child).
Bonus fix: the ns perpendicular-card caster's facing parse now handles part-suffixed
segments (`s.1` → side frame `e.1`), so heads get real n/s caster cards.

**Verified live**: record probe — the human's carrier holds `set_a=6 (body idx)` +
`set_b=6 (head idx)`, slots c/d FREE (hands-ready), head leaf `parent=carrier, layer=1`;
corridor↔brute identity **0/0 mismatches over 524,288 words per class** with the
composition live; the fixture walked to the torches and home — its whole shadow (both
parts' contribution) TRACKS the body, no stale column at the origin; it parked at
(104,54) exactly. The wolf soaked on schedule throughout.

**10/10 items done — the stream is COMPLETE.**

## 2026-07-30 — P4 complete: the first human in-world (PLACED — user F6)

Mid-phase the user redirected: "npc's will control groups of pawns. We don't need a npc
for humans right now. We will place the human and the player (or yourself via chrome for
debug) can move it." The `Humans` brain + `Pair` composite + `world` default I had just
built and soak-verified were REVERTED (git history holds them; `brains/mod.rs` records
the group-control future); the npc default stays `wolves`.

**The placed fixture:** human `0x30800005` (`human_female`, payload `PART(0, body 7)` +
`PART(1, head 11)`) minted once at **(104, 54)** — the standing static drill fixture from
the shadow-polish request. Verified: stands at its tile with the authored variants (SQL:
payload row byte-exact, zone 99); SURVIVES npc restarts untouched (the wolves brain's
adoption is kind-scoped); `__bridge.moveEntity` walked it (104,54)→(106,54)→back, both
parts gliding together, and it parked home EXACTLY (micro 0x8600 = (104,54)).

**Joint drill** (zoom 1 + 2, captures in the session record): the human renders body+head
coherent at both zooms — s-facing head with a readable face at zoom 2, head mirroring
correctly on west walks — beside the user's walled compound; the wolf wanders past
unaffected. Shadows: the human casts (its span-2 card makes a LONG shadow — size tuning
territory). THE USER'S EYES are the final oracle — expected feedback: head seat/offset
(`&head.offset.y` in `content/visual/pawns.rd`), overall size (`&body.size`), and the
big-card shadow. Housekeeping: the adopt-first-window race minted a replacement wolf on
one restart (snapshot replay arrived after the 3 s window); the stale wolf was deleted —
one wolf (`0x30800004`) + one human remain.

## 2026-07-30 — P3 complete: multi-part movers live

**MoverLayer renders parts** — a `Mover` owns one warm prim PER SLOT (`parts:
PartPrim[]`; slot 0 = the carrier for selection/hit-testing, `pawnAt` hits ANY part but
returns the carrier's prim id). Slot 0 boxes via the kind's layout as before; other slots
place at `offset`·tilePx from the carrier's base-centre anchor at `scale` × its size,
zIndex a hundredth above per slot, offset.x mirrored on a west facing. Each slot draws its
payload `PART` def — joined from the new `onPawnParts` (either side may arrive first; a
live mover re-applies on join) — else the pawn's own def. `moverSlotTexture` (WorldBridge)
builds `<stem>/<variant>/<facing>[.<part>]`, degrading to the canonical bare stem when the
manifest lacks the variant folder (`TextureResolver.has` → `Viewport.hasTexture`) — the
wolf's variant-0 case. `thingTexture` simplified to cold-things-only (the id-derived
variant pick DELETED).

**Verified live** (redeployed bundles): the wolf renders + walks through the parts path
unchanged (1 slot, `pawn/animal/wolf/e`, npc soak on schedule); a browser-minted human
(`human_female`, payload body 7 + head 11) rendered BOTH parts — head
`pawn/human/female/11/s.1` at 0.625 × the body's size, seated by the authored offset —
and a west walk showed body+head GLIDING TOGETHER mid-trip (no lag), the head mirrored to
the correct side, facing flips coherent. `tick` needed no extra work: `applyVisual` IS the
per-frame move and updates every slot. Drill pawn deleted after the captures (screenshots
in the session record; the user's eyes are P4's oracle).

**Detours worth recording:** (a) an all-zero-surface scare was a PALETTE misread (PIL
returns indices unless `.convert("RGB")`) — the human surfaces were healthy; `bin/art
surface pawn/human/{female,male}` was re-run harmlessly. The WOLF's `surface.e.0.png` IS
genuinely all-zero yet renders — surface coverage is not the draw gate previously assumed
(pawn visibility comes off the albedo alpha path); noted, no action. (b) slot offsets
scale by `size0/slots[0].size` where `size0` is the LAYOUT box (span-derived, 256 px) —
so authored tile offsets run ~1.33× nominal for the span-2 human; the P4 placement drill
tunes the authored value against the user's eyes rather than re-deriving the scale.

## 2026-07-30 — P2 complete: DSL parts

**The loader reads ALL prims** (`shared/dsl/src/loader.rs`): `VisualParts` gains
`parts: Vec<VisualPart>` — one entry per `^prim call`, detected by the `prims.{i}.kind`
stamp `prims_push` writes; per-part fields `part` (default 0), `scale` (default 1,
multiplier on slot 0's size), `offset.x/y` (tiles), plus the slot's own
texture/size/span/anchors/tint/geo. Unit test `a_two_prim_visual_yields_a_parts_list`
(2-prim human shape + the 1-prim wolf shape).

**The corpus**: `human_female`/`human_male` appended to `content/data/things.rd`
(speed 16 tics/tile) and authored in the new `content/visual/pawns.rd` — body slot
(size 1.5, span 2, feet-anchored) + head slot (`part 1`, `scale 0.625` per the user's
spec, `offset.y −1.15` as the first-guess seat, tuned by the P4 drill); the
fit(0-2)/fat(3-5)/average(6-8) grouping recorded in the file's comments. NEW smoke test
`the_real_repo_corpus_loads_and_the_humans_declare_their_parts` loads the REAL `content/`
tree (the check that would have caught the stale manifests): corpus parses, both humans
resolve 2 parts with head scale 0.625, the wolf stays 1-part. DSL suite 47/47.

**The wasm surface**: `moverParts(kind)` (JS objects, one per slot; unknown kinds yield a
default slot) REPLACES `moverPrim` — old export deleted, MoverLayer swapped to slot 0's
tint/geo (multi-slot rendering is P3). Wasm bundle + webgl tsc green.

## 2026-07-30 — P1 complete: variant + part art served end-to-end

**The stem grammar** (`server/edge/src/textures.rs`): stems are now
`<base>[/<variant>][/<facing>[.<part>]]` → leaf `<base>/<variant>/<map>.<facing>.<part>.png`
(defaults `1`/`s`/`0` — every existing stem resolves unchanged). Along the way the module's
doc + tests were found describing a SUPERSEDED layout (`1.s.0/1/albedo.png`) the code never
produced — edge tests are in no build gate, so they'd never run; both were rewritten to the
real canonical shape and now pass (4/4, incl. the new grammar test: full form, variant-only,
part-only, wolf sprite-gen folders, a non-numeric `.suffix` staying a directory).
Verified live after redeploy: curl 200 for female variant 3 body AND part-1 head, a
head-only variant 9, plus the legacy wolf + conifer stems.

**The serving manifest** (`tex_manifest.rs`): every numeric variant folder now registers
(canonical `1` keeps the bare stem; others under `<prefix>/<v>/…`), and every PART present
registers its own stem (`<facing>.<part>` for part ≥ 1), each with its own hash/maps/LOD
row. New test `scans_numeric_variants_and_parts_into_their_own_stems` (body+head variant,
head-only variant, canonical bare stem); 18/18 edge tests green. Live:
`/textures-manifest` carries 75 female stems + legacy rows intact.

**The corpus manifest** (`bin/art manifest`): the stale-empty pawn.rd regenerated with
real entries — `_is_kind_src_dir` learned that subtype-level `sprite.<kind>.<dir>.<part>`
sheets do NOT make their dir a kind (their kind FOLDERS are), and that a numeric variant
subdir holding map leaves does; new `_part_pairs` emits per-part variant counts as
`&part_id`/`&part_variants`. pawn.rd now lists female + male with parts [0, 1] → variants
**[9, 16]** (the acceptance numbers exactly) and the wolf's variant folders. Regenerating
also refreshed the long-stale biome manifests; I2 records the pre-existing
variant-less-kind gap that leaves e.g. `blueprint/wall` unindexed there (nothing consumes
these yet).

**The client resolver**: zero changes needed — stems are opaque to it. Probe on the live
page: `resolve("pawn/human/female/3/e")`, `…/3/e.1`, `…/11/s.1` all return real frames
(`geo:false`) at three DISTINCT 128×128 atlas positions (y = 0 / 320 / 576).

## 2026-07-30 — P0 complete: packed defs + the payload sidecar live in spacetime

**Packed pawn defs end-to-end.** `resolve_thing` now returns the packed
`TYPE_PAWN | species | kind | variant` def (species read off the kind's texture stem
through the new code-owned palette `codec::object::pawn_species_subtype_id` — comment
there explains why NOT the alphabetical manifest subcategories); the worker's `tics_for`
keys packed defs by `def_kind_id` (legacy raw ids still key as-is); the wolves brain
adopts by `def_sans_variant` (new codec helper); MoverLayer decodes the kind from the
packed def's kind half (nonzero high half discriminates packed vs legacy). Verified: all
builds green (codec 60 tests, core 25, worker/orch/master/npc/edge/wasm/webgl-tsc), and
the live soak — the npc resolved `def=0x30010070` and its CREATE minted wolf
`0x30800001`, adopted + wandering on schedule (~2 s/tile trips).

**The sidecar pair, slaved via a macro hook.** `entity_tables!` gained an optional
`state_hook: fn` arm (no-op stamped otherwise; data_shard rebuilt unchanged); the pawn
module declares `payload_log` + `payload` (shapes in `TABLES.md`) and hooks
`payload_follow_state` — the zone re-key rides INSIDE every `entity_state` upsert
transaction. `spawn` gained `payload: Vec<u32>` and writes both sidecar rows in the
mint transaction (empty payload ⇒ no rows). Encoding lives in the new
`shared/codec/src/payload.rs` (`PAYLOAD_OP_PART`, `part_entry`, `payload_parts` +
4 tests: round-trip, unknown-op skip, malformed tail, empty).

**Hops are payload-free STRUCTURALLY** — the worker contains zero sidecar code (grep
clean; I1 records the acceptance re-scope from the planned worker unit test).

**CREATE is variable-arity + type-routed.** `def position count payload×count` (the
`count` word third, same slot as INIT_ZONE's); framed in `codec::action::program`,
excluded from the write/read sets and routes (3 new codec tests incl. framing a trailing
instruction past the payload); the worker's arm routes by `def_type_id` — TYPE_PAWN
spawns, anything else warns by name WITHOUT eating the pending PROMOTE latch reset;
`ACTIONS.md` updated (CREATE = THE creation verb; INIT_ZONE's "one variable-arity verb"
claim corrected). The npc emits `count 0`.

**The fan.** Edge: `ServerMsg::Payload` frame, `payload` joined into the per-zone pawn
subscription (+ on-applied replay for resting pawns) and insert/update callbacks. Core:
mirrors the frame, decodes `PART` entries → `Event::PawnParts` (engine + web transports);
wasm boundary ships flattened (slot, def) pairs; `WasmClient.onPawnParts` exposes them.
Unit: `protocol::tests::a_payload_frame_decodes_to_part_slots` (wire JSON → both slots).

**The live drill** (after `rd redeploy --run --no-reset` — data PRESERVED — + sim-crate
rebuild/restart + legacy raw-def wolf rows deleted by SQL): a marker pawn minted from the
BROWSER (`__bridge.client.queue`) with `PART(0, 0x32001237)` + `PART(1, 0x3200123B)` —
sidecar row landed byte-identical in spawn zone 83, tic 20396; the browser's
`onPawnParts` log received both slots; `moveEntity` walked it (94,56)→(98,56) ACROSS the
zone-83→99 boundary — after arrival the payload row read back **byte-identical, zone
re-keyed 83→99, tic still 20396** (content-change tic preserved). A `TYPE_THING` def
CREATE logged `CREATE rejected: no shard arm for this def type` and minted nothing.
Drill pawn deleted (all four tables); the wolf kept walking throughout.
