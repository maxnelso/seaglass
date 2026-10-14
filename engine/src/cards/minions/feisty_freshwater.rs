//! `Feisty Freshwater` (`BGDUO_110`) — Tier 4 Elemental (`6/4`), Duos only.
//! **Deathrattle:** You and your teammate each gain two free **Refreshes** (Golden `BGDUO_110_G`,
//! `12/8`: four).
//!
//! The player's free Refreshes come at once (in combat, back with the auras); the teammate's are
//! a gift (`docs/duos_tavern.md` §7.2), which in combat reaches their warband's auras at once and
//! comes back with them (`docs/duos.md` §2).

use crate::cards::{CardTemplate, DeathrattleContext, TeammateWarband};
use crate::model::{CardId, PlayerAuras, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 10401;
pub const NAME: &str = "Feisty Freshwater";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 6, 4, 4)
        .with_tribe(Tribe::Elemental)
        .on_deathrattle(on_deathrattle)
        .on_teammate_gift(teammate_gift)
        .on_teammate_combat_gift(teammate_combat_gift)
}

/// The player whose `auras` these are gains `refreshes` free Refreshes.
fn gain_refreshes(auras: &mut PlayerAuras, refreshes: i32) {
    auras.free_refreshes += refreshes.max(0) as u32;
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let refreshes = 2 * dying.golden_mult();
    gain_refreshes(ctx.auras, refreshes);
    ctx.auras.give_teammate(TeammateGift::Effect {
        card_id: ID,
        amount: refreshes,
        team: false,
    });
}

/// The teammate's Tavern: `amount` free Refreshes.
pub fn teammate_gift(state: &mut TavernState, amount: i32, _: &mut Rng) -> Vec<Unit> {
    gain_refreshes(&mut state.auras, amount);
    Vec::new()
}

/// The teammate's warband in combat: `amount` free Refreshes, back with its auras.
pub fn teammate_combat_gift(warband: &mut TeammateWarband<'_>, amount: i32, _: &mut Rng) {
    gain_refreshes(warband.auras, amount);
}
