//! `Maiev Shadowsong` (`TB_BaconShop_HERO_62`) — Armor 18 (Duos 17).
//!
//! Hero Power: `Imprison` (`TB_BaconShop_HP_068`), 1 Gold: Choose a card in the Tavern to lock in your hand. After 2 turns, unlock it.
//!
//! Active, a card in the Tavern (minion or spell), once per turn: the card leaves the shop for the
//! hand (not a buy) locked for 2 turns (`locked_turns = 2`: it can't be played or sold until the
//! start of the second turn after). It needs room in the hand.

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 61914;
pub const POWER: CardId = 61919;

/// Turns the card stays locked.
const LOCKED_TURNS: u32 = 2;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(imprison)
            .on_hero_power_usable(super::hand_has_room)
            .with_activate_target(ActivateTargetKind::ShopCard),
    )]
}

/// Using it: the shop card to the hand, locked.
fn imprison(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    let Some(t) = target.filter(|&t| t < state.shop.len()) else {
        return;
    };
    let mut card = state.shop.remove(t);
    card.locked_turns = LOCKED_TURNS;
    state.add_to_hand(card);
}
