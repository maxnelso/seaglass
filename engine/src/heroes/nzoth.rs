//! `N'Zoth` (`TB_BaconShop_HERO_93`) — Armor 12 (Duos 10).
//!
//! Hero Power: `Avatar of N'Zoth` (`TB_BaconShop_HP_105`), 0 Gold: Start the game with a 2/2 Fish that gains all your Deathrattles in combat.
//!
//! Passive. When the player gets the hero (`hero_chosen`), a `Fish of N'Zoth` (a Tier 1 `2/2`
//! Beast token) joins the board if it has room. The Fish (`TB_BaconShop_HP_105t`): "After a
//! different friendly Deathrattle minion dies in combat, gain its Deathrattle": its
//! `friendly_death` hook adds the dead minion's Deathrattle (its card's, then those an effect gave
//! it) to its `extra_deathrattles`, for the combat. A gained Deathrattle resolves with the Fish as
//! its minion (its Golden, its stats; engine decision).

use crate::cards::{self, tokens, BoardCtx, CardHooks};
use crate::combat::MAX_BOARD_SIZE;
use crate::model::{CardId, HeroId, Unit};
use crate::tavern::TavernState;

pub const ID: HeroId = 66483;
pub const POWER: CardId = 66484;

/// The Hero Power's hooks and the Fish's, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (POWER, CardHooks::EMPTY.on_hero_chosen(start_with_a_fish)),
        (
            tokens::TOKEN_FISH_OF_NZOTH,
            CardHooks::EMPTY.on_friendly_death(gain_deathrattle),
        ),
    ]
}

/// The start of the game: the Fish joins the board.
fn start_with_a_fish(state: &mut TavernState) {
    if state.board.len() < MAX_BOARD_SIZE {
        state.board.push(tokens::make_fish_of_nzoth());
        state.sync_all_auras();
    }
}

/// A different friendly minion died in combat: the Fish gains its Deathrattles.
fn gain_deathrattle(ctx: &mut BoardCtx<'_>, self_idx: usize, dying: &Unit) {
    if !ctx.in_combat
        || ctx
            .board
            .get(self_idx)
            .is_none_or(|fish| fish.id == dying.id)
    {
        return;
    }
    let own = cards::is_deathrattle_minion(dying.card_id).then_some(dying.card_id);
    let gained: Vec<CardId> = own
        .into_iter()
        .chain(dying.extra_deathrattles.iter().copied())
        .collect();
    ctx.board[self_idx].extra_deathrattles.extend(gained);
}
