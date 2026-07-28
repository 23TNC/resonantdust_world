//! `Uplink<C, S>` — a self-healing upstream SpacetimeDB connection (sim-self-heal P1).
//!
//! Generalises the gateway's `directory.rs` pattern: the SDK does NOT auto-reconnect, so every
//! consumer holds a connection alongside an `alive` flag the SDK's callbacks clear, and
//! rebuilds a dead connection on next use. This crate is that pattern once, for everyone:
//!
//! - **Closure-based** (F3): the caller supplies `build` (returns `(Arc<C>, Arc<AtomicBool>)`
//!   with `alive` already wired into `on_disconnect` + `on_connect_error` + every
//!   subscription's `on_error` — the third is the one the gateway pattern missed) and an
//!   optional async `subscribe` closure (returns the subscription handle(s), held so dropping
//!   doesn't unsubscribe). `C` is the module's generated `DbConnection`; the helper never
//!   names SDK types, which is also what makes it unit-testable with `C = ()`.
//! - **`get()` is the whole API**: returns the live connection, or rebuilds it — gated on the
//!   subscribe closure completing, so a returned connection NEVER has an un-applied cache.
//!   Startup and mid-run recovery are the same code path (F2): build best-effort, work with
//!   what's alive, skip what isn't.
//! - **Capped backoff**: failed rebuilds space out 0.5 s → 8 s (doubling), so a down shard is
//!   retried politely instead of hot-looped; while backing off, `get()` fails fast.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

/// The backoff schedule: first retry delay, doubling to the cap.
const BACKOFF_START: Duration = Duration::from_millis(500);
const BACKOFF_CAP: Duration = Duration::from_secs(8);

type BuildFn<C> = dyn Fn() -> Result<(Arc<C>, Arc<AtomicBool>), String> + Send + Sync;
type SubscribeFuture<S> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<S, String>> + Send>>;
type SubscribeFn<C, S> = dyn Fn(Arc<C>, Arc<AtomicBool>) -> SubscribeFuture<S> + Send + Sync;

/// A live connection: the conn, its liveness flag, and the held subscription handle(s).
struct Live<C, S> {
    conn: Arc<C>,
    alive: Arc<AtomicBool>,
    /// Held so the subscription isn't torn down (dropping a handle unsubscribes).
    _sub: Option<S>,
}

/// Rebuild pacing: when the next attempt is allowed and how many have failed in a row.
struct Backoff {
    attempts: u32,
    next_attempt_at: Instant,
}

/// A self-healing upstream connection. `C` = the module's `DbConnection`; `S` = whatever the
/// subscribe closure returns (a handle, a tuple of handles, `()` for sub-less consumers).
pub struct Uplink<C, S = ()> {
    /// Shown in logs — the upstream's name (e.g. `"pawn"`, `"index"`).
    name: String,
    build: Box<BuildFn<C>>,
    subscribe: Option<Box<SubscribeFn<C, S>>>,
    /// Async mutex: the rebuild path awaits the subscribe closure, and concurrent `get()`s
    /// must share one rebuild rather than race.
    inner: tokio::sync::Mutex<(Option<Live<C, S>>, Backoff)>,
    /// Bumped on every successful rebuild — lets a caller detect a reconnect and re-run
    /// once-per-connection standup work (e.g. the master re-stamping `set_orchestrator` on a
    /// republished event shard).
    generation: std::sync::atomic::AtomicU64,
}

impl<C, S> Uplink<C, S> {
    /// A sub-less uplink (a pure call surface — e.g. the master's shard `bump` conns).
    pub fn new(
        name: impl Into<String>,
        build: impl Fn() -> Result<(Arc<C>, Arc<AtomicBool>), String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            build: Box::new(build),
            subscribe: None,
            inner: tokio::sync::Mutex::new((None, Backoff { attempts: 0, next_attempt_at: Instant::now() })),
            generation: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// An uplink whose cache must be applied before use: `get()` returns only after
    /// `subscribe` completes (subscribe-and-wait; the closure owns its own timeout).
    pub fn with_subscription(
        name: impl Into<String>,
        build: impl Fn() -> Result<(Arc<C>, Arc<AtomicBool>), String> + Send + Sync + 'static,
        subscribe: impl Fn(Arc<C>, Arc<AtomicBool>) -> SubscribeFuture<S> + Send + Sync + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            build: Box::new(build),
            subscribe: Some(Box::new(subscribe)),
            inner: tokio::sync::Mutex::new((None, Backoff { attempts: 0, next_attempt_at: Instant::now() })),
            generation: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// The live connection — rebuilt (build → subscribe → store) if absent or dead. Fails fast
    /// while the backoff window is open. NEVER panics: a dead upstream is a skipped pass, not
    /// a dead process.
    pub async fn get(&self) -> Result<Arc<C>, String> {
        let mut guard = self.inner.lock().await;
        let (live, backoff) = &mut *guard;

        if let Some(l) = live.as_ref() {
            if l.alive.load(Ordering::Acquire) {
                return Ok(l.conn.clone());
            }
            tracing::warn!(uplink = %self.name, "connection dead; will rebuild");
            *live = None;
        }

        let now = Instant::now();
        if now < backoff.next_attempt_at {
            return Err(format!(
                "{}: backing off ({} failed attempt(s), retry in {:?})",
                self.name,
                backoff.attempts,
                backoff.next_attempt_at - now
            ));
        }

        match self.rebuild().await {
            Ok(l) => {
                let conn = l.conn.clone();
                if backoff.attempts > 0 {
                    tracing::info!(uplink = %self.name, after_attempts = backoff.attempts, "reconnected");
                }
                *live = Some(l);
                backoff.attempts = 0;
                backoff.next_attempt_at = now;
                self.generation.fetch_add(1, Ordering::AcqRel);
                Ok(conn)
            }
            Err(err) => {
                backoff.attempts += 1;
                let delay = BACKOFF_START
                    .saturating_mul(1u32 << (backoff.attempts - 1).min(4))
                    .min(BACKOFF_CAP);
                backoff.next_attempt_at = now + delay;
                tracing::warn!(uplink = %self.name, attempt = backoff.attempts, ?delay, %err, "rebuild failed");
                Err(format!("{}: {err}", self.name))
            }
        }
    }

    /// The current connection generation — bumped on every successful rebuild. Compare across
    /// `get()` calls to detect a reconnect (and re-run per-connection standup work).
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    async fn rebuild(&self) -> Result<Live<C, S>, String> {
        let (conn, alive) = (self.build)()?;
        let sub = match &self.subscribe {
            Some(subscribe) => Some(subscribe(conn.clone(), alive.clone()).await?),
            None => None,
        };
        // A connection that died DURING subscribe must not be handed out as live.
        if !alive.load(Ordering::Acquire) {
            return Err("connection died during subscribe".into());
        }
        Ok(Live { conn, alive, _sub: sub })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;
    use tokio::time::{advance, pause};

    /// A fake upstream: counts build calls, fails while `fail` is set, and hands out the
    /// shared `alive` flag (the test flips it to simulate SDK disconnect callbacks).
    struct Fake {
        builds: Arc<AtomicU32>,
        fail: Arc<AtomicBool>,
        alive: Arc<AtomicBool>,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                builds: Arc::new(AtomicU32::new(0)),
                fail: Arc::new(AtomicBool::new(false)),
                alive: Arc::new(AtomicBool::new(true)),
            }
        }

        fn uplink(&self) -> Uplink<u32> {
            let builds = self.builds.clone();
            let fail = self.fail.clone();
            let alive = self.alive.clone();
            Uplink::new("fake", move || {
                builds.fetch_add(1, Ordering::SeqCst);
                if fail.load(Ordering::SeqCst) {
                    return Err("upstream down".into());
                }
                alive.store(true, Ordering::SeqCst);
                Ok((Arc::new(7u32), alive.clone()))
            })
        }
    }

    #[tokio::test]
    async fn get_errs_while_down_and_recovers_when_up() {
        let fake = Fake::new();
        fake.fail.store(true, Ordering::SeqCst);
        let up = fake.uplink();
        assert!(up.get().await.is_err(), "down upstream must err, not panic");
        fake.fail.store(false, Ordering::SeqCst);
        // Immediately after a failure the backoff window is open — wait it out.
        tokio::time::sleep(BACKOFF_START).await;
        assert_eq!(*up.get().await.expect("recovers once the build succeeds"), 7);
    }

    #[tokio::test]
    async fn a_cleared_alive_flag_triggers_exactly_one_rebuild() {
        let fake = Fake::new();
        let up = fake.uplink();
        up.get().await.unwrap();
        up.get().await.unwrap();
        assert_eq!(fake.builds.load(Ordering::SeqCst), 1, "a live conn is reused");
        // Simulate the SDK clearing the flag (disconnect / connect-error / sub on_error).
        fake.alive.store(false, Ordering::SeqCst);
        up.get().await.unwrap();
        assert_eq!(fake.builds.load(Ordering::SeqCst), 2, "dead conn rebuilt on next use");
    }

    #[tokio::test]
    async fn get_resolves_only_after_the_subscribe_closure_completes() {
        let subscribed = Arc::new(AtomicBool::new(false));
        let s2 = subscribed.clone();
        let up: Uplink<u32, u8> = Uplink::with_subscription(
            "subbed",
            move || {
                let alive = Arc::new(AtomicBool::new(true));
                Ok((Arc::new(1u32), alive))
            },
            move |_conn, _alive| {
                let s = s2.clone();
                Box::pin(async move {
                    tokio::task::yield_now().await;
                    s.store(true, Ordering::SeqCst);
                    Ok(9u8)
                })
            },
        );
        up.get().await.unwrap();
        assert!(subscribed.load(Ordering::SeqCst), "get() returned before the cache applied");
    }

    #[tokio::test]
    async fn failed_rebuilds_follow_the_backoff_schedule() {
        pause(); // virtual time
        let fake = Fake::new();
        fake.fail.store(true, Ordering::SeqCst);
        let up = fake.uplink();

        // Attempt 1 runs immediately; rapid retries inside the window do NOT build.
        assert!(up.get().await.is_err());
        assert_eq!(fake.builds.load(Ordering::SeqCst), 1);
        for _ in 0..5 {
            assert!(up.get().await.is_err());
        }
        assert_eq!(fake.builds.load(Ordering::SeqCst), 1, "backoff suppressed the hot loop");

        // After the first window (0.5 s) the next attempt runs; the window then doubles.
        advance(BACKOFF_START).await;
        assert!(up.get().await.is_err());
        assert_eq!(fake.builds.load(Ordering::SeqCst), 2);
        advance(BACKOFF_START).await; // only 0.5 s of the now-1 s window — still closed
        assert!(up.get().await.is_err());
        assert_eq!(fake.builds.load(Ordering::SeqCst), 2, "window doubled to 1 s");
        advance(BACKOFF_START).await;
        assert!(up.get().await.is_err());
        assert_eq!(fake.builds.load(Ordering::SeqCst), 3);

        // The cap: after many failures the window never exceeds BACKOFF_CAP.
        for _ in 0..10 {
            advance(BACKOFF_CAP).await;
            let _ = up.get().await;
        }
        let before = fake.builds.load(Ordering::SeqCst);
        advance(BACKOFF_CAP).await;
        let _ = up.get().await;
        assert_eq!(fake.builds.load(Ordering::SeqCst), before + 1, "capped, still retrying");

        // And recovery still works from deep backoff.
        fake.fail.store(false, Ordering::SeqCst);
        advance(BACKOFF_CAP).await;
        assert_eq!(*up.get().await.expect("recovered"), 7);
    }
}
