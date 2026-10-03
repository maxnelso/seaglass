//! `Mind Muck` (`BG23_357`) — Tier 2 Demon (`3/2`).
//! **Battlecry:** Choose a friendly Demon. It consumes a minion in the Tavern to gain its (`double its` if Golden) stats.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 219;
pub const NAME: &str = "Mind Muck";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 2, 2).with_tribe(Tribe::Demon)
}

pub fn on_battlecry(
    state: &mut TavernState,
    unit: &mut Unit,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let shop_minions: Vec<usize> = state
        .shop
        .iter()
        .enumerate()
        .filter(|(_, u)| !u.is_spell)
        .map(|(i, _)| i)
        .collect();
    if shop_minions.is_empty() {
        return;
    }
    let shop_pick = if shop_minions.len() == 1 {
        shop_minions[0]
    } else {
        shop_minions[rng.below(shop_minions.len())]
    };
    let consumed = state.shop.remove(shop_pick);
    pool.return_unit(&consumed);

    let mult = if unit.is_golden { 2 } else { 1 };
    let gain_atk = consumed.attack * mult;
    let gain_hp = consumed.health * mult;

    // Pick the leftmost friendly Demon on board; if none on board, Mind Muck itself is a Demon.
    if let Some(demon) = state
        .board
        .iter_mut()
        .find(|u| u.tribe.matches(Tribe::Demon))
    {
        demon.add_stats(gain_atk, gain_hp);
    } else {
        unit.add_stats(gain_atk, gain_hp);
    }
}
