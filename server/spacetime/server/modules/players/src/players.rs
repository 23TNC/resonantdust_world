use spacetimedb::{reducer, ReducerContext, Table};

/// The shard a freshly-claimed player is assigned to, as a `server_reference`
/// (`realm_id:8 | server_id:8` — see `docs/VARIABLES.md`).
///
/// `0` is `SERVER_REF_NONE`: realm 0, server 0 — "the one shard", while a single
/// `cards` database serves everyone. It can, since the auth DB is low-write and the
/// hot card state is what actually needs sharding. When that changes, replace this
/// constant with a real assignment policy (round-robin over live shards,
/// least-loaded, …) and stamp the result onto `Player.player_shard_reference`.
const DEFAULT_PLAYER_SHARD: u16 = 0;

/// Maximum byte length of a `Player.name`. Enforced by `validate_player_name`
/// on the input name and again after normalization in `claim_or_login`.
pub const MAX_PLAYER_NAME_LEN: usize = 64;

/// First `player_id` `next_player_id` will hand out on a fresh deployment.
/// Ids `0..FIRST_PLAYER_ID` are reserved for system / pseudo-players —
/// e.g., a "world" player that owns trees, rocks, and other unowned-by-
/// any-human world cards. Real players coming through `claim_or_login`
/// start at `FIRST_PLAYER_ID` and go up.
pub const FIRST_PLAYER_ID: u32 = 1024;

/// The in-app developer account. It logs in by this name (matching the client's
/// `DEVELOPER_NAME`) but lives at a RESERVED id ([`DEVELOPER_ID`], `< FIRST_PLAYER_ID`)
/// rather than a normal `1024+` player id, and is provisioned with `content-author`
/// pre-granted so the editor can author content with no out-of-band grant. It's
/// the one reserved-range name a human may claim.
pub const DEVELOPER_NAME: &str = "Developer";
/// Reserved id for the [`DEVELOPER_NAME`] account.
pub const DEVELOPER_ID: u32 = 512;

/// Public player identity row. Other clients mirror this table for
/// name lookups; keep it narrow so per-player private state
/// (entitlements, settings) lives in `PlayerProfile` instead.
///
/// "Which soul is this player currently controlling" is **not**
/// stored on the server — it's a purely client-side construct held
/// by `SoulManager`. Each
/// reducer that needs a soul takes `soul_card_id` explicitly (or
/// derives one via `cards::owning_soul` from a card already in
/// context). Souls themselves live as cards with
/// `FLAG_OWNED_BY_PLAYER` set and `owner_id = player_id`, so
/// "what souls does this player own" is `cards.owner_id().filter(player_id)`.
/// **One row per player** — flat, not versioned. It was a `valid_at`-keyed history
/// table; the history bought nothing (a GC sweep reaped every prior version every 10
/// minutes and nothing ever read one), so `valid_at` went with the rest of the legacy
/// bitemporal model. A mutation now updates the row in place.
#[spacetimedb::table(accessor = players, public)]
#[derive(Debug, Clone)]
pub struct Player {
    /// The player. One row per id — this is the identity, so it's the key.
    #[primary_key]
    pub player_id: u32,
    /// The shard serving this player's data — the server whose `cards`-module database
    /// holds their cards and souls. A **`server_reference`**: `realm_id:8 | server_id:8`
    /// (`docs/VARIABLES.md`). The client reads it at login to know which database to
    /// subscribe to. `0` today (realm 0, server 0 — a single shard); the assignment
    /// policy that distributes players across shards lands here when sharding splits.
    ///
    /// Was `data_shard`, a bare partition index that named nothing and couldn't address
    /// a shard in another realm. As a `server_reference` it's addressable the same way
    /// every other server is, and cross-realm falls out for free.
    ///
    /// No soul id is stored here. After connecting to the shard the client finds its
    /// soul(s) directly — `cards.owner_id().filter(player_id)` returns the player's
    /// top-level `player_soul` cards (a player can own more than one; that's the future
    /// multi-character handle). If the query is empty the client calls the shard's
    /// `spawn_soul` to mint one. These top-level player-souls are never rendered.
    pub player_shard_reference: u16,
    /// Display name. Match is case-sensitive — "Alice" and "alice" are different
    /// players.
    ///
    /// **Schema-enforced unique.** The old history schema *couldn't* use `#[unique]` —
    /// a player's own version rows would have collided on it — so uniqueness lived only
    /// in `claim_or_login`'s lookup, and any other writer silently bypassed it. Flattening
    /// the table removed that obstacle. The reducer still checks first, to fail with a
    /// readable message instead of a constraint violation.
    #[unique]
    pub name: String,
    /// Unix seconds at which this player most recently called
    /// `set_last_login`. `0` on a brand-new player (`create()` seeds
    /// it that way) until they finish their first login round-trip.
    ///
    /// Read by clients to decide the chat-subscription threshold: if
    /// this value is within the chat retention window (e.g. one hour),
    /// the client subscribes to messages since this timestamp,
    /// catching up on what was said while they were away. Otherwise
    /// they subscribe only to messages from the current login forward.
    ///
    /// The client updates this *after* installing its chat
    /// subscription — see `set_last_login`. Best-effort: a crash
    /// between read and write leaves the field stale, which just
    /// means the next login replays the same window.
    pub last_login_secs: u32,
    /// Free-form per-player flag bits. Public so other clients can
    /// read them when they need to render this player's owned
    /// surfaces (the faction subfield drives the object-texture
    /// pack picker; see [`PLAYER_FLAG_FACTION_SHIFT`] and the
    /// `objects/<faction>/<size>_<aspect>/` resolver). PlayerProfile
    /// would be the natural home for entitlement-style fields, but
    /// it's per-client subscribed — others can't see it, and
    /// other-player-owned art would render with the wrong faction.
    ///
    /// Bit layout:
    /// - bits 0..=1   — `faction` (u2) — **DEPRECATED**: faction moves to the
    ///   soul; bits reclaimable once the faction→soul migration lands.
    /// - bits 8..=15  — `permissions` capability byte (see `PERM_*` /
    ///   [`PLAYER_FLAG_PERMS_SHIFT`]). Authoritative for entitlement checks.
    /// - bits 2..=7, 16..=31 — reserved for future per-player toggles
    ///
    /// Catalog-style flag registry (mirroring `cards/flags.json`)
    /// can land once there are more fields to read by name; for
    /// today's small set, helpers below access bits directly.
    pub flags: u32,
}

/// Bit offset of the `faction` subfield inside [`Player::flags`].
/// 4 values total (`u2`); content semantics live client-side
/// today (`0 = neutral`, etc.) but the storage doesn't bake any
/// names — content can rename freely without a row migration.
pub const PLAYER_FLAG_FACTION_SHIFT: u32 = 0;
/// Mask for the `faction` subfield. Use as
/// `(player.flags >> PLAYER_FLAG_FACTION_SHIFT) & PLAYER_FLAG_FACTION_MASK`
/// to read.
pub const PLAYER_FLAG_FACTION_MASK: u32 = 0b11;

/// Extract the `faction` subfield from a player's `flags`. Returns
/// `0..=3`. Used by `claim_or_login` (default = 0) and any reducer
/// that gates on faction.
pub fn player_faction(player: &Player) -> u8 {
  ((player.flags >> PLAYER_FLAG_FACTION_SHIFT) & PLAYER_FLAG_FACTION_MASK) as u8
}

/// Re-pack a player's faction bits and write a new versioned row at
/// `time_ms`. Returns `Err` if no prior `Player` row exists for
/// `player_id`. The value is masked to the 2-bit slot — callers
/// passing `4..` lose the high bits silently (recipe authors are
/// expected to use the `Faction*` aliases in `recipes/aliases.json`).
///
/// Called by the recipe completion path (`action_completion::Effect::
/// SetPlayerFaction`) — recipes use `<owner-chain>.aspect.faction.set:
/// <int>` and the executor lands here. No direct reducer wraps this
/// today (faction is recipe-driven); add a `set_player_faction` reducer
/// here if a UI flow ever needs to call it outside the action system.
pub fn set_faction(ctx: &ReducerContext, player_id: u32, faction: u8) -> Result<(), String> {
    let faction_bits = (faction as u32) & PLAYER_FLAG_FACTION_MASK;
    let slot_mask = PLAYER_FLAG_FACTION_MASK << PLAYER_FLAG_FACTION_SHIFT;
    update_with(ctx, player_id, |p| {
        p.flags = (p.flags & !slot_mask) | (faction_bits << PLAYER_FLAG_FACTION_SHIFT);
    })
    .map(|_| ())
    .ok_or_else(|| format!("set_faction: no player row for player_id {player_id}"))
}

// ---- permissions -------------------------------------------------------
//
// A player's entitlements live in the `permissions` capability byte of
// `Player.flags` (bits 8..=15). The check is flag-based and authoritative:
// the `0..FIRST_PLAYER_ID` reserved id range is an *allocation* convention for
// system / developer accounts (the accounts you'd grant capabilities to), not
// the check itself. Capabilities compose — a granted set is the OR of `PERM_*`.

/// Bit offset of the permissions capability byte inside [`Player::flags`].
/// Disjoint from the (deprecated) faction subfield so both coexist through the
/// faction→soul migration that will later reclaim bits 0..=1.
pub const PLAYER_FLAG_PERMS_SHIFT: u32 = 8;
/// Mask for the permissions byte (8 capability bits).
pub const PLAYER_FLAG_PERMS_MASK: u32 = 0xFF;

/// May add or modify DSL content at runtime (`add_content` / `modify_content`).
pub const PERM_CONTENT_AUTHOR: u8 = 1 << 0;
// reserved capability bits: 1<<1 world-admin, 1<<2 player-admin, …

/// The player's granted capability set (the permissions byte of `flags`).
pub fn player_perms(player: &Player) -> u8 {
    ((player.flags >> PLAYER_FLAG_PERMS_SHIFT) & PLAYER_FLAG_PERMS_MASK) as u8
}

/// True iff the player holds **every** capability in `caps` (an OR of `PERM_*`).
pub fn player_has(player: &Player, caps: u8) -> bool {
    player_perms(player) & caps == caps
}

/// Re-pack a player's permissions byte and write a new versioned row at
/// `time_ms`. Returns `Err` if no prior `Player` row exists. Granting is itself
/// privileged — the caller (the gate) enforces who may invoke this; pre-release,
/// dev/system accounts in `0..FIRST_PLAYER_ID` are provisioned out-of-band.
pub fn set_permissions(ctx: &ReducerContext, player_id: u32, perms: u8) -> Result<(), String> {
    let slot_mask = PLAYER_FLAG_PERMS_MASK << PLAYER_FLAG_PERMS_SHIFT;
    update_with(ctx, player_id, |p| {
        p.flags = (p.flags & !slot_mask) | ((perms as u32) << PLAYER_FLAG_PERMS_SHIFT);
    })
    .map(|_| ())
    .ok_or_else(|| format!("set_permissions: no player row for player_id {player_id}"))
}

// (PlayerSession / Identity-keyed sessions removed — the GATE now owns the
// WS → player_id session map. The players reducers are gate-mediated: they take
// an explicit `player_id` the gate supplies, mirroring how the cards reducers
// already trust the gate's `caller_player_id`. The gate is the auth boundary.)

/// Per-player private state — the stuff the local player needs but
/// other players don't (entitlements, counters, settings). Kept off
/// the public `Player` row so other clients mirroring the player
/// table for name lookups don't pull in unlock bits etc. with it.
///
/// **Subscription pattern.** Public table, but each client only
/// subscribes to their *own* row via
/// `WHERE player_id = <caller's player_id>`. Server can't enforce
/// "no peeking at others" today — for low-sensitivity entitlement
/// data this is fine. Sensitive future fields should move to a
/// reducer-only path.
///
/// **Flat row.** One row per `player_id`, updated in place. Profile state isn't
/// time-stamped — there are no "what did the player have unlocked at time T" reads
/// downstream. (`Player` was the versioned one this contrasted against; it isn't
/// anymore — the history was deadweight there too and went with `valid_at`.)
///
/// **Initial row.** Created in `claim_or_login`'s new-player branch
/// alongside the soul spawn (`spawn_soul_for`).
#[spacetimedb::table(accessor = player_profiles, public)]
#[derive(Debug, Clone)]
pub struct PlayerProfile {
    #[primary_key]
    pub player_id: u32,
    /// Data-shard partition this row belongs to (`crate::DATA_SHARD`; `0` today).
    pub data_shard: u16,
}

fn now_ms(ctx: &ReducerContext) -> u64 {
    (ctx.timestamp.to_micros_since_unix_epoch() / 1_000) as u64
}

/// The player's row, by id. A direct primary-key lookup — one row per player.
pub fn get(ctx: &ReducerContext, player_id: u32) -> Option<Player> {
    ctx.db.players().player_id().find(player_id)
}

/// The player's row by (case-sensitive) `name`. A direct unique-index lookup; used by
/// `claim_or_login` to resolve a name → player_id without scanning the table.
pub fn get_by_name(ctx: &ReducerContext, name: &str) -> Option<Player> {
    ctx.db.players().name().find(name.to_string())
}

/// Write `player`, replacing any existing row for its id. Insert-or-update on the
/// `player_id` primary key.
fn upsert(ctx: &ReducerContext, player: Player) -> Player {
    if ctx.db.players().player_id().find(player.player_id).is_some() {
        ctx.db.players().player_id().update(player)
    } else {
        ctx.db.players().insert(player)
    }
}

/// Provision a brand-new player account: allocate the next id, write the
/// `Player` row (assigned to [`DEFAULT_PLAYER_SHARD`]) + its private `PlayerProfile`,
/// and return the new `player_id`. The single source of new-player
/// creation — shared by [`claim_or_login`]'s new-player branch and the
/// [`create_player`] reducer (registration without login). Does NOT validate the
/// name or check for collisions; callers do that first.
pub fn provision_player(ctx: &ReducerContext, name: String) -> u32 {
    let new_id = next_player_id(ctx);
    // No soul is created here — souls live in the assigned `cards` database, which
    // this module can't write to. The client (or harness) calls `spawn_soul`
    // there after reading `player_shard_reference` off this row.
    create(ctx, new_id, name, DEFAULT_PLAYER_SHARD);
    ctx.db.player_profiles().insert(PlayerProfile {
        player_id: new_id,
        data_shard: crate::DATA_SHARD,
    });
    new_id
}

/// Provision the [`DEVELOPER_NAME`] account at its reserved [`DEVELOPER_ID`] with
/// the `content-author` capability pre-granted. Unlike [`provision_player`] it
/// uses a FIXED reserved id (not `next_player_id`) and starts authorized, so the
/// in-app editor authors content with no out-of-band grant. Returns `DEVELOPER_ID`.
pub fn provision_developer(ctx: &ReducerContext) -> u32 {
    let flags = (PERM_CONTENT_AUTHOR as u32) << PLAYER_FLAG_PERMS_SHIFT;
    upsert(
        ctx,
        Player {
            player_id: DEVELOPER_ID,
            player_shard_reference: DEFAULT_PLAYER_SHARD,
            name: DEVELOPER_NAME.to_string(),
            last_login_secs: 0,
            flags,
        },
    );
    ctx.db.player_profiles().insert(PlayerProfile {
        player_id: DEVELOPER_ID,
        data_shard: crate::DATA_SHARD,
    });
    DEVELOPER_ID
}

/// Seed the server-side system accounts. Called from the module `#[init]` reducer
/// (runs once on a fresh publish), so reserved-range accounts exist before any
/// human logs in. Idempotent — skips any that already exist. Today: the developer
/// dev account at [`DEVELOPER_ID`] with content-author.
pub fn seed_system_players(ctx: &ReducerContext) {
    if get(ctx, DEVELOPER_ID).is_none() {
        provision_developer(ctx);
    }
}

// Insert a brand-new player. `player_shard_reference` is the shard this player is
// assigned to — see the `Player` struct docs.
pub fn create(
    ctx: &ReducerContext,
    player_id: u32,
    name: String,
    player_shard_reference: u16,
) -> Player {
    upsert(
        ctx,
        Player {
            player_id,
            player_shard_reference,
            name,
            // Seed at 0 — anything below `now - retention_window` is
            // interpreted by the client as "no recent session," so
            // the player's first login subscribes to no scrollback.
            last_login_secs: 0,
            // Faction = 0 (neutral). Future signup flows can pass
            // a chosen faction in here once the UI exists; today
            // every fresh account starts neutral and any later
            // mutation goes through a (TBD) `set_player_faction`
            // reducer that updates the row.
            flags: 0,
        },
    )
}

/// Read the player's row, mutate it via `f`, write it back. `None` if no row exists.
///
/// Was `update_with_at`, which picked the latest version at-or-before a `time_ms` and
/// wrote a new version at it. With one row per player there is no version to select and
/// no timestamp to stamp, so the time argument is gone — callers that had one were
/// passing server-now anyway.
pub fn update_with<F>(ctx: &ReducerContext, player_id: u32, f: F) -> Option<Player>
where
    F: FnOnce(&mut Player),
{
    let mut p = get(ctx, player_id)?;
    f(&mut p);
    Some(upsert(ctx, p))
}

// ---- name handling ----------------------------------------------------

/// Validate a name before inserting it into a `Player` row.
///
/// Length is checked in bytes (not chars) since storage is what we're bounding.
/// Whitespace-only and control-character names are rejected so that no player
/// can render as blank or smuggle in characters that break logging / display.
pub fn validate_player_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("player name cannot be empty".to_string());
    }
    if name.trim().is_empty() {
        return Err("player name cannot be only whitespace".to_string());
    }
    if name.chars().any(|c| c.is_control()) {
        return Err("player name cannot contain control characters".to_string());
    }
    if name.len() > MAX_PLAYER_NAME_LEN {
        return Err(format!(
            "player name length {} exceeds max {}",
            name.len(),
            MAX_PLAYER_NAME_LEN,
        ));
    }
    Ok(())
}

// ---- player lifecycle -------------------------------------------------

/// Single-row counter table holding the next player_id to allocate.
/// Private — internal allocator state. Mirrors `CardIdCounter`.
#[spacetimedb::table(accessor = player_id_counter)]
pub struct PlayerIdCounter {
    #[primary_key]
    pub id: u8,
    pub next: u32,
}

/// Allocate the next `player_id` in O(1). Backed by a single-row
/// counter table; lazy-seeded from the current `max(player_id) + 1`
/// on the first call after a fresh deployment, O(1) thereafter.
///
/// Previously a full scan over `players` history — fine when small,
/// slow once every login or mutation has accumulated version rows.
fn next_player_id(ctx: &ReducerContext) -> u32 {
    if let Some(counter) = ctx.db.player_id_counter().id().find(0) {
        let allocated = counter.next;
        ctx.db.player_id_counter().id().delete(0);
        ctx.db.player_id_counter().insert(PlayerIdCounter {
            id: 0,
            next: allocated.saturating_add(1),
        });
        allocated
    } else {
        // Lazy seed — one full scan, paid exactly once after a fresh
        // deployment. Two constraints:
        //  - Must include existing players so the counter doesn't hand
        //    out ids that already exist (covers databases that pre-date
        //    the counter table or were migrated from an older schema).
        //  - Must start at least at `FIRST_PLAYER_ID` so the
        //    `0..FIRST_PLAYER_ID` reserved range stays free for
        //    system / pseudo-players (e.g., a world-owner for trees,
        //    rocks, etc.).
        // The `.max(FIRST_PLAYER_ID)` clamp picks whichever lower bound
        // is stricter — existing data wins if it ran past the reserve.
        let current_max = ctx
            .db
            .players()
            .iter()
            .map(|p| p.player_id)
            .max()
            .unwrap_or(0);
        let allocated = current_max.saturating_add(1).max(FIRST_PLAYER_ID);
        ctx.db.player_id_counter().insert(PlayerIdCounter {
            id: 0,
            next: allocated.saturating_add(1),
        });
        allocated
    }
}

/// Delete `player_id`'s row.
///
/// **Cards/souls are NOT cascaded here** — they live in the player's
/// assigned `cards` shard (a separate database this module can't write
/// to). Reaping a deleted player's cards is the card shard's job: its
/// GC sweep reaps world-/owner-dead rows, and a dedicated card-side
/// purge reducer (gateway-driven) can hard-delete on account removal.
pub fn delete_player(ctx: &ReducerContext, player_id: u32) {
    ctx.db.players().player_id().delete(player_id);
}


/// Stamp `last_login_secs` with the server's current wall-clock time
/// for the caller's player. Called by the client *after* it has read
/// the previous `last_login_secs` and installed its chat subscription
/// — see the field doc on `Player.last_login_secs`. Idempotent in the
/// sense that repeated calls just keep bumping the timestamp.
///
/// `player_id` is supplied by the gate (which owns the session) — same
/// gate-mediated auth pattern as the cards reducers' `caller_player_id`.
///
/// `client_time_ms` is accepted for wire-format parity but ignored — the row is
/// stamped at server-now (the render delay is applied client-side).
///
/// No-op (returns `Ok`) if the player has no prior row, which can
/// happen mid-creation; the next login will land on a real row.
#[reducer]
pub fn set_last_login(
    ctx: &ReducerContext,
    // Named without a leading underscore: SpacetimeDB's `/call` keys args on the
    // exact Rust param name, so `_client_time_ms` would be unaddressable by the
    // gate relay. Accepted but unused (the reducer stamps at server-now).
    client_time_ms: u64,
    player_id: u32,
) -> Result<(), String> {
    let _ = client_time_ms;
    // Stamp at server-now. The render delay `D` is applied client-side (a single
    // shared value), so the server writes true times and never back-stamps.
    let now_secs = (now_ms(ctx) / 1_000) as u32;
    update_with(ctx, player_id, |p| {
        p.last_login_secs = now_secs;
    });
    Ok(())
}


/// Trust-on-first-use registration / login.
///
/// If no `Player` exists with the given (case-sensitive) name, one is
/// created (with a `player_shard_reference` + profile). Either way the player row's
/// `last_login_secs` is bumped. The **gate** reads the resulting `player_id`
/// off the player row (looked up by name) and records the WS → player_id
/// session itself — this reducer no longer establishes an Identity session.
///
/// **This is intentionally insecure.** Anyone can call `claim_or_login`
/// with any name and become that player — there is no password, token, or
/// external auth check. Replace this with token-based or external auth
/// before exposing the module to anyone you don't trust.
#[reducer]
pub fn claim_or_login(
    ctx: &ReducerContext,
    // No leading underscore — `/call` keys on the exact param name, so the gate
    // relay must be able to address it. Accepted but unused (server-now stamp).
    client_time_ms: u64,
    name: String,
) -> Result<(), String> {
    let _ = client_time_ms;
    validate_player_name(&name)?;
    // Bootstrap login. `client_time_ms` is accepted for wire-format parity but
    // ignored — the row is stamped at server-now. The render delay is applied
    // client-side, and the gate reads this row server-side (it isn't projected on
    // the client), so no back-stamp is needed.
    let now_ms = now_ms(ctx);

    let player_id = match get_by_name(ctx, &name) {
        Some(player) => {
            // Reserved-range players (`__world__`, future NPC owners,
            // etc.) live at `player_id < FIRST_PLAYER_ID`. They're
            // server-internal and must never be claimable by a human
            // — claiming would let a player drag-pick the world's
            // entire tree / rock inventory through normal inventory
            // ops. `next_player_id` already starts above the reserve,
            // so this branch is the only entry point that could resolve
            // to a reserved id (via name lookup of a server-seeded row).
            // The developer dev account is the one reserved-range name a human may
            // claim (a fixed id + pre-granted content-author); every other reserved
            // account stays server-internal.
            if player.player_id < FIRST_PLAYER_ID && name != DEVELOPER_NAME {
                return Err(format!(
                    "player name {:?} is reserved",
                    name
                ));
            }
            player.player_id
        }
        None => {
            // Shared new-player provisioning (id + Player row + profile). The
            // client connects to the assigned card shard and calls `spawn_soul`
            // there to mint the player_soul if it owns none yet. The developer is
            // provisioned at its reserved id with content-author instead.
            if name == DEVELOPER_NAME {
                provision_developer(ctx)
            } else {
                provision_player(ctx, name)
            }
        }
    };

    // The gate establishes the WS → player_id session (it reads this
    // player_id off the subscribed player row, looked up by name); the
    // players module no longer tracks an Identity-keyed session.

    // Always stamp a fresh `last_login_secs` row, even on the
    // existing-player branch. Two reasons:
    //
    //   1. **Welcome-back stamp.** Players returning to the game
    //      naturally re-bump the timestamp; the client uses it to
    //      decide which chat scrollback window to subscribe to.
    //   2. **Clock-sync bootstrap.** This is the one row write the
    //      client *needs* to receive as a `Reducer`-tagged event so
    //      `noteServerTime` can seed the offset window. On the
    //      new-player branch `create_at` above already wrote a row; on
    //      the existing-player branch nothing else in the reducer
    //      writes to a subscribed table (`player_sessions` is
    //      server-private). Without this update, returning players
    //      would land with an empty offset window and the first
    //      subsequent reducer would fail the grace check.
    update_with(ctx, player_id, |p| {
        p.last_login_secs = (now_ms / 1_000) as u32;
    });

    Ok(())
}

/// Create a player account **without logging in** — registration / programmatic
/// provisioning (the future "players create their users" path, and how the test
/// harness seeds extra players to craft initial conditions). Shares
/// [`provision_player`] with [`claim_or_login`]; the only difference is this
/// always *creates* (erroring on a taken name) and never establishes a session
/// or bumps `last_login_secs`. The player_soul is minted separately via
/// `spawn_soul` on the card shard, same as the login path.
///
/// **Intentionally insecure**, like `claim_or_login`: no auth. Stamps at
/// server-now (no back-stamp; the render delay is applied client-side).
#[reducer]
pub fn create_player(
    ctx: &ReducerContext,
    // No leading underscore — `/call` keys on the exact param name. Unused
    // (server-now stamp), kept for wire-format consistency.
    client_time_ms: u64,
    name: String,
) -> Result<(), String> {
    let _ = client_time_ms;
    validate_player_name(&name)?;
    // Checked first so a taken name fails with this message rather than as a raw
    // unique-constraint violation on `Player.name`.
    if get_by_name(ctx, &name).is_some() {
        return Err(format!("create_player: a player named {name:?} already exists"));
    }
    provision_player(ctx, name);
    Ok(())
}

// (client_disconnected removed — there's no Identity-keyed session to reap.
// The gate owns the WS → player_id map and drops it when the client's WS closes;
// gate sessions are ephemeral, reconstructed on reconnect from shard truth.)
