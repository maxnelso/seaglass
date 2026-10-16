//! `Ragnaros the Firelord` (`TB_BaconShop_HERO_11`) — Armor 15 (Duos 15).
//!
//! Hero Power: `BUY, INSECT!` (`TB_BaconShop_HP_087`), 0 Gold: After you buy 12 cards, get Sulfuras. (12 left!)
//!
//! Passive. Every card bought counts (`hero_after_buy`, `counter`, across turns); the 12th turns
//! the Hero Power into *Sulfuras* (`TB_BaconShop_HP_087t`; its `card_id` changes, as Bru'kan's
//! Elements do): "At the end of your turn, give your left and right-most minions +8/+8"
//! (`hero_end_turn`; a single minion gains it once: confirmed).

use crate::cards::CardHooks;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 57892;
pub const POWER: CardId = 64424;

/// Sulfuras (`TB_BaconShop_HP_087t`), the Hero Power it becomes.
pub const SULFURAS: CardId = 64426;

/// Cards to buy.
const BUYS: i32 = 12;

/// Sulfuras' buff.
const BUFF: i32 = 8;

/// The Hero Power's hooks and Sulfuras', for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (POWER, CardHooks::EMPTY.on_hero_after_buy(after_buy)),
        (SULFURAS, CardHooks::EMPTY.on_hero_end_turn(sulfuras)),
    ]
}

/// After a card is bought: the 12th gives Sulfuras.
fn after_buy(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.counter += 1;
    if power.counter >= BUYS {
        power.counter = 0;
        power.card_id = SULFURAS;
    }
}

/// Sulfuras, end of turn: the left- and right-most minions gain +8/+8.
fn sulfuras(state: &mut TavernState, repeats: u32, _: &mut CardPool, _: &mut Rng) {
    let Some(last) = state.board.len().checked_sub(1) else {
        return;
    };
    for _ in 0..repeats {
        state.board[0].add_stats(BUFF, BUFF);
        if last > 0 {
            state.board[last].add_stats(BUFF, BUFF);
        }
    }
}
