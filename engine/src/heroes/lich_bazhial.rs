//! `Lich Baz'hial` (`TB_BaconShop_HERO_25`) — Armor 18 (Duos 15).
//!
//! Hero Power: `Graveyard Shift` (`TB_BaconShop_HP_049`), 2 Gold: Steal a card from the Tavern. Take 2 damage.
//!
//! Active, a card in the Tavern (minion or spell), once per turn: the card leaves the shop for the
//! hand (not a buy: no buy triggers), then the hero takes 2 damage (`deal_hero_damage`: Armor
//! first). It needs room in the hand, and can't be used if the damage would bring Health and Armor
//! to 0 (engine decision). A card the hand has no room for by then (a Gold-spent effect filled it)
//! stays in the Tavern (engine decision).

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 58044;
pub const POWER: CardId = 60285;

/// The damage it deals the hero.
const DAMAGE: i32 = 2;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(graveyard_shift)
            .on_hero_power_usable(usable)
            .with_activate_target(ActivateTargetKind::ShopCard),
    )]
}

/// Room in the hand, and the damage wouldn't kill the hero.
fn usable(state: &TavernState) -> bool {
    super::hand_has_room(state) && state.health + state.armor.max(0) > DAMAGE
}

/// Using it: the shop card to the hand, then 2 damage.
fn graveyard_shift(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    if let Some(t) = target.filter(|&t| t < state.shop.len() && super::hand_has_room(state)) {
        let card = state.shop.remove(t);
        state.add_to_hand(card);
    }
    state.deal_hero_damage(DAMAGE);
}
