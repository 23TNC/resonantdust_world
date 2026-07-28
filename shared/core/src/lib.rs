//! resonantdust-core — the foundational shared logic, hello-world placeholder.
//!
//! Pure Rust with no wasm dependency, so it builds and tests natively on every
//! consumer: the gateway and the spacetime modules link it directly as an rlib,
//! and the webgl client reaches the same code through the `resonantdust-shared`
//! wasm bundle. Real responsibilities (codec / dsl / state / protocol / rules …)
//! get carved into sibling crates as they appear.

/// The one shared computation, for now: a greeting. Stands in for the real
/// logic that the server runs natively and the client runs in wasm — the same
/// code on both sides, which is the whole point of the shared workspace.
pub fn greeting(name: &str) -> String {
    format!("hello world from resonantdust shared, {name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greets_by_name() {
        assert_eq!(greeting("world"), "hello world from resonantdust shared, world");
    }
}
