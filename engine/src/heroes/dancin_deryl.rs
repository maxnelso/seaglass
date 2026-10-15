//! `Dancin' Deryl` (`TB_BaconShop_HERO_36`) — Armor 18 (Duos 16).
//!
//! Hero Power: `Hat Trick` (`TB_BaconShop_HP_042`), 0 Gold: When you play a minion, give it a +1/+1 hat that passes to a friendly minion when sold.
//!
//! Passive (`docs/heroes.md` §11.2): `hero_after_play` gives a minion played from hand `+1/+1` and
//! `hats += 1`. When a minion with hats is sold (`hero_after_sell`, or `pass_hats` if the player
//! no longer has *Hat Trick*), each hat gives a random remaining friendly board minion `+1/+1` and
//! `hats += 1`.

use crate::cards::{CardHooks, Played};
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 59806;
pub const POWER: CardId = 59860;

pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_after_play(after_play)
            .on_hero_after_sell(after_sell),
    )]
}

fn after_play(state: &mut TavernState, played: &Played, _: &mut CardPool, _: &mut Rng) {
    if !played.from_hand {
        return;
    }
    if let Some(unit) = state.board.get_mut(played.board_pos) {
        unit.add_stats(1, 1);
        unit.hats += 1;
    }
}

fn after_sell(state: &mut TavernState, sold: &Unit, _: &mut CardPool, rng: &mut Rng) {
    pass_hats(state, sold.hats, rng);
}

/// Pass `hats` +1/+1 hats to random remaining friendly board minions (`docs/heroes.md` §11.2).
pub fn pass_hats(state: &mut TavernState, hats: u32, rng: &mut Rng) {
    for _ in 0..hats {
        if state.board.is_empty() {
            break;
        }
        let idx = rng.below(state.board.len());
        state.board[idx].add_stats(1, 1);
        state.board[idx].hats += 1;
    }
}
