//! `Converter Portrait` (`BG36_MagicItem_600`) — Greater, 6 Gold.
//!
//! Get a Faceless Converter. Start of Combat: Give your Faceless Converters Reborn.
//!
//! Picked, it gives a plain `Faceless Converter` from the catalog (not the pool; it waits for
//! room in a full hand: `docs/trinkets.md` §5). Step 1c: every friendly `Faceless Converter`
//! (Golden too) gains Reborn.

use crate::cards::minions::faceless_converter;
use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::{CardId, Keyword};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134453;

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
    super::give_minion(state, faceless_converter::ID);
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    for unit in ctx.own.board.iter_mut() {
        if unit.card_id == faceless_converter::ID {
            unit.apply_keyword(Keyword::Reborn, false);
        }
    }
}
