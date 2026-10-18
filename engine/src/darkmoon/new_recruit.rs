//! `New Recruit` (`BGS_Treasures_033`) — Tier 1 Darkmoon Prize.
//!
//! The Tavern offers an extra minion with +2/+2 this game.
//!
//! A game-long player effect (one stack per cast): every rolled shop holds one more minion per
//! stack (`player_shop_slots`; never more than the Tavern's 7 cards, the spell slot included), and
//! after each roll (a Refresh, an unfrozen turn start: `player_after_refresh`) its right-most
//! minion per stack gains +2/+2 (engine decision: the extra minion is the right-most one; a frozen
//! shop's kept minions and the minions that fill its free slots gain nothing).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, EffectDuration, PlayerEffect, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState, MAX_SHOP_SIZE};

pub const ID: CardId = 66463;

/// The extra minion's stats.
const BUFF: i32 = 2;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        spell(cast)
            .on_player_shop_slots(shop_slots)
            .on_player_after_refresh(after_refresh),
    )]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.auras.add_effect(ID, 1, EffectDuration::Game);
}

fn shop_slots(effect: &PlayerEffect, state: &TavernState, slots: usize) -> usize {
    let room = MAX_SHOP_SIZE - usize::from(state.include_shop_spells);
    (slots + effect.stacks as usize).min(room).max(slots)
}

fn after_refresh(state: &mut TavernState, effect: &mut PlayerEffect, _: &mut Rng) {
    let minions: Vec<usize> = (0..state.shop.len())
        .filter(|&i| !state.shop[i].is_spell)
        .collect();
    for &i in minions.iter().rev().take(effect.stacks as usize) {
        state.shop[i].add_stats(BUFF, BUFF);
    }
}
