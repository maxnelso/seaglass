//! `Eternal Portrait` (`BG30_MagicItem_301`) — Lesser, 6 Gold, Undead.
//!
//! Get an Eternal Knight. Start of Combat: Give your Eternal Knights Taunt and Reborn.
//!
//! Picked, it gives a plain `Eternal Knight` from the catalog (not the pool; it waits for room in
//! a full hand: `docs/trinkets.md` §5). Step 1c: every friendly `Eternal Knight` (Golden too)
//! gains Taunt and Reborn.

use crate::cards::minions::eternal_knight;
use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::{CardId, Keyword};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 111388;

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
    super::give_minion(state, eternal_knight::ID);
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    for unit in ctx.own.board.iter_mut() {
        if unit.card_id == eternal_knight::ID {
            unit.apply_keyword(Keyword::Taunt, false);
            unit.apply_keyword(Keyword::Reborn, false);
        }
    }
}
