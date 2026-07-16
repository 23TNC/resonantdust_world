//! Conflict-component grouping — pure union-find, no I/O, so it is unit-testable in isolation.
//!
//! Two events conflict when they share a **write target**: both write entity E, so their writes must
//! compose in order on one worker. (Reads don't conflict — a worker reads `< tic`, so an event
//! reading E is unaffected by another event writing E this tic.) Union is transitive: A–B share x,
//! B–C share y ⇒ {A,B,C} is one component. On a *complete* event set the partition is independent
//! of input order, which is what makes a doubled orchestrator safe.

use std::collections::HashMap;

/// One conflict-component: the events to hand a single worker, and the entities whose `(E, tic)`
/// slots to claim for it.
pub struct WorkGroup {
    /// The component's `event_reference`s. The worker composes them in ascending order.
    pub events: Vec<u32>,
    /// The distinct write-target entities across the component — the slots to `claim`.
    pub entities: Vec<u32>,
}

/// Partition `events` (each `(event_reference, write_targets)`) into conflict-components.
///
/// An event with no write targets (e.g. a bare `CREATE`, whose minted id isn't an operand) is its
/// own singleton — it shares nothing, so it never merges.
pub fn group(events: &[(u32, Vec<u32>)]) -> Vec<WorkGroup> {
    let n = events.len();
    let mut uf = UnionFind::new(n);

    // First event to touch a given entity; anyone else touching it unions in.
    let mut first_by_entity: HashMap<u32, usize> = HashMap::new();
    for (i, (_, targets)) in events.iter().enumerate() {
        for &e in targets {
            match first_by_entity.get(&e) {
                Some(&j) => uf.union(i, j),
                None => {
                    first_by_entity.insert(e, i);
                }
            }
        }
    }

    // Collect members by root. Preserve input order within a group (⇒ ascending event_reference if
    // the caller passes them so) and dedupe entities.
    let mut by_root: HashMap<usize, (Vec<u32>, Vec<u32>)> = HashMap::new();
    for (i, (event_ref, targets)) in events.iter().enumerate() {
        let root = uf.find(i);
        let entry = by_root.entry(root).or_default();
        entry.0.push(*event_ref);
        for &e in targets {
            if !entry.1.contains(&e) {
                entry.1.push(e);
            }
        }
    }

    // Stable output: order groups by their smallest event_reference.
    let mut groups: Vec<WorkGroup> = by_root
        .into_values()
        .map(|(events, entities)| WorkGroup { events, entities })
        .collect();
    groups.sort_by_key(|g| g.events.iter().copied().min().unwrap_or(0));
    groups
}

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        UnionFind { parent: (0..n).collect(), rank: vec![0; n] }
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]]; // path halving
            x = self.parent[x];
        }
        x
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra == rb {
            return;
        }
        match self.rank[ra].cmp(&self.rank[rb]) {
            std::cmp::Ordering::Less => self.parent[ra] = rb,
            std::cmp::Ordering::Greater => self.parent[rb] = ra,
            std::cmp::Ordering::Equal => {
                self.parent[rb] = ra;
                self.rank[ra] += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sort a group's members so assertions don't depend on collection order.
    fn sorted(mut g: Vec<u32>) -> Vec<u32> {
        g.sort();
        g
    }

    #[test]
    fn disjoint_events_are_separate_singletons() {
        // e1 writes A, e2 writes B — no shared target, two groups.
        let groups = group(&[(1, vec![0xA]), (2, vec![0xB])]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].events, vec![1]);
        assert_eq!(groups[1].events, vec![2]);
    }

    #[test]
    fn shared_target_merges_two_events() {
        // both write A ⇒ one component, ordered by event_reference.
        let groups = group(&[(1, vec![0xA]), (2, vec![0xA])]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].events, vec![1, 2]);
        assert_eq!(groups[0].entities, vec![0xA]);
    }

    #[test]
    fn union_is_transitive_through_a_bridge() {
        // e1{A}, e2{B}, e3{A,B} — e3 bridges, so all three are one component.
        let groups = group(&[(1, vec![0xA]), (2, vec![0xB]), (3, vec![0xA, 0xB])]);
        assert_eq!(groups.len(), 1);
        assert_eq!(sorted(groups[0].events.clone()), vec![1, 2, 3]);
        assert_eq!(sorted(groups[0].entities.clone()), vec![0xA, 0xB]);
    }

    #[test]
    fn partition_is_order_independent() {
        // The same events in a different input order yield the same partition (by event set).
        let a = group(&[(1, vec![0xA]), (2, vec![0xB]), (3, vec![0xA, 0xB])]);
        let b = group(&[(3, vec![0xB, 0xA]), (2, vec![0xB]), (1, vec![0xA])]);
        assert_eq!(a.len(), b.len());
        assert_eq!(sorted(a[0].events.clone()), sorted(b[0].events.clone()));
    }

    #[test]
    fn no_targets_is_a_singleton() {
        // A bare CREATE (no write operands) shares nothing.
        let groups = group(&[(1, vec![]), (2, vec![])]);
        assert_eq!(groups.len(), 2);
        assert!(groups[0].entities.is_empty());
    }

    #[test]
    fn two_chains_stay_separate() {
        // {1,2} on A and {3,4} on B — two independent components.
        let groups = group(&[(1, vec![0xA]), (2, vec![0xA]), (3, vec![0xB]), (4, vec![0xB])]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].events, vec![1, 2]);
        assert_eq!(groups[1].events, vec![3, 4]);
    }
}
