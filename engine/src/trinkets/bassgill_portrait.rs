//! `Bassgill Portrait` (`BG32_MagicItem_301`) — Greater, 1 Gold, Murloc.
//!
//! Get a Bassgill. After you summon a Murloc in combat, give it Divine Shield.
//!
//! Picked: a plain `Bassgill`, a minion in no pool (`docs/trinkets.md` §11.9).
//! `trinket_combat_summon`: every friendly Murloc summoned in combat (by a Deathrattle, a Reborn,
//! from the hand, ...) gets Divine Shield.

use crate::cards::minions::bassgill;
use crate::cards::CardHooks;
use crate::model::{CardId, Keyword, Tribe, TrinketState, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 120942;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_combat_summon(combat_summon),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, bassgill::ID);
}

fn combat_summon(_: &mut TrinketState, summoned: &mut Unit) {
    if summoned.tribe.matches(Tribe::Murloc) {
        summoned.apply_keyword(Keyword::DivineShield, false);
    }
}
