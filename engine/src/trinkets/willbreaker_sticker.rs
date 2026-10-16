//! `Willbreaker Sticker` (`BG36_MagicItem_852`) — Lesser, 2 Gold.
//!
//! Get 2 random Tavern spells. When you play one, discard the other. At the start of your turn, repeat this.
//!
//! Picked, and at each turn start: 2 random Tavern spells (drawn as the shop draws one: the
//! player's Tier or lower, Duos spells in Duos) go to the hand as one group of a single play, as
//! `Wandering Willbreaker`'s (`tavern.md` §6): playing one discards the other. They wait for room
//! in a full hand (`docs/trinkets.md` §5).

use crate::cards::{spells, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134639;

/// Spells each time.
const SPELLS: usize = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(give)
            .on_trinket_turn_start(give),
    )]
}

fn give(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    let group = state.next_willbreaker_group;
    state.next_willbreaker_group += 1;
    for _ in 0..SPELLS {
        let mut spell = spells::draw_shop_spell(state.tavern_tier, state.auras.duos_game, rng);
        spell.willbreaker_group = group;
        spell.willbreaker_remaining = 1;
        state.add_to_hand_or_wait(spell);
    }
}
