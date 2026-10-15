//! `Zephrys, the Great` (`TB_BaconShop_HERO_91`) — Armor 17 (Duos 15).
//!
//! Hero Power: `Three Wishes` (`TB_BaconShop_HP_102`), 3 Gold: If you have two copies of a minion, find the third. (3 Wishes left!)
//!
//! Active, untargeted, once per turn, three times per game (`stacks`): with two non-Golden copies
//! of a minion on the board and in the hand (the first such pair, board then hand, left to right:
//! engine decision), the third (from the pool if it has a copy, else a plain one from the catalog)
//! goes to the hand, where they Triple. It needs room in the hand.

use crate::cards::{self, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 64485;
pub const POWER: CardId = 64486;

/// Wishes per game.
const WISHES: i32 = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(three_wishes)
            .on_hero_power_usable(|state| {
                super::active_power(state).is_some_and(|p| p.stacks < WISHES)
                    && super::hand_has_room(state)
                    && pair(state).is_some()
            }),
    )]
}

/// The card of the first pair of non-Golden copies, board then hand.
fn pair(state: &TavernState) -> Option<CardId> {
    let cards: Vec<CardId> = state
        .board
        .iter()
        .chain(&state.hand)
        .filter(|u| !u.is_spell && !u.is_golden && cards::template(u.card_id).is_some())
        .map(|u| u.card_id)
        .collect();
    cards
        .iter()
        .copied()
        .find(|&id| cards.iter().filter(|&&c| c == id).count() >= 2)
}

/// Using it: the third copy.
fn three_wishes(state: &mut TavernState, _: Option<usize>, pool: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.stacks += 1;
    }
    let Some(card_id) = pair(state) else {
        return;
    };
    let Some(template) = cards::template(card_id) else {
        return;
    };
    if pool.remaining_copies(card_id) > 0 {
        pool.take_copy(card_id);
    }
    state.add_to_hand(template.instantiate());
}
