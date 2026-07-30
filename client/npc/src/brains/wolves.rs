//! wolves — the first-pawns v1 brain: spawn ONE server-minted wolf (`CREATE`, def resolved
//! from the content corpus — F5) near a home tile, adopt the minted id from the zone's state
//! fan-out (F3), then wander it with single A→B `MOVE_TO`s — the WORKER chains the hops (F2);
//! this brain only picks destinations and waits for arrivals (or a timeout) before the next.

use std::time::Duration;

use client::world::tile_to_position;
use client::Event;
use resonantdust_codec::action::{CREATE, PROMOTE};
use resonantdust_codec::object::{def_sans_variant, position_macro, TYPE_PAWN};
use resonantdust_codec::refs::entity_ref_type_id;
use resonantdust_codec::speed::DEFAULT_TICS_PER_TILE;
use resonantdust_codec::tic::TIC_HZ;

use crate::{resolve_thing, Bot, Brain, Rng};

/// Server-minted pawn ids live in the TOP half of the object space (`pawn::SPAWN_BASE`) —
/// the adoption filter, so a legacy client-minted wolf is never adopted.
const MINTED_BASE: u32 = 0x0080_0000;

pub struct Wolves {
    rng: Rng,
    /// Home tile (env `NPC_HOME`, default the dev vantage `100,50`) — anchor + wander center.
    home: (i32, i32),
    /// Wander radius, tiles (env `NPC_RADIUS`, default 6).
    radius: i32,
    /// The wolf's def (resolved from the corpus at start; `0` until then).
    def: u32,
    /// The wolf's authored tics-per-tile (corpus, pawn-movement F1 — same value the worker
    /// spaces hops with), for deadline arithmetic.
    speed: u16,
    /// The adopted minted wolf, once its first `StateObject` lands.
    wolf: Option<u32>,
    /// The current trip's destination, if one is in flight.
    dest: Option<(i32, i32)>,
    /// Give-up wall time for the current trip — a lost chain (I3 territory) re-issues.
    deadline: std::time::Instant,
    /// Where the wolf last was, authoritatively (for trip planning).
    at: (i32, i32),
    /// Adopt-first window: CREATE only if no existing minted wolf shows up by this wall time
    /// (the zone snapshot replays resting pawns on subscribe) — restarts re-use their wolf
    /// instead of minting a pack.
    spawn_after: std::time::Instant,
    created: bool,
}

impl Wolves {
    pub fn new(rng: Rng) -> Self {
        let home = std::env::var("NPC_HOME")
            .ok()
            .and_then(|s| {
                let (x, y) = s.split_once(',')?;
                Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
            })
            .unwrap_or((100, 50));
        let radius: i32 = std::env::var("NPC_RADIUS").ok().and_then(|s| s.parse().ok()).unwrap_or(6);
        Self {
            rng,
            home,
            radius,
            def: 0,
            speed: DEFAULT_TICS_PER_TILE,
            wolf: None,
            dest: None,
            deadline: std::time::Instant::now(),
            at: home,
            spawn_after: std::time::Instant::now(),
            created: false,
        }
    }

    fn pick_dest(&mut self) -> (i32, i32) {
        let span = (self.radius * 2 + 1) as u32;
        (
            self.home.0 - self.radius + self.rng.below(span) as i32,
            self.home.1 - self.radius + self.rng.below(span) as i32,
        )
    }

    /// Issue one A→B move and arm the give-up deadline from the trip length.
    fn issue_move(&mut self, bot: &Bot, dest: (i32, i32)) {
        let Some(wolf) = self.wolf else { return };
        if bot.client.move_entity(wolf, dest.0, dest.1).is_err() {
            tracing::error!("engine gone during move");
            return;
        }
        let hops = (dest.0 - self.at.0).abs().max((dest.1 - self.at.1).abs()).max(1) as u64;
        // `hops + 1`: the seed hop promotes the start WITHOUT stepping (ACTIONS.md §Movement),
        // so the first step lands one tics_per_tile after the intent.
        let trip_ms = (hops + 1) * self.speed as u64 * 1000 / TIC_HZ as u64;
        self.deadline = std::time::Instant::now() + Duration::from_millis(trip_ms + 5000);
        self.dest = Some(dest);
        tracing::info!(wolf = format!("{wolf:#010x}"), from = ?self.at, to = ?dest, hops, "trip issued");
    }
}

impl Brain for Wolves {
    async fn on_start(&mut self, bot: &mut Bot) {
        let home_macro = position_macro(tile_to_position(self.home.0, self.home.1));
        // The active radius must cover the whole wander disc, or the wolf walks out of the
        // subscription and its state (and our adoption) goes dark.
        bot.anchor_and_wait("wolves", self.home.0, self.home.1, self.radius as u16 + 2, home_macro, Duration::from_secs(5)).await;

        // F5: the corpus is the def authority — no pinned constants.
        match &bot.server_url {
            Some(url) => match resolve_thing(url, "wolf").await {
                Ok((def, speed)) => {
                    self.def = def;
                    self.speed = speed;
                    tracing::info!(def = format!("{def:#010x}"), speed, "wolf packed def + speed resolved from the content corpus");
                }
                Err(err) => {
                    tracing::error!(%err, "wolf def resolution failed — cannot spawn");
                    return;
                }
            },
            None => {
                tracing::error!("no server url captured at login — cannot resolve the wolf def");
                return;
            }
        }

        // Adopt-first: give the zone snapshot a moment to replay an EXISTING minted wolf
        // (a restart re-uses its wolf); `tick` CREATEs one only if nothing shows.
        self.spawn_after = std::time::Instant::now() + Duration::from_secs(3);
        tracing::info!("awaiting an existing minted wolf (CREATE if none appears)");
    }

    fn on_event(&mut self, _bot: &Bot, event: &Event) {
        let Event::StateObject { entity_reference, definition_reference, tile_x, tile_y, removed, .. } = event
        else {
            return;
        };
        if *removed {
            // `StateGone` is TRUSTWORTHY now — the edge swallows zone-migration deletes
            // (movement-hardening P2, closing first-pawns I4), so a removal that reaches us
            // is a real despawn (or a shard wipe — movement-hardening I2's ghost: driving a
            // deleted id re-materializes it at position 0). Honor it: drop the adoption and
            // fall back to the adopt-first-then-CREATE window, exactly like a fresh start.
            if self.wolf == Some(*entity_reference) {
                tracing::warn!(wolf = format!("{entity_reference:#010x}"), "wolf removed — dropping adoption, re-entering adopt-or-CREATE");
                self.wolf = None;
                self.dest = None;
                self.created = false;
                self.spawn_after = std::time::Instant::now() + Duration::from_secs(3);
            }
            return;
        }
        // F3: adopt the first MINTED pawn with our KIND (variant-agnostic — a wolf is a wolf
        // whichever coat it wears; server ids live in the top band, so a legacy client-minted
        // wolf never matches). A legacy raw-object_id def (pre-packed rows) never equals a
        // packed def, so old rows are simply not adopted — cleaned up at the redeploy step.
        if self.wolf.is_none()
            && def_sans_variant(*definition_reference) == def_sans_variant(self.def)
            && self.def != 0
            && entity_ref_type_id(*entity_reference) == TYPE_PAWN
            && (*entity_reference & 0x00FF_FFFF) >= MINTED_BASE
        {
            self.wolf = Some(*entity_reference);
            self.at = (*tile_x, *tile_y);
            tracing::info!(wolf = format!("{entity_reference:#010x}"), at = ?self.at, "minted wolf adopted");
            return;
        }
        if self.wolf == Some(*entity_reference) {
            self.at = (*tile_x, *tile_y);
            if let Some(dest) = self.dest {
                if (*tile_x, *tile_y) == dest {
                    tracing::info!(?dest, "trip arrived (authoritative)");
                    self.dest = None;
                }
            }
        }
    }

    fn tick(&mut self, bot: &Bot) {
        if self.wolf.is_none() {
            // No wolf adopted: mint ONE, once, after the adopt-first window (ACTIONS.md
            // §CREATE — the minted id comes back through the zone's state fan-out, F3).
            if !self.created && self.def != 0 && std::time::Instant::now() > self.spawn_after {
                let spawn = self.pick_dest();
                // CREATE is variable-arity (human-pawns F2): def, position, count, payload×count.
                // The wolf carries no payload (its variant rides the def) — count 0.
                let program = vec![PROMOTE, CREATE, self.def, tile_to_position(spawn.0, spawn.1), 0];
                if bot.client.queue(program).is_err() {
                    tracing::error!("engine gone during spawn");
                    return;
                }
                self.created = true;
                tracing::info!(?spawn, def = self.def, "wolf CREATE queued; awaiting the minted id");
            }
            return;
        }
        match self.dest {
            None => {
                let dest = self.pick_dest();
                self.issue_move(bot, dest);
            }
            Some(dest) if std::time::Instant::now() > self.deadline => {
                // SAFE under chain supersession (movement-hardening F1): the new intent's
                // seed re-stamps the trip-serial, so a still-alive old chain dies at its
                // next hop — a re-issue can no longer duplicate chains. Pick a FRESH dest
                // (the old one may be exactly why the trip stalled).
                tracing::warn!(?dest, "trip deadline passed — superseding with a fresh trip");
                let fresh = self.pick_dest();
                self.issue_move(bot, fresh);
            }
            Some(_) => {}
        }
    }
}
