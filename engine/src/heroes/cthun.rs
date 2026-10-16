//! `C'Thun` (`TB_BaconShop_HERO_29`) — Armor 20 (Duos 18).
//!
//! Hero Power: `Saturday C'Thuns!` (`TB_BaconShop_HP_104`), 1 Gold: At end of turn, give a friendly minion +1/+1. Repeat 0 times. (Improves each turn!)
//!
//! Active, untargeted, once per turn: using it arms the turn's end (`armed`). At the end of the
//! turn (`hero_end_turn`), `turn` times (once on turn 1, one more each turn: engine decision), a
//! random friendly minion gains +1/+1 (one `rng.below(k)` each). With end-of-turn repeats
//! (`Drakkari Enchanter`, `Ghastly Sticker`: `docs/tavern.md` §7.5) all of that repeats, and the
//! arming is used once (confirmed).

use crate::cards::CardHooks;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 58535;
pub const POWER: CardId = 66246;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(arm)
            .on_hero_end_turn(end_turn),
    )]
}

/// Using it: the turn's end buffs.
fn arm(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.armed = 1;
    }
}

/// End of turn, armed: `turn` random friendly minions gain +1/+1.
fn end_turn(state: &mut TavernState, repeats: u32, _: &mut CardPool, rng: &mut Rng) {
    // The arming is used once, after all the repeats (`docs/tavern.md` §7.5): taking it first is
    // the same, as nothing below reads it.
    let armed = power_mut(&mut state.auras).map_or(0, |p| std::mem::take(&mut p.armed));
    if armed == 0 {
        return;
    }
    for _ in 0..repeats {
        for _ in 0..state.turn.max(1) {
            let pick = match state.board.len() {
                0 => return,
                1 => 0,
                k => rng.below(k),
            };
            state.board[pick].add_stats(1, 1);
        }
    }
}
