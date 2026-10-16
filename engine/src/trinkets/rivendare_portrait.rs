//! `Rivendare Portrait` (`BG30_MagicItem_310`) — Greater, 3 Gold, Beast.
//!
//! Get a Titus Rivendare. Start of Combat: Double the Health of your Titus Rivendares.
//!
//! Picked, it gives a plain `Titus Rivendare` from the catalog (not the pool; it waits for room in
//! a full hand: `docs/trinkets.md` §5). Step 1c: every friendly `Titus Rivendare` (Golden too)
//! gains its Health.

use crate::cards::minions::titus_rivendare;
use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112102;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_start_of_combat(start_of_combat),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, titus_rivendare::ID);
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    for i in 0..ctx.own.board.len() {
        if ctx.own.board[i].card_id == titus_rivendare::ID {
            let health = ctx.own.board[i].health.max(0);
            ctx.own.buff_unit(i, 0, health, "Rivendare Portrait");
        }
    }
}
