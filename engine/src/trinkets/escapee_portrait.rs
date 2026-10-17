//! `Escapee Portrait` (`BG36_MagicItem_363`) — Greater, 7 Gold, Pirate.
//!
//! Get a Golden Enterprising Escapee. Your Lockboxes open 1 turn sooner.
//!
//! Picked: a Golden `Enterprising Escapee` from the catalog (made Golden, as `Golden Touch` does).
//! Every `Lockbox` in the hand opens 1 turn sooner, once: as picked, at each turn start and after
//! each action, each Lockbox the trinket hasn't shortened yet (`Unit::trinket_marks`) counts down
//! by 1, and one at 0 opens (`cards::resolve_ready_hand_cards`) **(engine decision**: checked
//! then**)**.

use crate::cards::minions::enterprising_escapee;
use crate::cards::{self, tokens, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133714;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_turn_start(sooner)
            .on_trinket_after_action(sooner),
    )]
}

fn bought(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    super::give_golden_minion(state, enterprising_escapee::ID);
    sooner(state, pool, rng);
}

fn sooner(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    let mut shortened = false;
    for card in &mut state.hand {
        if card.card_id == tokens::SPELL_LOCKBOX
            && card.lockbox_turns_left > 0
            && !card.trinket_marks.contains(&ID)
        {
            card.lockbox_turns_left -= 1;
            card.trinket_marks.push(ID);
            shortened = true;
        }
    }
    if shortened {
        cards::resolve_ready_hand_cards(state, rng);
    }
}
