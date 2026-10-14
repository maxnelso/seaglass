//! `Wheeled Crewmate` (`BGDUO31_207`) — Tier 3 Pirate (`6/3`), Duos only.
//! **Deathrattle:** Reduce the Cost of upgrading your team's Taverns by (`1`) (Golden
//! `BGDUO31_207_G`, `12/6`: by (`2`)).
//!
//! A gift to the whole team (`docs/duos_tavern.md` §7.2): each player's `upgrade_cost` goes down,
//! never below 0; in combat, after the combat.

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 10308;
pub const NAME: &str = "Wheeled Crewmate";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 6, 3, 3)
        .with_tribe(Tribe::Pirate)
        .on_deathrattle(on_deathrattle)
        .on_teammate_gift(teammate_gift)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    ctx.auras.give_teammate(TeammateGift::Effect {
        card_id: ID,
        amount: dying.golden_mult(),
        team: true,
    });
}

/// Each player's Tavern: upgrading costs `amount` less (never below 0).
pub fn teammate_gift(state: &mut TavernState, amount: i32, _: &mut Rng) -> Vec<Unit> {
    state.upgrade_cost = state.upgrade_cost.saturating_sub(amount.max(0) as u32);
    Vec::new()
}
