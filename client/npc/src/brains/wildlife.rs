//! wildlife — the LEGACY pack behavior, ported unchanged onto the [`Brain`] trait (first-pawns
//! P4 acceptance: behavior preserved): place 4 CLIENT-minted wolves in zone (0,0), then move
//! each to a random cell every tick. Superseded by [`super::wolves`] (server-minted `CREATE` +
//! A→B moves) — kept as the harness's regression fixture until the wolves-brain fully lands.

use std::time::Duration;

use client::Event;
use resonantdust_codec::object::TYPE_PAWN;
use resonantdust_codec::refs::{pack_entity_reference, pack_server_reference};

use crate::{Bot, Brain, Rng};

const ZONE_DIM: u32 = 16;
const WOLF_COUNT: u32 = 4;

/// The stable `entity_reference` the legacy pack addresses a wolf by. Client-minted (the
/// stopgap `CREATE` replaces): `TYPE_PAWN` server byte, object `index + 1`.
fn wolf_key(index: u32) -> u32 {
    pack_entity_reference(pack_server_reference(TYPE_PAWN, 0), index + 1)
}

pub struct Wildlife {
    rng: Rng,
}

impl Wildlife {
    pub fn new(rng: Rng) -> Self {
        Self { rng }
    }
}

impl Brain for Wildlife {
    async fn on_start(&mut self, bot: &mut Bot) {
        bot.anchor_and_wait("wildlife", 0, 0, 2, 0, Duration::from_secs(5)).await;
        for i in 0..WOLF_COUNT {
            let (x, y) = (self.rng.below(9) as i32, self.rng.below(6) as i32);
            if bot.client.place(wolf_key(i), x, y).is_err() {
                tracing::error!("engine gone during spawn");
                return;
            }
        }
        tracing::info!(wolves = WOLF_COUNT, "spawned wolf pack in zone (0,0)");
    }

    fn on_event(&mut self, _bot: &Bot, _event: &Event) {}

    fn tick(&mut self, bot: &Bot) {
        for i in 0..WOLF_COUNT {
            let (x, y) = (self.rng.below(ZONE_DIM) as i32, self.rng.below(ZONE_DIM) as i32);
            if bot.client.move_entity(wolf_key(i), x, y).is_err() {
                tracing::error!("engine gone during move");
                return;
            }
        }
    }
}
