//! `Water Wheel` (`BG35_MagicItem_851`) — Lesser, 0 Gold, Elemental.
//!
//! After you play an Elemental, get a random Tavern spell. (Twice per turn.)
//!
//! `trinket_after_play`: the first 2 Elementals played from the hand each turn (`armed`, which
//! starts again each turn) each give a random Tavern spell, drawn as the shop draws one, which
//! waits for room in a full hand.

use crate::cards::{spells, CardHooks, Played};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130889;

/// Spells per turn.
const PER_TURN: u32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_after_play(after_play),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn after_play(state: &mut TavernState, played: &Played, _: &mut CardPool, rng: &mut Rng) {
    if !played.from_hand || played.magnetized || !played.tribe.matches(Tribe::Elemental) {
        return;
    }
    let Some(t) = super::state_mut(&mut state.auras, ID).filter(|t| t.armed < PER_TURN) else {
        return;
    };
    t.armed += 1;
    let spell = spells::draw_shop_spell(state.tavern_tier, state.auras.duos_game, rng);
    state.add_to_hand_or_wait(spell);
}
