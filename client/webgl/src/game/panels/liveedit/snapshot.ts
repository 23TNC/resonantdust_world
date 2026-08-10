//! The live-edit SNAPSHOT — one read of the shared eval, sliced four ways
//! (`2026-08-09-live-edit` F5).
//!
//! Every tab describes the same pawn at the same tic. Four tabs fetching independently would run
//! the eval four times per poll and, worse, could show four different instants inside one panel —
//! a needs bar from one tic beside an emotions list from another. So the scene builds ONE of
//! these per refresh and the panel hands slices out.
//!
//! The tabs are pure renderers over this: nothing in `*Tab.ts` fetches, and nothing decides
//! ordering or colour. Both of those are corpus rules, and re-deriving them in TS is how a second
//! implementation starts.

import type { ConditionCard } from "../conditions/ConditionCards";

/** One active trait on the selected object. Colour is AUTHORED (F2) — an unauthored trait
 *  resolves to a neutral rather than refusing to render. */
export interface TraitRow {
  /** The u32 gameplay `definition_reference` — the identity, and the hover's fallback name. */
  reference: number;
  label: string;
  /** `0xRRGGBB`. */
  color: number;
}

/** One need, read as a WHOLE (F3/I3).
 *
 *  `value`, `min` and `max` and `rate` come from a SINGLE evaluation on purpose. The authored
 *  bounds are the need's domain, but conditions and traits narrow the EFFECTIVE clamp, and the
 *  rate is a product over those same modifiers — so assembling these from separate calls gives a
 *  panel that is individually plausible and jointly wrong (a bar reading full while the effective
 *  max is 60). */
export interface NeedRow {
  reference: number;
  label: string;
  /** Current satisfaction, in the need's own units. */
  value: number;
  /** The EFFECTIVE clamp — already narrowed by active modifiers, not the authored domain. */
  min: number;
  max: number;
  /** Signed rate of change of SATISFACTION per unit time (I4): negative = losing = red,
   *  positive = gaining = green, for every need including inverted domains. The panel colours
   *  this; it never decides what the sign means. */
  rate: number;
  /** `0xRRGGBB` — the bar fill, authored on the need (F2). */
  color: number;
}

/** One emotion with its current magnitude. */
export interface EmotionRow {
  index: number;
  label: string;
  color: number;
  value: number;
  /** The argmax winner — the pawn's ACTIVE emotion (emotions F3). */
  active: boolean;
}

/** Everything the panel shows, from one evaluation. */
export interface LiveEditSnapshot {
  /** Identity of this snapshot's CONTENT — the panel skips a re-render when it is unchanged, so
   *  a poll twice a second cannot tear down a tooltip under the user's cursor. */
  key: string;
  /** The object's TOML name, for the panel's own header. */
  name: string;
  /** The object's world position, for the preview camera to follow (F1). Absent for a
   *  non-pawn selection, which leaves the preview where it was rather than jumping. */
  worldX?: number;
  worldY?: number;
  traits: TraitRow[];
  needs: NeedRow[];
  /** Reused verbatim from the conditions surface — same rows, same order, same pie slices. The
   *  eval has already sorted them (conditions F3 / emotions F4) and this must not re-sort. */
  conditions: ConditionCard[];
  emotions: EmotionRow[];
}

/** What the panel needs from the scene. One method: the scene owns the eval, the panel owns the
 *  display, and the seam between them is a single call per refresh. */
export interface LiveEditProviders {
  snapshot(entity: number): LiveEditSnapshot | null;
}

/** Shown for a non-pawn selection, a despawned entity, or before the tic estimate anchors. The
 *  panel opens with empty tabs rather than refusing (F4) — a command that appears to do nothing
 *  reads as broken. */
export const EMPTY_SNAPSHOT: LiveEditSnapshot = {
  key: "",
  name: "",
  traits: [],
  needs: [],
  conditions: [],
  emotions: [],
};
