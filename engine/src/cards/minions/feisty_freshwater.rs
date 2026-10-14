//! `Feisty Freshwater` (`BGDUO_110`) — Tier 4 Elemental (`6/4`), Duos only.
//! **Deathrattle:** You and your teammate each gain two free **Refreshes** (Golden `BGDUO_110_G`,
//! `12/8`: four).
//!
//! The player's free Refreshes come at once (in combat, back with the auras); the teammate's are
//! a gift (`docs/duos_tavern.md` §7.2).

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 10401;
pub const NAME: &str = "Feisty Freshwater";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 6, 4, 4)
        .with_tribe(Tribe::Elemental)
        .on_deathrattle(on_deathrattle)
        .on_teammate_gift(teammate_gift)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let refreshes = 2 * dying.golden_mult();
    ctx.auras.free_refreshes += refreshes as u32;
    ctx.auras.give_teammate(TeammateGift::Effect {
        card_id: ID,
        amount: refreshes,
        team: false,
    });
}

/// The teammate's Tavern: `amount` free Refreshes.
pub fn teammate_gift(state: &mut TavernState, amount: i32, _: &mut Rng) -> Vec<Unit> {
    state.auras.free_refreshes += amount.max(0) as u32;
    Vec::new()
}
