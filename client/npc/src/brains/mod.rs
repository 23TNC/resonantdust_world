//! The brain catalogue — each module is one behavior over the shared [`crate::Bot`] harness.
//! A new brain is a new module + a dispatch arm in `main.rs`, never a new harness. Brains
//! will eventually control GROUPS of pawns (user, human-pawns P4); the placed human fixture
//! is NOT a brain — it is minted once (a CREATE with its PART payload) and moved by the
//! player or a debug console, never by the npc.

pub mod wolves;
