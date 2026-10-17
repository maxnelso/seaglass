//! `Eternal Portrait (Greater)` (`BG36_MagicItem_216`) — Greater, 5 Gold, Undead.
//!
//! Get a Golden Eternal Knight. After a friendly Undead dies, your Eternal Knights have +4/+2 this game.
//!
//! Picked: a Golden `Eternal Knight` from the catalog. Each friendly Undead that dies, in combat
//! (`trinket_friendly_death`) or destroyed in the Tavern (`trinket_tavern_destroy`), counts as an
//! Eternal Knight that died for the Eternal Knights' bonus (+4/+2 each, doubled for a Golden one;
//! their card's counter), on top of the Knight's own count: a Knight that dies counts twice
//! **(engine decision)**.

use crate::cards::minions::eternal_knight;
use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133404;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_friendly_death(friendly_death)
            .on_trinket_tavern_destroy(tavern_destroy),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_golden_minion(state, eternal_knight::ID);
}

fn friendly_death(ctx: &mut BoardCtx<'_>, dying: &Unit) {
    if dying.tribe.matches(Tribe::Undead) {
        ctx.auras.add_counter(eternal_knight::ID, 1);
    }
}

fn tavern_destroy(state: &mut TavernState, destroyed: &Unit, _: &mut CardPool, _: &mut Rng) {
    if destroyed.tribe.matches(Tribe::Undead) {
        state.auras.add_counter(eternal_knight::ID, 1);
        state.sync_all_auras();
    }
}
