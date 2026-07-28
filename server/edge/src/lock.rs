//! Poison-recovering `RwLock` access (sim-self-heal P4b). A panic while a lock is held POISONS
//! it, and a poisoned `.unwrap()` turns every later content/texture request into a panic until
//! the edge restarts — a transient fault made permanent. Our locked values are swapped/updated
//! as whole values (snapshot `Arc`s, version stamps), so a poisoned guard's data is still the
//! last consistent value: recover it and keep serving.

use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

pub trait RwRecover<T> {
    /// `read()`, recovering a poisoned guard instead of panicking.
    fn read_r(&self) -> RwLockReadGuard<'_, T>;
    /// `write()`, recovering a poisoned guard instead of panicking.
    fn write_r(&self) -> RwLockWriteGuard<'_, T>;
}

impl<T> RwRecover<T> for RwLock<T> {
    fn read_r(&self) -> RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(|e| {
            tracing::warn!("recovering a poisoned read lock (a handler panicked while holding it)");
            e.into_inner()
        })
    }
    fn write_r(&self) -> RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(|e| {
            tracing::warn!("recovering a poisoned write lock (a handler panicked while holding it)");
            e.into_inner()
        })
    }
}
