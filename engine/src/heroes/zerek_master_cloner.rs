//! `Zerek, Master Cloner` (`BG31_HERO_005`) — Armor 12 (Duos 10).
//!
//! Hero Power: `Cloning Gallery` (`BG31_HERO_005p`), 0 Gold: Once per game, summon an exact copy of a friendly minion.
//!
//! Active, a friendly minion, once per game (`stacks` is 1 once used): an exact copy of it (stats,
//! keywords, state; not its mark) is summoned to its right (`cards::on_tavern_summon`: the summon
//! observers see it), and it can make a triple. It needs room on the board.

use crate::cards::{self, ActivateTargetKind, CardHooks};
use crate::combat::MAX_BOARD_SIZE;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 117409;
pub const POWER: CardId = 117410;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(cloning_gallery)
            .on_hero_power_usable(|state| {
                super::unused_this_game(state) && state.board.len() < MAX_BOARD_SIZE
            })
            .with_activate_target(ActivateTargetKind::BoardAny),
    )]
}

/// Using it: an exact copy to the minion's right, once per game.
fn cloning_gallery(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.stacks = 1;
    }
    let Some(t) = target.filter(|&t| t < state.board.len()) else {
        return;
    };
    let mut copy = state.board[t].clone();
    copy.marked = false;
    let card_id = copy.card_id;
    state.board.insert(t + 1, copy);
    cards::on_tavern_summon(&mut state.board, t + 1);
    state.check_and_resolve_triple(card_id);
}
