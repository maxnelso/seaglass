//! `Ur'zul Sticker` (`BG35_MagicItem_154`) — Greater, 0 Gold, Demon.
//!
//! After you play a Demon, another friendly Demon consumes a minion in the Tavern to gain its stats.
//!
//! `trinket_after_play`: after a Demon is played (not Magnetized, as `Insatiable Ur'zul`), a
//! random other Demon on the board, then a random minion in the Tavern (one draw each), and the
//! Demon consumes it (`trinkets::consume_shop_minion`); nothing without both.

use crate::cards::{CardHooks, Played};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131282;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_play(after_play))]
}

fn after_play(state: &mut TavernState, played: &Played, pool: &mut CardPool, rng: &mut Rng) {
    if played.magnetized || !played.tribe.matches(Tribe::Demon) {
        return;
    }
    let demons: Vec<usize> = (0..state.board.len())
        .filter(|&i| i != played.board_pos && state.board[i].tribe.matches(Tribe::Demon))
        .collect();
    let shop: Vec<usize> = (0..state.shop.len())
        .filter(|&i| !state.shop[i].is_spell)
        .collect();
    if demons.is_empty() || shop.is_empty() {
        return;
    }
    let demon = super::pick_tied(&demons, rng);
    let minion = super::pick_tied(&shop, rng);
    super::consume_shop_minion(state, demon, minion, pool);
}
