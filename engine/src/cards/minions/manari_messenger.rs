//! `Man'ari Messenger` (`BGDUO_121`) — Tier 5 Demon (`9/6`), Duos only.
//! **Battlecry:** Minions in your team's Taverns have `+3/+2` this game (Golden `BGDUO_121_G`,
//! `18/12`: `+6/+4`).
//!
//! In both players' Taverns, as `Staff of Enrichment` (without spell bonuses): the minions in the
//! Tavern now and those it offers later (`tavern_all_atk` / `tavern_all_hp`). Its player's at
//! once, the teammate's through an `Effect` gift (`docs/duos_tavern.md` §8.4).

use crate::cards::CardTemplate;
use crate::model::{CardId, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 10502;
pub const NAME: &str = "Man'ari Messenger";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 9, 6, 5)
        .with_tribe(Tribe::Demon)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
        .on_teammate_gift(teammate_gift)
}

/// Both players' Taverns: `+3/+2` (`+6/+4` if Golden).
pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let mult = unit.golden_mult();
    buff_tavern(state, mult);
    state.auras.give_teammate(TeammateGift::Effect {
        card_id: ID,
        amount: mult,
        team: false,
    });
}

/// The teammate's Tavern: `+3/+2` times `amount`.
pub fn teammate_gift(state: &mut TavernState, amount: i32, _: &mut Rng) -> Vec<Unit> {
    buff_tavern(state, amount);
    Vec::new()
}

/// Minions in the Tavern have `+3/+2` times `mult` this game, those in it now too.
fn buff_tavern(state: &mut TavernState, mult: i32) {
    let (atk, hp) = (3 * mult, 2 * mult);
    state.auras.tavern_all_atk += atk;
    state.auras.tavern_all_hp += hp;
    for minion in state.shop.iter_mut().filter(|u| !u.is_spell) {
        minion.add_stats(atk, hp);
    }
}
