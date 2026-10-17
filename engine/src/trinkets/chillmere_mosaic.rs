//! `Chillmere Mosaic` (`BG35_MagicItem_755`) — Greater, 5 Gold, Dragon, Murloc.
//!
//! Spellcraft: Refresh the Tavern with Battlecry minions. They cost (1).
//!
//! Picked, and at each turn start: its Spellcraft spell, `Chillmere Mosaic` (`BG35_MagicItem_755t`),
//! a temporary token spell (`docs/trinkets.md` §11.6). Cast: a Refresh the player does
//! (`TavernState::refresh_shop_with_minions`) whose minions are Battlecry minions of the player's
//! Tier or lower from the pool, each marked by the trinket (`Unit::trinket_marks`; not the
//! minions other effects add to that Refresh). `trinket_buy_price`: a marked minion costs 1 Gold
//! (other price changes after it), as long as it stays in the Tavern (frozen too); bought, its
//! mark goes (`trinket_minion_bought`).

use crate::cards::spells::{spell, SPELLCRAFT};
use crate::cards::{self, CardHooks};
use crate::model::{CardId, TrinketState, Unit};
use crate::rng::Rng;
use crate::tavern::{BuyPrice, CardPool, TavernState};

pub const ID: CardId = 131308;
/// Its Spellcraft spell.
pub const SPELL: CardId = 131310;

/// What its minions cost.
const PRICE: u32 = 1;

/// The trinket's and its spell's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            ID,
            CardHooks::EMPTY
                .on_trinket_bought(give_spell)
                .on_trinket_turn_start(give_spell)
                .on_trinket_buy_price(price)
                .on_trinket_minion_bought(minion_bought),
        ),
        (SPELL, spell(cast).with_flags(SPELLCRAFT)),
    ]
}

fn give_spell(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.add_to_hand_or_wait(super::spellcraft_spell(SPELL, "Chillmere Mosaic"));
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    state.refresh_shop_with_minions(
        cards::is_battlecry_minion,
        |unit| unit.trinket_marks.push(ID),
        pool,
        rng,
    );
}

fn price(_: &TrinketState, _: &TavernState, card: &Unit, price: BuyPrice) -> BuyPrice {
    if card.is_spell || !card.trinket_marks.contains(&ID) {
        return price;
    }
    BuyPrice {
        cost: PRICE,
        ..price
    }
}

fn minion_bought(_: &mut TavernState, bought: &mut Unit) {
    bought.trinket_marks.retain(|&m| m != ID);
}
