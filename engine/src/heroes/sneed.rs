//! `Sneed` (`BG21_HERO_030`) — Armor 15 (Duos 13).
//!
//! Hero Power: `Pilot the Shredder` (`BG21_HERO_030p`), 0 Gold: Start the game with a 2/1 Shredder that summons a minion from your hand and gives it Divine Shield.
//!
//! Passive. When the player gets the hero (`hero_chosen`), a `Sneed's New Shredder` (a Tier 1
//! `2/1` Mech token) joins the board if it has room. The Shredder (`BG21_HERO_030t`):
//! "Deathrattle: Summon the highest-Health minion from your hand and give it Divine Shield for
//! this combat": a copy of the highest-Health minion in the hand (a tie at random: one
//! `rng.below(k)`) not yet summoned from the hand this combat, with Divine Shield, as `Diremuck
//! Forager`'s hand summons; a Golden one, the two highest (engine decision).

use crate::cards::{tokens, BoardCtx, CardHooks};
use crate::combat::MAX_BOARD_SIZE;
use crate::model::{CardId, HeroId, Unit};
use crate::tavern::TavernState;

pub const ID: HeroId = 76521;
pub const POWER: CardId = 76520;

/// The Hero Power's hooks and the Shredder's, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            POWER,
            CardHooks::EMPTY.on_hero_chosen(start_with_a_shredder),
        ),
        (
            tokens::TOKEN_SNEEDS_SHREDDER,
            CardHooks::EMPTY.on_deathrattle(shredder_deathrattle),
        ),
    ]
}

/// The start of the game: the Shredder joins the board.
fn start_with_a_shredder(state: &mut TavernState) {
    if state.board.len() < MAX_BOARD_SIZE {
        state.board.push(tokens::make_sneeds_shredder());
        state.sync_all_auras();
    }
}

/// Deathrattle: summon the highest-Health minion(s) from the hand, with Divine Shield.
fn shredder_deathrattle(dying: &Unit, ctx: &mut BoardCtx<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let candidates: Vec<usize> = (0..ctx.hand.len())
            .filter(|&i| {
                !ctx.hand[i].is_spell && !ctx.hand_summoned.get(i).copied().unwrap_or(true)
            })
            .collect();
        let Some(best) = candidates.iter().map(|&i| ctx.hand[i].health).max() else {
            return;
        };
        let tied: Vec<usize> = candidates
            .into_iter()
            .filter(|&i| ctx.hand[i].health == best)
            .collect();
        let hand_idx = match tied.len() {
            1 => tied[0],
            k => tied[ctx.rng.below(k)],
        };
        let mut copy = ctx.hand[hand_idx].clone();
        copy.divine_shield = true;
        if ctx.summon_as(dying.id, copy, "Deathrattle").is_none() {
            return;
        }
        ctx.hand_summoned[hand_idx] = true;
    }
}
