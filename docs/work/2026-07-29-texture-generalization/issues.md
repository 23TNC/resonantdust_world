# Issues — texture-generalization

_Defects found during execution land here. Known inputs: tile-lighting's unproven churn
suspect (an out-of-range `__torch` intensity vs the u8 [0,1] lane — clamp drills, and
consider clamping `carriedLightFor`'s store/compare for robustness); background tabs freeze
rAF and void counter drills; art-128's texture migration remains in flight._
