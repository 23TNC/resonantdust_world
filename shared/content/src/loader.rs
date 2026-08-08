//! Content loader — parse `content/*.toml` into one ready-to-query, fully
//! MATERIALIZED [`Bundle`] (work `2026-08-04-toml-content`; the `.rd` DSL that
//! preceded it is deleted — git holds its history).
//!
//! The corpus is DATA: explicit ids (F1 — they are stored in zones, pawn defs and
//! payload words, so the loader REFUSES duplicates), one record per def, biome
//! classification as declarative rule tables (F3). Schema: `docs/VARIABLES.md`.
//!
//! Every accessor reads precomputed data. The server links this as an rlib while
//! the client reaches it through the `resonantdust-shared` wasm bundle — same
//! content, same ids, both sides.

/// A single load-time problem (a parse error, an id-law violation), tagged with its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
  pub file: String,
  pub message: String,
}

/// The outcome of classifying one tile: which biome claimed the cell and what it
/// decided to place. Names, not ids: worldgen resolves them to `def_id`s through
/// this same bundle before packing.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GenTile {
  /// The biome that claimed this cell (`None` if none matched).
  pub biome: Option<String>,
  /// The ground tile name it chose.
  pub tile: Option<String>,
  /// The primary thing-layer name, or `None` if the cell stays bare.
  pub thing1: Option<String>,
}

/// One packed-map channel's MATERIAL binding on a prim. The stem's `packed` map
/// holds a per-pixel weight in each of its (up to 4) RGBA channels; this says what
/// that weight paints — a `tint` (base `0xRRGGBB` colour) and, optionally, a
/// `material_id` (1-based into the material registry; `0` = none) whose noise-driven
/// hue/chroma jitter the bake pass applies. All-zero = an unbound channel (identity:
/// the residual reconstructs the flat albedo, no variation). See `docs/components/client/webgl/design/lighting.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PackedChannel {
  /// 1-based material-registry id (`material_id`); `0` = no material (flat tint).
  pub material_id: u16,
  /// The channel's base colour, packed `0xRRGGBB`.
  pub tint: u32,
}

/// A named material's rendering parameters (the `<material>` registry, mirrored to
/// the client). `noise_field` names a tiling character field the client resolves to
/// an atlas index; the swings drive hue/chroma jitter in OKLab (never lightness);
/// `sample_space` is `"uv"` (rides the sprite) or `"world"` (pinned to the ground).
/// All-zero swings = identity (smooth tintable).
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialParams {
  /// The noise-field character name (`strand`/`mottle`/…); `""` = no field (flat).
  pub noise_field: String,
  /// Hue rotation amplitude at full noise, in DEGREES.
  pub hue_swing: f64,
  /// Chroma perturbation amplitude at full noise.
  pub chroma_swing: f64,
  /// Warm↔cool asymmetry of the swing, `-1`..`1` (cool..warm).
  pub warm_cool_bias: f64,
  /// `"uv"` (default) or `"world"` — where the noise is sampled.
  pub sample_space: String,
  /// NORMAL-DETAIL field name (material-system P1) — a tiling field RNM-blended onto the
  /// base normal at bake, restoring the high-frequency structure generated normals lack.
  /// `""` = none.
  pub detail_field: String,
  /// Normal-detail amplitude; `0` = off (identity — the base normal is untouched).
  pub detail_amp: f64,
  /// Normal-detail spatial scale (UV tiling multiplier); `1` = the field's native tile.
  pub detail_scale: f64,
}

impl Default for MaterialParams {
  fn default() -> Self {
    Self { noise_field: String::new(), hue_swing: 0.0, chroma_swing: 0.0, warm_cool_bias: 0.0,
           sample_space: "uv".into(), detail_field: String::new(), detail_amp: 0.0, detail_scale: 1.0 }
  }
}

/// The visual parts a def declares. `tint` multiplies a LOADED texture (usually
/// white/no-op for full-colour art); `geo_color` is the flat silhouette colour shown
/// while the prim is still on the geo tier (defaults to `tint` when the def sets
/// none); `texture` is the sprite STEM (`None` when the def declares no texture, or
/// declares the built-in `"white"` fill). Shared by tiles and things.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualParts {
  pub tint: u32,
  pub geo_color: u32,
  pub texture: Option<String>,
  /// The tiles this prim OCCUPIES — `(w, h)`, default `(1, 1)`. Logical grid extent
  /// (movement / hit-testing), authored in the object's own frame; the object's
  /// position is its TOP-LEFT tile. Rotation swaps `w`/`h` for an n/s facing.
  /// **Server-side occupancy is deferred** (`docs/components/shared/codec/design/object-model.md`).
  pub footprint: (f64, f64),
  /// The prim's logical ANCHOR within its footprint — `(x, y)` in `0..1`, default
  /// `(0.5, 0.5)` (centre). `(0.5, 1.0)` is bottom-centre. The `sprite_anchor`
  /// point of the art is pinned here.
  pub anchor: (f64, f64),
  /// The sprite's on-screen scale in TILES, default `1.0`. SUPERSEDED by `span` +
  /// `sprite_scale` (def-frame-anchors P5) — kept while legacy consumers migrate.
  pub size: f64,
  /// The sprite frame's WORLD SPAN in tiles — pow2, ≤ one zone — default `1.0`.
  pub span: f64,
  /// Pre-atlas sprite scale `(w, h)`, default `(1, 1)`: applied to the decoded sprite
  /// BEFORE it is packed.
  pub sprite_scale: (f64, f64),
  /// The pivot ON THE SPRITE that aligns to `anchor` — `(x, y)` in `0..1`, default
  /// `(0.5, 0.5)`. Measured against the [`DirFrame::sub`] SUBFRAME (subframe-ingest F7).
  pub sprite_anchor: (f64, f64),
  /// Per-(variant, rotation) subframe + pivot (subframe-ingest F1/F7/I10):
  /// `[VARIANTS_PER_DEF][ROTATIONS_PER_DEF]`, resolved through the authoring
  /// fallback chain at LOAD (most-specific first: `v3.r1 → v3.e → v3 → r1 → e → base`,
  /// per COMPONENT). Index 3 (west) is the east master mirrored, derived by the
  /// client — never authored.
  pub dir_frames: [[DirFrame; ROTATIONS_PER_DEF]; VARIANTS_PER_DEF],
  /// The prim's up-to-4 packed-map channel material bindings, indexed by packed RGBA
  /// channel. Default (all zero) = no material system in play.
  pub packed: [PackedChannel; 4],
  /// The LIGHT this kind emits, or `None` when it emits nothing. Authored per KIND,
  /// not per instance (work `2026-07-25-primitive-graph`).
  pub light: Option<LightParts>,
  /// EVERY part slot, in slot order — the pawn PARTS list (human-pawns P2).
  /// `parts[0]` mirrors the flat prim-0 fields above (a wolf's whole visual = one
  /// entry); a human carries body + head.
  pub parts: Vec<VisualPart>,
}

/// One DIRECTION's authored art rect and pivot (subframe-ingest F1/F7). Both are
/// FRACTIONS (`0..1`): a fraction addresses the same art at every served size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DirFrame {
  /// `(x, y, w, h)` of the art inside the master, `0..1`. Default `(0, 0, 1, 1)` — the
  /// whole frame, i.e. "this master is already tight".
  pub sub: (f64, f64, f64, f64),
  /// The pivot ON [`Self::sub`] that pins to the prim's `anchor`, `0..1`. Default `(0.5, 0.5)`.
  pub anchor: (f64, f64),
}

impl Default for DirFrame {
  fn default() -> Self {
    Self { sub: (0.0, 0.0, 1.0, 1.0), anchor: (0.5, 0.5) }
  }
}

/// Rotation slots per definition — **the one index space for facings and linked cells alike**
/// (subframe-ingest F9). A sprite uses the first four (`0 = s, 1 = e, 2 = n, 3 = w`) and a
/// linked tile uses all sixteen (the autotile cell, `y·4 + x`). Mirrors `ROTATIONS_PER_DEF`
/// in `client/webgl`'s `records.ts` — the two must move together.
pub const ROTATIONS_PER_DEF: usize = 16;

/// Variant slots per definition. **A variant and a rotation are DIFFERENT AXES**
/// (subframe-ingest I10) — a facing lands in the STEM, a variant in the CELL — so a
/// subframe is indexed by BOTH: `[variant][rotation]`.
pub const VARIANTS_PER_DEF: usize = 16;

/// Band slots per need / need slots per kind — the authoring caps (needs-moodlets P1).
pub const NEED_BANDS: usize = 4;
pub const NEEDS_PER_KIND: usize = 8;

/// One BAND on a need's satisfaction — the DERIVED condition active while
/// `lo <= satisfaction < hi` (needs-moodlets F2: band conditions are computed by every
/// observer from `(need row, tic, corpus)`; no grant events exist for them). Bands are
/// authored EXCLUSIVE, so at most one band condition per need is active.
#[derive(Debug, Clone, PartialEq)]
pub struct NeedBand {
  /// The `<condition>` name this band activates (resolved via [`Bundle::condition_id`]).
  pub condition: String,
  /// Band start (inclusive), in the need's OWN units (interactions F7). Default `0`.
  pub lo: f64,
  /// Band end (exclusive), in the need's own units.
  pub hi: f64,
}

/// A `[[need]]` def's parameters (needs-moodlets P1, stat-model F4/F6). A need is a
/// SATISFACTION on its OWN authored `min..max` domain (the corpus decides range and sign,
/// the system stores and CLAMPS — stored as u16 FIXED-POINT on exactly these bounds),
/// depleting toward `min`; nothing ticks it — observers compute `satisfaction_at(tic)`
/// from `deplete` (F4), piecewise across condition expiries (stat-model F7).
#[derive(Debug, Clone, PartialEq)]
pub struct NeedParams {
  /// Display label ("Thirst") — authoring/debug only. The need itself is NEVER shown.
  pub label: String,
  /// The authored domain floor — depletion's resting point AND the fixed-point encoding
  /// floor (stat-model F4). `min` may be negative: min/max IS the sign treatment (F7).
  pub min: f64,
  /// The authored domain ceiling — "full". Default `1` (the pre-F7 fractional domain).
  pub max: f64,
  /// The empty-intersection rule for modifier ranges (stat-model F6): `true` = the
  /// combined minimum stands (`winner = "min"`, the default), `false` = the maximum.
  pub min_wins: bool,
  /// TICS from `max` to `min` at BASE rate. `0` = unauthored (the need never drains).
  pub deplete: f64,
  /// The derived-condition bands, in authored slot order.
  pub bands: Vec<NeedBand>,
}

/// One STAT contribution of a trait level or a condition (stat-model F5/F6): `add` SUMS
/// into the stat's value; `min`/`max` join the range intersection (max-of-mins /
/// min-of-maxes) inside the stat's authored global bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct StatModifier {
  pub stat: String,
  pub add: f64,
  pub min: Option<f64>,
  pub max: Option<f64>,
}

/// One NEED modifier of a trait level or a condition (stat-model F6/F7): `rate` MULTIPLIES
/// the depletion rate (multipliers form a product; `1.0` = no change); `min`/`max` narrow
/// the need's effective clamp inside its authored domain. Any mutation of the modifier set
/// re-stamps the affected need rows — the re-stamp law (F7).
#[derive(Debug, Clone, PartialEq)]
pub struct NeedModifier {
  pub need: String,
  pub rate: f64,
  pub min: Option<f64>,
  pub max: Option<f64>,
}

/// A `[[stat]]` def (stat-model F6/F8) — a DERIVED quantity: never stored, never fanned;
/// computed anywhere from the pawn's trait/condition rows + this corpus.
#[derive(Debug, Clone, PartialEq)]
pub struct StatParams {
  pub label: String,
  /// Authored GLOBAL safety bounds — the outermost clamp.
  pub min: f64,
  pub max: f64,
  /// The empty-intersection rule (F6): `true` = `winner = "min"` (the combined minimum
  /// stands), `false` = `"max"`.
  pub min_wins: bool,
}

/// A `<condition>` def's parameters — the DISPLAYED consequence of hidden state.
/// `duration == 0` marks a DERIVED condition (alive exactly while its band holds);
/// `> 0` a TIMED one (a stored row expiring `duration` tics after its written tic).
///
/// A condition's effects are an OPEN set (conditions F6): `stats`/`needs`/`emotions`
/// modifier lists, all feeding the SAME machinery traits feed (stat-model F5/F6;
/// emotions F2 — MOOD is retired, emotions F4).
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionParams {
  pub label: String,
  /// Lifetime in TICS for a stored grant; `0` = DERIVED (band-computed).
  pub duration: f64,
  /// Stat contributions while active (SCALARS — conditions have no levels).
  pub stats: Vec<StatModifier>,
  /// Need modifiers while active. A grant/expiry of a condition carrying one of these is
  /// exactly the mutation the re-stamp law (F7) speaks about.
  pub needs: Vec<NeedModifier>,
  /// Emotion contributions while active (emotions F2) — what this condition makes the
  /// pawn FEEL. Feeds the active-emotion argmax and the card's pie slices.
  pub emotions: Vec<EmotionModifier>,
  /// Display PRIORITY, descending — the details panel maximizes the top 4 and minimizes the
  /// rest (conditions F2). AUTHORED, never derived (intensity cannot express "mild but
  /// urgent"). Absent = `0`. Author in tens so a new condition can be slotted between two
  /// without renumbering. The full sort — `priority` desc, Σ emotion magnitude desc,
  /// `condition_id` asc — lives in [`crate::needs_eval::active_conditions`], NOT in any
  /// consumer (F3).
  pub priority: i32,
}

/// One `[[emotion]]` def (emotions F1): the u4 DECLARATION INDEX is the whole identity —
/// `fine` is REQUIRED first (index 0 = the "alters nothing" default). Never
/// registry-numbered; emotions never ride the wire.
#[derive(Debug, Clone, PartialEq)]
pub struct EmotionParams {
  pub label: String,
  /// `0xRRGGBB` — the pie slice / panel wash color.
  pub color: u32,
}

/// One emotion contribution (emotions F2): `magnitude` is 1..=15 (u4; +0 is authored by
/// ABSENCE). Packs to the canonical u8 via [`pack_emotion_modifier`] (I3 — ONE packer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmotionModifier {
  /// The emotion's u4 declaration index.
  pub emotion: u8,
  pub magnitude: u8,
}

/// The ONE u8 pack for an emotion modifier — `emotion:4 | magnitude:4` (the user's
/// layout, emotions I3). Its inverse is [`unpack_emotion_modifier`].
pub fn pack_emotion_modifier(m: EmotionModifier) -> u8 {
  (m.emotion << 4) | (m.magnitude & 0x0f)
}

/// See [`pack_emotion_modifier`].
pub fn unpack_emotion_modifier(b: u8) -> EmotionModifier {
  EmotionModifier { emotion: b >> 4, magnitude: b & 0x0f }
}

/// One LEVEL of a trait — the modifier set active at that level (stat-model F5). Index in
/// [`TraitParams::levels`] = level − 1; level 0 = the trait absent.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TraitLevel {
  pub stats: Vec<StatModifier>,
  pub needs: Vec<NeedModifier>,
  /// Emotion contributions at this level (emotions F2 — the per-level array form).
  pub emotions: Vec<EmotionModifier>,
}

/// A `[[trait]]` def — a LEVELED stat contributor, "traits/skills" (stat-model F1/F5).
/// Thing defs bind starting `(trait, level)` pairs minted at CREATE (F11); a pawn's row
/// stores its level, and the level indexes the authored table here.
#[derive(Debug, Clone, PartialEq)]
pub struct TraitParams {
  pub label: String,
  /// The per-level modifier tables, index = level − 1. Never empty (a marker trait with
  /// no modifiers still has one empty level).
  pub levels: Vec<TraitLevel>,
  /// Free-form capability TAGS (attack F1 — the user's generalization): `bite` authors
  /// `tags = ["attack"]`, and a TAG-check affordance passes for ANY carried trait
  /// bearing the tag — new attack forms (claw, peck) are pure content, no new
  /// affordances. Tags never ride the wire; they resolve through the corpus at eval.
  pub tags: Vec<String>,
}

/// One operand of an interaction effect (interactions F5): an `"@name"` reference into the
/// interaction's declared `inputs`, a bare name constant (a def baked into the effect), or
/// a number constant. `@` references resolve to their input INDEX at load — a dangling one
/// refuses the load.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
  /// The index into [`InteractionParams::inputs`] the caller's event word binds.
  Input(usize),
  /// A baked name (e.g. `need = "thirst"` on an interaction that only ever drinks water).
  Name(String),
  /// A baked number.
  Value(f64),
}

/// The `satisfy` effect: move `need`'s satisfaction on `target` by the SIGNED `amount`
/// (clamped to the need's authored domain by the executor).
#[derive(Debug, Clone, PartialEq)]
pub struct SatisfyEffect {
  pub target: Operand,
  pub need: Operand,
  pub amount: Operand,
}

/// The `move` effect (input-rework F6): walk `target` to `to` — the worker queues the
/// movement-chain seed (`PROMOTE MOVE_TO`), spaced by the pawn's DERIVED `ground_speed`.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveEffect {
  pub target: Operand,
  pub to: Operand,
}

/// An `[[interaction]]` def — something a pawn can DO (interactions F5): an input
/// SIGNATURE plus declarative effects binding inputs or constants, gated by affordance
/// PREDICATES (stat-model F5).
#[derive(Debug, Clone, PartialEq)]
pub struct InteractionParams {
  pub label: String,
  /// The pie-menu label (input-rework F1); defaults to `label`.
  pub menu_text: String,
  /// The affordance gates — EVERY listed predicate must pass for the acting pawn.
  pub affordances: Vec<String>,
  /// The declared input signature — event input words bind these IN ORDER. Names are the
  /// RESERVED menu vocabulary (input-rework F5): `pawn`/`destination`/`amount`.
  pub inputs: Vec<String>,
  /// The satisfy effect, if the interaction moves a need.
  pub satisfy: Option<SatisfyEffect>,
  /// The move effect, if the interaction walks the pawn (input-rework F6). At least one
  /// of `satisfy`/`move_effect`/`destroy` must be authored.
  pub move_effect: Option<MoveEffect>,
  /// TIMED condition grants on execute (expiry from each condition's own `duration`).
  pub grants: Vec<String>,
  /// The destroy effect (lumberjack F5): `"carrier"` clears the validated offerer's cell
  /// via the cold-overlay `SET … kind 0`. The `yields` successor (place a thing where the
  /// carrier stood — lumberjack I9) is RESERVED beside it, not built.
  pub destroy: Option<String>,
  /// The spawn effect (food-chain F5/F6): write `thing`'s kind into a cell — `at = "on"`
  /// = the TARGET pawn's floor cell (death's meat); `"adjacent"` = the first EMPTY
  /// pathable cell of the CARRIER's 3×3, fixed (dy, dx) scan (forage's plant matter).
  pub spawn: Option<SpawnEffect>,
  /// The remove effect (food-chain F5): `"target"` deletes the target pawn via the pawn
  /// shard's `remove` reducer — death's second half. Idempotent at execution.
  pub remove: Option<String>,
  /// The store effect (inventory F4): `"carrier"` — the validated offerer leaves the
  /// world (the destroy tombstone lane) and its kind def lands in the acting pawn's
  /// first free inventory slot, with the free-count `SET_NEED` in the SAME program
  /// (F3/I3 — rows and count never diverge).
  pub store: Option<String>,
  /// The placement rule (input-rework F4 / lumberjack F2): `"on"` = the ACTING pawn
  /// stands on the carrier; `"adjacent"` = Chebyshev ≤ 1 from the carrier's cell,
  /// INCLUSIVE of it; `"target"` = the DESTINATION tile is the carrier; `"self"` = the
  /// carrier IS the target pawn (food-chain I9); `"slot"` = the carrier is an inventory
  /// SLOT of the acting pawn (inventory F5 — the slot index rides the inputs).
  pub location: String,
  /// TICS this interaction takes (lumberjack, consuming the I9 reservation): 0 =
  /// instantaneous; N > 0 queues a completion event at +N which RE-VALIDATES
  /// affordances + location at fire (ACTIONS.md § The intent queue).
  pub duration: f64,
  /// The intent-queue DISPLAY block (intent-queue-ui F2) — how this interaction's
  /// circle renders in the details panel's strip, and whether a click may cancel it
  /// while executing. All presentation; the worker reads only `cancelable`.
  pub queue: QueueVisual,
}

/// The spawn effect's authored shape — either a NAMED thing at a placement
/// (food-chain F5/F6) or `"carried"` (inventory F5): spawn the acting pawn's SLOT
/// item beside it (the adjacent scan), which is what makes ONE drop generic over
/// every item. `Carried` REFUSES when no empty pathable cell exists — the item
/// stays held (inventory I10; never forage's all-full-swallows rule).
#[derive(Debug, Clone, PartialEq)]
pub enum SpawnEffect {
  Thing { thing: String, at: String },
  Carried,
}

/// An interaction's queue-strip presentation (intent-queue-ui F2; schema in
/// `VARIABLES.md`). Defaults = a standard neutral circle, no ring, not cancelable.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueVisual {
  /// Tooltip text; `None` = the interaction's `label`.
  pub hover: Option<String>,
  /// Circle scale, 1.0 = standard (move_to authors 0.6).
  pub size: f64,
  /// Circle fill color (`0xRRGGBB`), `None` = the panel's neutral.
  pub background: Option<u32>,
  /// The ACTIVE entry's ring: `"cw"` | `"ccw"` | `"none"`.
  pub progress: String,
  /// Ring color, `None` = the panel's neutral.
  pub progress_color: Option<u32>,
  /// `true` = the ring FILLS as the event progresses; `false` = it empties.
  pub progress_fill: bool,
  /// May a click cancel this interaction WHILE EXECUTING (pending entries are always
  /// removable).
  pub cancelable: bool,
}

impl Default for QueueVisual {
  fn default() -> Self {
    QueueVisual {
      hover: None,
      size: 1.0,
      background: None,
      progress: "none".into(),
      progress_color: None,
      progress_fill: true,
      cancelable: false,
    }
  }
}

/// The ONE placement-rule range check (lumberjack F2), shared by the worker's
/// EXECUTE_INTERACTION gate and the wasm pie-menu filter so the two cannot drift:
/// `cheb` is the Chebyshev distance in tiles between the acting pawn and the carrier
/// cell. `"on"` = standing on it; `"adjacent"` = on or beside (≤ 1, INCLUSIVE);
/// `"target"` places no constraint on the pawn (movement's rule).
pub fn location_in_range(location: &str, cheb: u32) -> bool {
  match location {
    "on" => cheb == 0,
    "adjacent" => cheb <= 1,
    // "self" (food-chain I9): the carrier IS the target pawn — distance is definitionally
    // zero, so no spatial constraint. "slot" (inventory F5): the carrier is a slot OF the
    // acting pawn — likewise zero. "target" places none (movement's rule).
    _ => true,
  }
}

#[cfg(test)]
mod location_tests {
  use super::location_in_range;

  #[test]
  fn the_placement_rules_range_exactly() {
    // "on" is standing-on only; "adjacent" is INCLUSIVE Chebyshev ≤ 1 (lumberjack F2 —
    // the wolf drinking while standing ON the pond stays legal); "target" never gates.
    assert!(location_in_range("on", 0));
    assert!(!location_in_range("on", 1));
    assert!(location_in_range("adjacent", 0));
    assert!(location_in_range("adjacent", 1));
    assert!(!location_in_range("adjacent", 2));
    assert!(location_in_range("target", 0));
    assert!(location_in_range("target", 7));
  }
}

/// An `[[affordance]]` def — a named PREDICATE over a pawn's STATS or a NEED's lazy
/// value (stat-model F5/F10; food-chain F4): `can_move_ground` ⇔ `ground_speed > 0`;
/// `can_die` ⇔ `corpus ≤ 0`. Structured, not an expression string.
#[derive(Debug, Clone, PartialEq)]
pub struct AffordanceParams {
  pub label: String,
  pub check: AffordanceCheck,
}

/// The predicate body (food-chain F4). A NEED check evaluates the lazy value at `now`
/// and DOUBLES as the need-write TRIGGER key (F5 — the worker sweeps a target's
/// carried interactions for checks on the need it just wrote).
#[derive(Debug, Clone, PartialEq)]
pub enum AffordanceCheck {
  /// Exactly one of `above`/`below` is authored (both thresholds EXCLUSIVE).
  Stat { stat: String, above: Option<f64>, below: Option<f64> },
  /// `satisfaction_at(now) <cmp> value` on the need's effective domain.
  Need { need: String, cmp: Cmp, value: f64 },
  /// Passes iff ANY carried trait authors this TAG (attack F1 — capability by tag:
  /// `can_attack = { tag = "attack" }` matches bite, claw, peck… pure content).
  Tag { tag: String },
}

impl AffordanceParams {
  /// The need a `Need` check reads (the trigger key), if any.
  pub fn trigger_need(&self) -> Option<&str> {
    match &self.check {
      AffordanceCheck::Need { need, .. } => Some(need),
      AffordanceCheck::Stat { .. } | AffordanceCheck::Tag { .. } => None,
    }
  }
}

/// One part SLOT of a kind's visual skeleton (human-pawns P2). Slot index =
/// declaration order; slot 0 boxes the carrier.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualPart {
  pub tint: u32,
  pub geo_color: u32,
  /// The sprite STEM (`None` = a flat tint rect / the built-in `white`).
  pub texture: Option<String>,
  /// The master FILE part suffix (`<facing>.<part>` — a head is part 1).
  pub part: u32,
  /// The slot's pre-atlas art scale (uniform), def-frame-anchors semantics.
  pub scale: f64,
  /// Tile-space draw offset from the carrier's anchor.
  pub offset: (f64, f64),
  /// ELEVATION above the ground plane, tiles (z-positioning) — projects to a y-shift.
  pub elevation: f64,
  /// Per-slot draw depth (pawn-part-placement P2) — negated when facing away.
  pub depth: f64,
  /// The slot's own frame fields, same semantics as the flat prim-0 copies above.
  pub size: f64,
  pub span: f64,
  pub sprite_scale: (f64, f64),
  pub sprite_anchor: (f64, f64),
  pub anchor: (f64, f64),
  pub dir_frames: [[DirFrame; ROTATIONS_PER_DEF]; VARIANTS_PER_DEF],
}

/// The deterministic per-tile draw — `<salt> ^rand` / a biome rule's scatter roll:
/// SplitMix64 finalizer over `seed ^ salt·φ`, yielding `[0, 1)`. Different salts give
/// independent draws for one tile. **The one derivation** — the `.rd` VM and the TOML
/// rule classifier both call this, so no scatter re-rolls across the migration
/// (toml-content P0 pinned it).
pub fn tile_rand(seed: u64, salt: i64) -> f64 {
  let mut h = seed ^ (salt as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
  h ^= h >> 30;
  h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
  h ^= h >> 27;
  h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
  h ^= h >> 31;
  (h >> 11) as f64 / (1u64 << 53) as f64
}

// ── materialized defs (internal; toml_loader fills these too) ─────────────────────

/// A definition's TAXONOMY — the four names that identify it (work
/// `2026-08-04-definition-registry` F1). The corpus authors these; the server numbers them into a
/// `definition_reference` through the registry.
///
/// `sub_type` and `variant` are APPLICABILITY ARRAYS ([F2]): the def applies to every tuple in the
/// cross-product, so one `conifer` covers `[forest, plains, grassland] × [0..15]` rather than 48
/// blocks. They are also the TEXTURE PATH — `<type>/<sub_type>/<kind>/<variant>` is the art tree's
/// shape, which is why the stem is derived rather than authored.
///
/// Absent (`None`) on a def that has not been given a taxonomy yet — the field is being introduced
/// additively so the corpus and the golden fixture stay green throughout ([I9]).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Taxonomy {
  pub type_name: String,
  pub kind: String,
  /// Every subType this def applies to. Never empty when a taxonomy is present.
  pub sub_type: Vec<String>,
  /// Every variant label it applies to — `"0".."15"` for art variations, or a named form
  /// (`"wall"`). Never empty when a taxonomy is present.
  pub variant: Vec<String>,
}

impl Taxonomy {
  /// The texture stem this taxonomy addresses. The corpus used to author this string; deriving it
  /// is the point of authoring the taxonomy at all.
  ///
  /// **A NAMED variant is part of the address; a NUMERIC one is not.** This is the linked-vs-plain
  /// split `VARIABLES.md` already draws: a linked object's `variant` is its FORM (`wall`, `fence`,
  /// `rock`) and names a distinct art folder, so it belongs in the stem —
  /// `biome-tile/default/smooth/wall`. A plain def's `variant` is an art VARIATION index that
  /// worldgen rolls per cell, so the stem stops at the kind and the resolver appends the roll —
  /// `biome-thing/default/conifer`, then `/4/e` at resolve time.
  ///
  /// Uses the FIRST subType, which is what the single-subType defs (every one today) resolve to.
  /// A multi-subType def's art is shared across its subTypes by construction — the stem names the
  /// kind, and the biome only decides where it is scattered.
  pub fn stem(&self) -> String {
    let sub = self.sub_type.first().map(String::as_str).unwrap_or("default");
    let base = format!("{}/{}/{}", self.type_name, sub, self.kind);
    match self.variant.as_slice() {
      // Exactly one variant, and it is a NAME rather than an index → part of the address.
      [only] if only.parse::<u32>().is_err() => format!("{base}/{only}"),
      _ => base,
    }
  }

  /// Every `(sub_type, variant)` pair this def applies to — [F2]'s cross-product, which is one
  /// registry row each.
  pub fn tuples(&self) -> Vec<(&str, &str)> {
    let mut out = Vec::with_capacity(self.sub_type.len() * self.variant.len());
    for s in &self.sub_type {
      for v in &self.variant {
        out.push((s.as_str(), v.as_str()));
      }
    }
    out
  }
}

/// One tile def, fully evaluated. `name` may be `""` for a RETIRED id (an F1 hole).
#[derive(Debug, Default, Clone)]
pub(crate) struct TileDef {
  pub name: String,
  /// Which REVISION ([F17]). Unauthored = 0; every live version stays authored.
  pub version: u32,
  /// The authored taxonomy, or `None` while the corpus is mid-migration ([I9]).
  pub taxonomy: Option<Taxonomy>,
  pub color: Option<u32>,
  pub visual: Option<VisualParts>,
  pub build: Option<String>,
  pub height: Option<f64>,
  /// `[linked_w, linked_h, padding, rotation, cast_shadow, receives_shadows]`.
  pub lanes: [f64; 6],
  /// Interaction bindings: what this carrier OFFERS (stat-model F5/F9 — the
  /// interaction carries its own affordance gate).
  pub interactions: Vec<InteractionBind>,
  /// May a pawn ENTER this tile? (pathfinding F1 — absence authors true; the
  /// `Default` false only reaches RETIRED slots, which the accessor guards.)
  pub pathable: bool,
}

/// One carrier→interaction binding — the PER-CARRIER lane (stat-model F9 for the
/// offer + magnitude; logs-drop F1 for the yield). The water binds `drink 3`; the
/// tree binds `cut_down` with `yields = "logs"`.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct InteractionBind {
  pub name: String,
  /// The carrier's magnitude for the `amount` input (the pie menu bakes it in).
  pub magnitude: f64,
  /// The thing a destroy-effect interaction leaves at this carrier's cell
  /// (logs-drop F1/F2) — load-validated against thing kinds; `None` = clear.
  pub yields: Option<String>,
}

/// One thing def, fully evaluated. `name` may be `""` for a retired id.
#[derive(Debug, Default, Clone)]
pub(crate) struct ThingDef {
  pub name: String,
  /// Which REVISION ([F17]). Unauthored = 0; every live version stays authored.
  pub version: u32,
  /// The authored taxonomy, or `None` while the corpus is mid-migration ([I9]).
  pub taxonomy: Option<Taxonomy>,
  pub color: Option<u32>,
  pub visual: Option<VisualParts>,
  /// The need NAMES this kind carries (`needs = [...]`), load-validated; consumers read
  /// gameplay refs through [`Bundle::thing_needs`] (interactions F1).
  pub needs: Vec<String>,
  /// The STARTING trait bindings — `(trait name, level ≥ 1)` — minted at CREATE
  /// (stat-model F11). A bare-string binding authors level 1.
  pub traits: Vec<(String, u16)>,
  /// Interaction bindings (stat-model F5/F9; the yield lane — logs-drop F1).
  pub interactions: Vec<InteractionBind>,
  /// May a pawn ENTER a cell this thing occupies? (pathfinding F1 — absence authors
  /// true; the `Default` false only reaches RETIRED slots, which the accessor guards.)
  pub pathable: bool,
}

/// One biome, with its classifier body in either dialect.
#[derive(Debug, Clone)]
pub(crate) struct BiomeDef {
  pub name: String,
  /// The STORED subtype id (`0` = none/reserved — `biome_subtype_id` returns `None`).
  pub subtype: u16,
  pub body: BiomeBody,
}

/// The classifier body — the declarative rule form (F3). An enum still, so a future
/// dialect slots in behind the same `generate()` without touching consumers.
#[derive(Debug, Clone)]
pub(crate) enum BiomeBody {
  Rules(BiomeRules),
}

/// A comparison op, spelled exactly as the retired `.rd` ops were (F3): the golden
/// gate depends on reproducing the same half-open edges. Public since food-chain F4 —
/// need-check affordances carry one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cmp {
  Gte,
  Gt,
  Lt,
  Lte,
}

impl Cmp {
  pub fn pass(self, v: f64, t: f64) -> bool {
    match self {
      Cmp::Gte => v >= t,
      Cmp::Gt => v > t,
      Cmp::Lt => v < t,
      Cmp::Lte => v <= t,
    }
  }
}

/// The declarative biome body: a CONJUNCTION of dimension thresholds (the corpus never
/// used `or` — toml-content I1), a ground tile, and ordered scatter draws where the
/// LAST passing draw wins the cell (the `.rd` overwrite semantics).
#[derive(Debug, Clone, Default)]
pub(crate) struct BiomeRules {
  /// `(dimension index, op, threshold)` — all must pass.
  pub when: Vec<(usize, Cmp, f64)>,
  pub tile: Option<String>,
  /// `(salt, p, thing)` — rolls `tile_rand(seed, salt) < p`.
  pub scatter: Vec<(i64, f64, String)>,
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn fnv_bytes(h: &mut u64, bytes: &[u8]) {
  for &b in bytes {
    *h ^= b as u64;
    *h = h.wrapping_mul(0x0000_0100_0000_01b3);
  }
}

fn fnv_str(h: &mut u64, s: &str) {
  fnv_bytes(h, s.as_bytes());
  fnv_bytes(h, b"\0"); // separator, so ("ab","c") and ("a","bc") differ
}

/// Everything the runtime needs, built once at load — REGISTRIES in id order
/// (index = id − 1; a `""`-named slot is a retired id) + evaluated params.
#[derive(Default, Debug)]
pub struct Bundle {
  pub(crate) tiles: Vec<TileDef>,
  pub(crate) things: Vec<ThingDef>,
  /// Evaluation order = priority (first match wins), NOT an id namespace.
  pub(crate) biomes: Vec<BiomeDef>,
  pub(crate) materials: Vec<(String, MaterialParams)>,
  /// The six GAMEPLAY category registries (interactions F1, stat-model), corpus order —
  /// identity is the registry-allocated `gameplay/<category>/<name>/default` def ref,
  /// never a position.
  pub(crate) needs: Vec<(String, NeedParams)>,
  pub(crate) conditions: Vec<(String, ConditionParams)>,
  pub(crate) traits: Vec<(String, TraitParams)>,
  pub(crate) interactions: Vec<(String, InteractionParams)>,
  pub(crate) affordances: Vec<(String, AffordanceParams)>,
  pub(crate) stats: Vec<(String, StatParams)>,
  /// The SIXTEEN emotions, declaration order — index = the u4 identity (emotions F1;
  /// `fine` first, never registry-numbered).
  pub(crate) emotions: Vec<(String, EmotionParams)>,
  /// `(type, name) → subtype_id` for subtype axes with no record of their own — pawn species today
  /// (definition-registry F16). A biome's subtype is authored on the biome instead.
  pub(crate) subtypes: Vec<(String, String, u16)>,
  /// Registry name caches (id order) — what the slice-returning accessors serve.
  tile_names: Vec<String>,
  thing_names: Vec<String>,
  biome_names: Vec<String>,
  material_names: Vec<String>,
  need_names: Vec<String>,
  condition_names: Vec<String>,
  trait_names: Vec<String>,
  interaction_names: Vec<String>,
  affordance_names: Vec<String>,
  stat_names: Vec<String>,
  /// The REGISTRY override for `name → definition_reference` (definition-registry P4/P5), keyed
  /// `(is_tile, name)`. `None` means "resolve from corpus position", which is what a registry-less
  /// boot and every unit test does.
  ///
  /// The FULL packed def, not just the kind half. The kind half is what `tile_def_id` and
  /// `thing_object_id` serve, but the type and subtype halves are the taxonomy's numbering too —
  /// and throwing them away at bind time is what used to force the npc to recover a pawn's species
  /// by string-parsing its texture path. [`Bundle::definition_reference`] hands the whole thing
  /// back, which is what lets that parsing (and the code-owned species palette behind it) die.
  registry: Option<std::collections::HashMap<(bool, String), u32>>,
  /// The GAMEPLAY registry override, keyed `(category, name)` (interactions F1) — the
  /// same posture as `registry`: `None` resolves from the corpus-position SEED, which is
  /// exactly what a fresh registry allocates, so the two agree by construction.
  gameplay_registry: Option<std::collections::HashMap<(String, String), u32>>,
}

impl Bundle {
  /// Supply the registry's `name → kind_id` resolution, keyed `(is_tile, name)`.
  ///
  /// A name the registry does not carry falls back to the corpus's authored id, so a partially
  /// seeded registry degrades to today's behaviour rather than to nothing.
  pub fn with_registry(mut self, map: std::collections::HashMap<(bool, String), u32>) -> Self {
    self.registry = Some(map);
    self
  }

  /// Whether a registry has been injected — for a consumer that wants to log which authority it
  /// is resolving through.
  pub fn has_registry(&self) -> bool {
    self.registry.is_some()
  }

  /// The full `definition_reference` the registry carries for a name, if any. This is the
  /// TAXONOMY's number — type, subtype, kind and variant all — so a caller that needs a pawn's
  /// species reads it off the id rather than off an art path.
  pub fn definition_reference(&self, is_tile: bool, name: &str) -> Option<u32> {
    self.registry.as_ref()?.get(&(is_tile, name.to_string())).copied()
  }

  /// The KIND half of the registry's answer — what the `*_def_id` accessors serve.
  fn registry_id(&self, is_tile: bool, name: &str) -> Option<u16> {
    // `kind_reference = kind_id:12 | variant_id:4`; the kind half is the high 12 of the low 16.
    self.definition_reference(is_tile, name).map(|d| ((d >> 4) & 0xFFF) as u16)
  }

  /// Supply the GAMEPLAY registry's resolution, keyed `(category, name)` — built by consumers
  /// from `/definitions` rows with `type == "gameplay"` (`sub_type` = category, `kind` = name).
  pub fn with_gameplay_registry(
    mut self,
    map: std::collections::HashMap<(String, String), u32>,
  ) -> Self {
    self.gameplay_registry = Some(map);
    self
  }

  /// The category registry a gameplay category name indexes, or `None` for an unknown one.
  fn gameplay_names(&self, category: &str) -> Option<&[String]> {
    match category {
      "need" => Some(&self.need_names),
      "condition" => Some(&self.condition_names),
      "trait" => Some(&self.trait_names),
      "interaction" => Some(&self.interaction_names),
      "affordance" => Some(&self.affordance_names),
      "stat" => Some(&self.stat_names),
      _ => None,
    }
  }

  /// A gameplay def's u32 `definition_reference` — registry-first, else the corpus-position
  /// SEED (`gameplay | category | position | default`), which is exactly what a fresh
  /// registry allocates from, so the two agree by construction (interactions F1).
  pub fn gameplay_reference(&self, category: &str, name: &str) -> Option<u32> {
    if let Some(map) = &self.gameplay_registry {
      if let Some(id) = map.get(&(category.to_string(), name.to_string())) {
        return Some(*id);
      }
    }
    let subtype = resonantdust_codec::object::gameplay_subtype_id(category)?;
    let kind = Self::id_of(self.gameplay_names(category)?, name)?;
    Some(resonantdust_codec::object::pack_definition_from_ids(
      resonantdust_codec::object::TYPE_GAMEPLAY,
      subtype,
      kind,
      0,
    ))
  }

  /// The reverse: what a gameplay `definition_reference` MEANS — `(category, name)`.
  /// Registry-first (a scan; the map is tiny), else the seed unpack. `None` for a
  /// non-gameplay reference or an unknown one.
  pub fn gameplay_lookup(&self, reference: u32) -> Option<(String, String)> {
    if let Some(map) = &self.gameplay_registry {
      if let Some(((category, name), _)) = map.iter().find(|(_, id)| **id == reference) {
        return Some((category.clone(), name.clone()));
      }
    }
    use resonantdust_codec::object as obj;
    if obj::def_type_id(reference) != obj::TYPE_GAMEPLAY {
      return None;
    }
    let category = obj::gameplay_category(obj::def_subtype_id(reference))?;
    let name = Self::name_of(self.gameplay_names(category)?, obj::def_kind_id(reference))?;
    Some((category.to_string(), name.to_string()))
  }

  /// A need's params by gameplay `definition_reference` — the eval's resolution path.
  pub fn need_params_by_ref(&self, reference: u32) -> Option<NeedParams> {
    let (category, name) = self.gameplay_lookup(reference)?;
    (category == "need").then(|| self.need_params(&name)).flatten()
  }

  /// A condition's params by gameplay `definition_reference`.
  pub fn condition_params_by_ref(&self, reference: u32) -> Option<ConditionParams> {
    let (category, name) = self.gameplay_lookup(reference)?;
    (category == "condition").then(|| self.condition_params(&name)).flatten()
  }

  /// Finalize the name caches after the def vecs are filled (both loaders call this).
  pub(crate) fn index(mut self) -> Self {
    self.tile_names = self.tiles.iter().map(|d| d.name.clone()).collect();
    self.thing_names = self.things.iter().map(|d| d.name.clone()).collect();
    self.biome_names = self.biomes.iter().map(|d| d.name.clone()).collect();
    self.material_names = self.materials.iter().map(|(n, _)| n.clone()).collect();
    self.need_names = self.needs.iter().map(|(n, _)| n.clone()).collect();
    self.condition_names = self.conditions.iter().map(|(n, _)| n.clone()).collect();
    self.trait_names = self.traits.iter().map(|(n, _)| n.clone()).collect();
    self.interaction_names = self.interactions.iter().map(|(n, _)| n.clone()).collect();
    self.affordance_names = self.affordances.iter().map(|(n, _)| n.clone()).collect();
    self.stat_names = self.stats.iter().map(|(n, _)| n.clone()).collect();
    self
  }

  fn id_of(names: &[String], name: &str) -> Option<u16> {
    names.iter().position(|n| n == name && !n.is_empty()).map(|i| i as u16 + 1)
  }
  fn name_of(names: &[String], id: u16) -> Option<&str> {
    (id != 0)
      .then(|| names.get(id as usize - 1))
      .flatten()
      .filter(|n| !n.is_empty())
      .map(String::as_str)
  }

  // ---------- tiles ----------

  /// Every tile name in `def_id` order (index 0 → def_id 1; `""` = a retired id).
  pub fn tile_names(&self) -> &[String] {
    &self.tile_names
  }
  /// The `def_id` for a tile name (1-based; `None` if unknown). This is the u12
  /// packed into a zone's tile slot (`resonantdust_codec::packed::pack_tile`).
  pub fn tile_def_id(&self, name: &str) -> Option<u16> {
    self.registry_id(true, name).or_else(|| Self::id_of(&self.tile_names, name))
  }
  /// The `subtype_id` for a `(type, name)` pair authored in `subtypes.toml`, or `None`.
  ///
  /// This is where a pawn SPECIES gets its number now that the code-owned palette is gone
  /// (F16): `subtype_id_of("pawn", "animal")` is `1`, exactly the value the palette held, so every
  /// stored pawn def keeps reading the same.
  pub fn subtype_id_of(&self, type_name: &str, name: &str) -> Option<u16> {
    self.subtypes.iter().find(|(t, n, _)| t == type_name && n == name).map(|(_, _, id)| *id)
  }

  /// A FNV-1a fingerprint of the fields the **simulation** reads for a tile — its VERSION input
  /// (definition-registry [F12](../../../docs/work/2026-08-04-definition-registry/forks.md#f12)).
  ///
  /// Deliberately excludes `color`, `visual` and the lighting lanes: art, tint and comments do not
  /// bump a version, because the invariant the versioning buys is **"same id ⇒ same behaviour"**,
  /// and behaviour is what the simulation reads. A re-master already propagates through the
  /// texture manifest's own content hash without touching identity — bumping there would mint an
  /// id per art tweak and burn kind space for nothing.
  ///
  /// For a tile that is `height` (walls occlude and block) and `build` (what the panel places).
  pub fn tile_sim_version(&self, def_id: u16) -> Option<u64> {
    let d = self.tiles.get(def_id.checked_sub(1)? as usize)?;
    let mut h = FNV_OFFSET;
    fnv_str(&mut h, &d.name);
    fnv_str(&mut h, d.build.as_deref().unwrap_or(""));
    fnv_bytes(&mut h, &d.height.unwrap_or(0.0).to_bits().to_le_bytes());
    for b in &d.interactions {
      fnv_str(&mut h, &b.name);
      fnv_bytes(&mut h, &b.magnitude.to_le_bytes());
      // The yield is SIM-visible (a fell composes a different SET) — it bumps.
      fnv_str(&mut h, b.yields.as_deref().unwrap_or(""));
    }
    Some(h)
  }

  /// The same for a thing: its `needs`, traits and interactions (what the simulation
  /// reads). Not its art. (`speed` left the schema — input-rework F8.)
  pub fn thing_sim_version(&self, object_id: u16) -> Option<u64> {
    let d = self.things.get(object_id.checked_sub(1)? as usize)?;
    let mut h = FNV_OFFSET;
    fnv_str(&mut h, &d.name);
    for n in &d.needs {
      fnv_str(&mut h, n);
    }
    for (t, level) in &d.traits {
      fnv_str(&mut h, t);
      fnv_bytes(&mut h, &level.to_le_bytes());
    }
    for b in &d.interactions {
      fnv_str(&mut h, &b.name);
      fnv_bytes(&mut h, &b.magnitude.to_le_bytes());
      fnv_str(&mut h, b.yields.as_deref().unwrap_or(""));
    }
    Some(h)
  }

  /// A tile's authored VERSION ([F17]) — which revision this def is. `0` when unauthored.
  pub fn tile_version(&self, def_id: u16) -> Option<u32> {
    Some(self.tiles.get(def_id.checked_sub(1)? as usize)?.version)
  }
  /// A thing's authored VERSION ([F17]).
  pub fn thing_version(&self, object_id: u16) -> Option<u32> {
    Some(self.things.get(object_id.checked_sub(1)? as usize)?.version)
  }

  /// A tile's authored TAXONOMY by `def_id`, or `None` if it has none yet ([I9]).
  pub fn tile_taxonomy(&self, def_id: u16) -> Option<&Taxonomy> {
    self.tiles.get(def_id.checked_sub(1)? as usize)?.taxonomy.as_ref()
  }
  /// A thing's authored TAXONOMY by `object_id`, or `None` if it has none yet ([I9]).
  pub fn thing_taxonomy(&self, object_id: u16) -> Option<&Taxonomy> {
    self.things.get(object_id.checked_sub(1)? as usize)?.taxonomy.as_ref()
  }

  /// The tile name for a `def_id` (`def_id == 0` is the empty sentinel).
  pub fn tile_name(&self, def_id: u16) -> Option<&str> {
    Self::name_of(&self.tile_names, def_id)
  }
  fn tile_def(&self, name: &str) -> Option<&TileDef> {
    self.tiles.iter().find(|d| d.name == name && !name.is_empty())
  }

  /// A tile's background colour as a packed `0xRRGGBB` — its visual prim's tint
  /// (fallback: a static `color.bg` from the older authored shape).
  pub fn tile_color_bg(&self, name: &str) -> Option<u32> {
    self.tile_def(name)?.color
  }
  /// [`Self::tile_color_bg`] keyed by `def_id`.
  pub fn color_bg_for_def(&self, def_id: u16) -> Option<u32> {
    self.tiles.get(def_id.checked_sub(1)? as usize)?.color
  }
  /// The [`VisualParts`] for a tile `def_id`.
  pub fn visual_for_def(&self, def_id: u16) -> Option<VisualParts> {
    self.tiles.get(def_id.checked_sub(1)? as usize)?.visual.clone()
  }
  /// Every tile's texture stem in `def_id` order; empty string = no texture.
  pub fn tile_texture_stems(&self) -> Vec<String> {
    self
      .tiles
      .iter()
      .map(|d| d.visual.as_ref().and_then(|v| v.texture.clone()).unwrap_or_default())
      .collect()
  }
  /// A tile's build category (`None` = not buildable).
  pub fn tile_build(&self, def_id: u16) -> Option<String> {
    self.tiles.get(def_id.checked_sub(1)? as usize)?.build.clone()
  }
  /// Every tile's build category in `def_id` order; `""` = not buildable
  /// (build-walls D3 — the build menu's scan table).
  pub fn tile_builds(&self) -> Vec<String> {
    self.tiles.iter().map(|d| d.build.clone().unwrap_or_default()).collect()
  }
  /// A tile's LIGHTING + LINKED lanes (texture-generalization P0), flat stride-6 per
  /// def_id: `[linked_w, linked_h, padding, rotation, cast_shadow, receives_shadows]` —
  /// zeros where unauthored.
  pub fn tile_lighting_lanes(&self) -> Vec<f64> {
    let mut out = Vec::with_capacity(self.tiles.len() * 6);
    for d in &self.tiles {
      out.extend_from_slice(&d.lanes);
    }
    out
  }
  /// A tile's HEIGHT in tiles (tile-lighting F2: > 0 opts into the cold lighting
  /// class). `None` = flat ground.
  pub fn tile_height(&self, def_id: u16) -> Option<f64> {
    self.tiles.get(def_id.checked_sub(1)? as usize)?.height
  }
  /// Every tile's height in `def_id` order (0 = flat/unauthored).
  pub fn tile_heights(&self) -> Vec<f64> {
    self.tiles.iter().map(|d| d.height.unwrap_or(0.0)).collect()
  }
  /// May a pawn ENTER this tile kind? (pathfinding F1). An unknown or RETIRED id reads
  /// PATHABLE — a corpus/state version skew must degrade to open ground, never freeze a
  /// pawn against an invisible wall.
  pub fn tile_pathable(&self, def_id: u16) -> bool {
    match def_id.checked_sub(1).and_then(|i| self.tiles.get(i as usize)) {
      Some(d) if !d.name.is_empty() => d.pathable,
      _ => true,
    }
  }
  /// Every tile's 4 packed-channel material bindings in `def_id` order.
  pub fn tile_packed_channels(&self) -> Vec<[PackedChannel; 4]> {
    self.tiles.iter().map(|d| d.visual.as_ref().map(|v| v.packed).unwrap_or_default()).collect()
  }

  // ---------- things ----------

  /// Every thing name in `object_id` order (index 0 → object_id 1; `""` = retired).
  pub fn thing_names(&self) -> &[String] {
    &self.thing_names
  }
  /// The `object_id` for a thing name (1-based; `None` if unknown) — the u12 packed
  /// into a zone's thing entry.
  pub fn thing_object_id(&self, name: &str) -> Option<u16> {
    self.registry_id(false, name).or_else(|| Self::id_of(&self.thing_names, name))
  }
  /// The thing name for an `object_id` (`0` is the empty sentinel).
  pub fn thing_name(&self, object_id: u16) -> Option<&str> {
    Self::name_of(&self.thing_names, object_id)
  }
  fn thing_def(&self, name: &str) -> Option<&ThingDef> {
    self.things.iter().find(|d| d.name == name && !name.is_empty())
  }

  /// A thing's colour as a packed `0xRRGGBB` — its visual prim tint.
  pub fn thing_color_bg(&self, name: &str) -> Option<u32> {
    self.thing_def(name)?.color
  }
  /// [`Self::thing_color_bg`] keyed by `object_id`.
  pub fn thing_color_for_object(&self, object_id: u16) -> Option<u32> {
    self.things.get(object_id.checked_sub(1)? as usize)?.color
  }
  /// The [`VisualParts`] for a thing `object_id` — cold things and pawns alike.
  pub fn visual_for_object(&self, object_id: u16) -> Option<VisualParts> {
    self.things.get(object_id.checked_sub(1)? as usize)?.visual.clone()
  }
  /// Every thing's texture stem in `object_id` order; `""` for a def with none.
  pub fn thing_texture_stems(&self) -> Vec<String> {
    self
      .things
      .iter()
      .map(|d| d.visual.as_ref().and_then(|v| v.texture.clone()).unwrap_or_default())
      .collect()
  }

  /// Every thing's spatial LAYOUT in `object_id` order, flattened **stride-10** per def:
  /// `[footprint.w, footprint.h, anchor.x, anchor.y, size, sprite_anchor.x,
  /// sprite_anchor.y, span, sprite_scale.w, sprite_scale.h]`. A def with no prim gets
  /// the default row `[1, 1, 0.5, 0.5, 1, 0.5, 0.5, 1, 1, 1]`, so the host never
  /// special-cases "unset".
  pub fn thing_layout(&self) -> Vec<f64> {
    let mut out = Vec::with_capacity(self.things.len() * 10);
    for d in &self.things {
      match &d.visual {
        Some(v) => out.extend_from_slice(&[
          v.footprint.0, v.footprint.1, v.anchor.0, v.anchor.1, v.size, v.sprite_anchor.0, v.sprite_anchor.1,
          v.span, v.sprite_scale.0, v.sprite_scale.1,
        ]),
        None => out.extend_from_slice(&[1.0, 1.0, 0.5, 0.5, 1.0, 0.5, 0.5, 1.0, 1.0, 1.0]),
      }
    }
    out
  }

  /// Every thing's SUBFRAMES in `object_id` order, flattened **stride-1536** per def:
  /// [`VARIANTS_PER_DEF`]` × `[`ROTATIONS_PER_DEF`]` × [sub.x, sub.y, sub.w, sub.h,
  /// anchor.x, anchor.y]`, all fractions. This is what the atlas CROPS to
  /// (subframe-ingest): one rect, four maps, registered by construction.
  pub fn thing_subframe(&self) -> Vec<f64> {
    let mut out = Vec::with_capacity(self.things.len() * VARIANTS_PER_DEF * ROTATIONS_PER_DEF * 6);
    for d in &self.things {
      let frames = d.visual.as_ref().map(|v| v.dir_frames).unwrap_or(
        [[DirFrame { sub: (0.0, 0.0, 1.0, 1.0), anchor: (0.5, 0.5) }; ROTATIONS_PER_DEF]; VARIANTS_PER_DEF],
      );
      for by_rot in frames {
        for f in by_rot {
          out.extend_from_slice(&[f.sub.0, f.sub.1, f.sub.2, f.sub.3, f.anchor.0, f.anchor.1]);
        }
      }
    }
    out
  }

  /// Every thing's emitted LIGHT in `object_id` order, flattened **stride-8** per def:
  /// `[r, g, b, intensity, reach, radius, height, flags]` (`flags` bit 0 = cast,
  /// bit 1 = hot, bit 2 = flicker). **`reach == 0` IS the "no light" test.**
  pub fn thing_light(&self) -> Vec<f64> {
    let mut out = Vec::with_capacity(self.things.len() * 8);
    for d in &self.things {
      match d.visual.as_ref().and_then(|v| v.light) {
        Some(l) => out.extend_from_slice(&[
          l.color.0, l.color.1, l.color.2, l.intensity, l.reach, l.radius, l.height,
          f64::from(u8::from(l.cast) | (u8::from(l.hot) << 1) | (u8::from(l.flicker) << 2)),
        ]),
        None => out.extend_from_slice(&[0.0; 8]),
      }
    }
    out
  }

  /// Every thing's 4 packed-channel material bindings in `object_id` order.
  pub fn thing_packed_channels(&self) -> Vec<[PackedChannel; 4]> {
    self.things.iter().map(|d| d.visual.as_ref().map(|v| v.packed).unwrap_or_default()).collect()
  }

  /// The needs a thing kind carries, as u32 gameplay `definition_reference`s
  /// (interactions F1; needs-moodlets P1). A name the resolution cannot number is
  /// dropped — load validation makes that unreachable for a well-formed corpus.
  pub fn thing_needs(&self, object_id: u16) -> Vec<u32> {
    self
      .things
      .get(object_id.checked_sub(1).map(usize::from).unwrap_or(usize::MAX))
      .map(|d| d.needs.iter().filter_map(|n| self.gameplay_reference("need", n)).collect())
      .unwrap_or_default()
  }
  /// May a pawn ENTER a cell this thing kind occupies? (pathfinding F1). Unknown or
  /// RETIRED ids read PATHABLE — version skew degrades to open ground (the same guard
  /// as [`Self::tile_pathable`]).
  pub fn thing_pathable(&self, object_id: u16) -> bool {
    match object_id.checked_sub(1).and_then(|i| self.things.get(i as usize)) {
      Some(d) if !d.name.is_empty() => d.pathable,
      _ => true,
    }
  }
  /// Every thing's needs in `object_id` order, flattened **stride-[`NEEDS_PER_KIND`]**
  /// per kind (`0` = empty slot). Values are u32 gameplay refs (exact in an f64).
  pub fn thing_needs_table(&self) -> Vec<f64> {
    let mut out = Vec::with_capacity(self.things.len() * NEEDS_PER_KIND);
    for d in &self.things {
      for i in 0..NEEDS_PER_KIND {
        let r = d.needs.get(i).and_then(|n| self.gameplay_reference("need", n)).unwrap_or(0);
        out.push(f64::from(r));
      }
    }
    out
  }

  /// The STARTING trait bindings a thing kind authors — `(trait name, level ≥ 1)`
  /// (stat-model F11): what CREATE mints as the pawn's trait rows. Runtime truth is the
  /// pawn's payload rows, not this.
  pub fn thing_traits(&self, object_id: u16) -> Vec<(String, u16)> {
    self
      .things
      .get(object_id.checked_sub(1).map(usize::from).unwrap_or(usize::MAX))
      .map(|d| d.traits.clone())
      .unwrap_or_default()
  }
  /// A thing kind's interaction bindings — `(interaction name, magnitude)`
  /// (stat-model F5/F9): what this carrier OFFERS.
  pub fn thing_interactions(&self, object_id: u16) -> Vec<InteractionBind> {
    self
      .things
      .get(object_id.checked_sub(1).map(usize::from).unwrap_or(usize::MAX))
      .map(|d| d.interactions.clone())
      .unwrap_or_default()
  }
  /// A tile def's interaction bindings. The water tile's `drink 3` lives here.
  pub fn tile_interactions(&self, def_id: u16) -> Vec<InteractionBind> {
    self
      .tiles
      .get(def_id.checked_sub(1).map(usize::from).unwrap_or(usize::MAX))
      .map(|d| d.interactions.clone())
      .unwrap_or_default()
  }

  // ---------- biomes ----------

  /// Every biome name in evaluation order (first match wins).
  pub fn biome_names(&self) -> &[String] {
    &self.biome_names
  }
  /// A biome's stable `subtype_id` — its identity in stored zones (explicit, never
  /// derived from order). `None` if unauthored or `0` (the reserved subtype).
  pub fn biome_subtype_id(&self, name: &str) -> Option<u16> {
    let d = self.biomes.iter().find(|b| b.name == name)?;
    (d.subtype > 0).then_some(d.subtype)
  }

  /// The first biome whose condition passes for these `dims`, in evaluation order.
  /// (`seed` kept in the signature — classification is dims-pure today, but the seed
  /// belongs to the per-tile contract worldgen calls with.)
  pub fn select_biome(&self, dims: &[f64], _seed: u64) -> Option<&str> {
    self.biomes.iter().find_map(|b| self.biome_matches(b, dims).then_some(b.name.as_str()))
  }

  fn biome_matches(&self, b: &BiomeDef, dims: &[f64]) -> bool {
    match &b.body {
      BiomeBody::Rules(r) => r
        .when
        .iter()
        .all(|&(dim, cmp, t)| cmp.pass(dims.get(dim).copied().unwrap_or(0.0), t)),
    }
  }

  /// Classify one tile end to end: pick its biome by `dims`, then read the ground
  /// `tile` + any `thing1` its body places (seeded scatter). The per-tile worldgen
  /// entry point — content decides *what*, worldgen packs it.
  pub fn generate(&self, dims: &[f64], seed: u64) -> GenTile {
    let Some(b) = self.biomes.iter().find(|b| self.biome_matches(b, dims)) else {
      return GenTile::default();
    };
    let mut gen = GenTile { biome: Some(b.name.clone()), ..GenTile::default() };
    match &b.body {
      BiomeBody::Rules(r) => {
        gen.tile = r.tile.clone();
        // Ordered draws, LAST hit wins (least→most dominant, as authored).
        for (salt, p, thing) in &r.scatter {
          if tile_rand(seed, *salt) < *p {
            gen.thing1 = Some(thing.clone());
          }
        }
      }
    }
    gen
  }

  // ---------- materials ----------

  /// Every material name in `material_id` order (index 0 → id 1).
  pub fn material_names(&self) -> &[String] {
    &self.material_names
  }
  /// The 1-based `material_id` for a name (`None` if unknown).
  pub fn material_id(&self, name: &str) -> Option<u16> {
    Self::id_of(&self.material_names, name)
  }
  /// The material name for a `material_id` (`0` is the empty sentinel).
  pub fn material_name(&self, id: u16) -> Option<&str> {
    Self::name_of(&self.material_names, id)
  }
  /// A material's [`MaterialParams`], or `None` if unknown.
  pub fn material_params(&self, name: &str) -> Option<MaterialParams> {
    self.materials.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone())
  }
  /// The whole material registry in `material_id` order.
  pub fn material_params_all(&self) -> Vec<MaterialParams> {
    self.materials.iter().map(|(_, p)| p.clone()).collect()
  }

  // ---------- needs & conditions (needs-moodlets P1) ----------

  /// Every need name in `need_id` order (index 0 → id 1).
  pub fn need_names(&self) -> &[String] {
    &self.need_names
  }
  /// The 1-based `need_id` for a name (`None` if unknown).
  pub fn need_id(&self, name: &str) -> Option<u16> {
    Self::id_of(&self.need_names, name)
  }
  /// The need name for a `need_id` (`0` is the empty sentinel).
  pub fn need_name(&self, id: u16) -> Option<&str> {
    Self::name_of(&self.need_names, id)
  }
  /// A need's [`NeedParams`], or `None` if unknown.
  pub fn need_params(&self, name: &str) -> Option<NeedParams> {
    self.needs.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone())
  }
  /// The whole need registry in `need_id` order.
  pub fn need_params_all(&self) -> Vec<NeedParams> {
    self.needs.iter().map(|(_, p)| p.clone()).collect()
  }

  /// Every condition name in `condition_id` order (index 0 → id 1).
  pub fn condition_names(&self) -> &[String] {
    &self.condition_names
  }
  /// The 1-based `condition_id` for a name (`None` if unknown).
  pub fn condition_id(&self, name: &str) -> Option<u16> {
    Self::id_of(&self.condition_names, name)
  }
  /// The condition name for a `condition_id` (`0` is the empty sentinel).
  pub fn condition_name(&self, id: u16) -> Option<&str> {
    Self::name_of(&self.condition_names, id)
  }
  /// A condition's [`ConditionParams`], or `None` if unknown.
  pub fn condition_params(&self, name: &str) -> Option<ConditionParams> {
    self.conditions.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone())
  }
  /// The whole condition registry in `condition_id` order.
  pub fn condition_params_all(&self) -> Vec<ConditionParams> {
    self.conditions.iter().map(|(_, p)| p.clone()).collect()
  }

  // ---------- emotions (emotions F1) ----------

  /// An emotion's u4 declaration index, or `None` if unknown.
  pub fn emotion_index(&self, name: &str) -> Option<u8> {
    self.emotions.iter().position(|(n, _)| n == name).map(|i| i as u8)
  }
  /// The whole emotion table in declaration (u4) order — `(name, params)`.
  pub fn emotion_params_all(&self) -> &[(String, EmotionParams)] {
    &self.emotions
  }

  // ---------- traits, interactions, affordances, stats (interactions P1, stat-model) ----------

  /// Every trait name, corpus order.
  pub fn trait_names(&self) -> &[String] {
    &self.trait_names
  }
  /// A trait's [`TraitParams`], or `None` if unknown.
  pub fn trait_params(&self, name: &str) -> Option<TraitParams> {
    self.traits.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone())
  }
  /// A trait's params by gameplay `definition_reference` — the stat eval's resolution of
  /// a pawn's packed trait rows.
  pub fn trait_params_by_ref(&self, reference: u32) -> Option<TraitParams> {
    let (category, name) = self.gameplay_lookup(reference)?;
    (category == "trait").then(|| self.trait_params(&name)).flatten()
  }

  /// Every stat name, corpus order.
  pub fn stat_names(&self) -> &[String] {
    &self.stat_names
  }
  /// A stat's [`StatParams`], or `None` if unknown.
  pub fn stat_params(&self, name: &str) -> Option<StatParams> {
    self.stats.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone())
  }

  /// Every interaction name, corpus order.
  pub fn interaction_names(&self) -> &[String] {
    &self.interaction_names
  }
  /// An interaction's [`InteractionParams`], or `None` if unknown.
  pub fn interaction_params(&self, name: &str) -> Option<InteractionParams> {
    self.interactions.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone())
  }
  /// An interaction's params by gameplay `definition_reference` — the worker's resolution
  /// of an `EXECUTE_INTERACTION` event's first operand.
  pub fn interaction_params_by_ref(&self, reference: u32) -> Option<InteractionParams> {
    let (category, name) = self.gameplay_lookup(reference)?;
    (category == "interaction").then(|| self.interaction_params(&name)).flatten()
  }

  /// Every affordance name, corpus order.
  pub fn affordance_names(&self) -> &[String] {
    &self.affordance_names
  }
  /// An affordance's [`AffordanceParams`], or `None` if unknown.
  ///
  /// The availability GATE itself lives in [`crate::stat_eval::affordance_passes`]
  /// (stat-model F5/F8): a predicate over DERIVED stats, computed from the pawn's
  /// trait/condition rows — the old trait-set `affordance_available` is deleted with the
  /// `requires` field it read (I11).
  pub fn affordance_params(&self, name: &str) -> Option<AffordanceParams> {
    self.affordances.iter().find(|(n, _)| n == name).map(|(_, p)| p.clone())
  }
}

/// Parse every `(name, source)` — all `.toml` — into a [`Bundle`]. A non-TOML source
/// name is a load error (the `.rd` dialect is deleted; git holds its history).
pub fn load(sources: &[(String, String)]) -> Result<Bundle, Vec<LoadError>> {
  if let Some((name, _)) = sources.iter().find(|(n, _)| !n.ends_with(".toml")) {
    return Err(vec![LoadError {
      file: name.clone(),
      message: "not a .toml source — the corpus is TOML (the .rd DSL was deleted 2026-08-04)".into(),
    }]);
  }
  crate::toml_loader::load_toml(sources)
}

/// The kind's emitted light — see [`VisualParts::light`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightParts {
  pub color: (f64, f64, f64),
  pub intensity: f64,
  pub reach: f64,
  pub radius: f64,
  pub height: f64,
  /// Casts shadows (default true). A fill light that lights without occluding costs
  /// the gather nothing — it is skipped in the shadow walk entirely.
  pub cast: bool,
  /// Animates per frame ⇒ the HOT class, re-baked every frame. Default false.
  pub hot: bool,
  /// Emits DECAY-LIGHTMAP flicker particles (lighting-feel P2). Orthogonal to `hot`.
  pub flicker: bool,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn non_toml_sources_refuse_to_load() {
    let e = load(&[("data/tiles.rd".to_string(), "<tile>".to_string())]).unwrap_err();
    assert!(e[0].message.contains("TOML"), "{}", e[0].message);
    assert_eq!(e[0].file, "data/tiles.rd");
  }

  #[test]
  fn tile_rand_is_the_documented_derivation() {
    // Pinned: SplitMix64 over seed ^ salt*phi. Distinct salts decorrelate; same
    // inputs reproduce (the worldgen sweep in tests/golden.rs leans on this).
    assert_eq!(tile_rand(42, 6), tile_rand(42, 6));
    assert_ne!(tile_rand(42, 6), tile_rand(42, 7));
    assert_ne!(tile_rand(42, 6), tile_rand(43, 6));
    let r = tile_rand(123456789, 9);
    assert!((0.0..1.0).contains(&r));
  }
}
