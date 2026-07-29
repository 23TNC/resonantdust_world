//! The build menu's content scan (build-walls P0, D3) — the DSL owns the menu. A kind is
//! buildable iff its `:data @define` hook sets the `tile.build` lane (`"wall &tile.build
//! set`); this scan groups every such tile by category, so adding a brick wall in content
//! alone lights a second icon — no client code.

import type { Content } from "../../client/wasm";

export interface BuildEntry {
  /** The tile def_id — the u32 object identifier the BUILD_WALL event carries. */
  defId: number;
  /** The kind's corpus name (`wall_smooth`). */
  name: string;
  /** The build category (`wall`) — one panel sub-tab per distinct value. */
  category: string;
  /** The kind's texture stem (the LINKED atlas the icon + world tiles draw from). */
  stem: string;
}

/** Scan the corpus for buildable tiles, grouped by category (insertion order = def order). */
export function buildMenuEntries(content: Content): Map<string, BuildEntry[]> {
  const builds = content.tileBuilds();
  const names = content.tileNames();
  const stems = content.tileTextureStems();
  const out = new Map<string, BuildEntry[]>();
  for (let i = 0; i < builds.length; i++) {
    const category = builds[i];
    if (!category) continue;
    const list = out.get(category) ?? [];
    list.push({ defId: i + 1, name: names[i] ?? `#${i + 1}`, category, stem: stems[i] ?? "" });
    out.set(category, list);
  }
  return out;
}

/** The BLUEPRINT stem for a buildable kind's stem, by convention (fork F1): the MATERIAL
 *  path segment is replaced with `blueprint` — `biome-tile/default/smooth/wall` →
 *  `biome-tile/default/blueprint/wall`. One blueprint atlas per KIND, material-agnostic. */
export function blueprintStemFor(stem: string): string {
  const parts = stem.split("/");
  if (parts.length < 2) return stem;
  parts[parts.length - 2] = "blueprint";
  return parts.join("/");
}
