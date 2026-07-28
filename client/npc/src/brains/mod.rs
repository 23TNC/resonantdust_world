//! The brain catalogue — each module is one behavior over the shared [`crate::Bot`] harness.
//! A new brain is a new module + a dispatch arm in `main.rs`, never a new harness.

pub mod wildlife;
pub mod wolves;
