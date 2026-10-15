//! `Shudderwock` (`TB_BaconShop_HERO_23`) — Armor 10 (Duos 8).
//!
//! Hero Power: `Snicker-snack` (`TB_BaconShop_HP_022`), 0 Gold: Trigger a friendly minion's Battlecry. (Unlocks on Turn 3.)
//!
//! Active, a friendly minion with a Battlecry, once per turn, from the 3rd turn on (`turn >= 3`):
//! its Battlecry triggers as `Young Murk-Eye`'s do (`cards::trigger_board_battlecry`).

use crate::cards::{self, ActivateTargetKind, CardHooks};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 58027;
pub const POWER: CardId = 58028;

/// The turn it unlocks on.
const UNLOCK_TURN: u32 = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(snicker_snack)
            .on_hero_power_usable(|state| state.turn >= UNLOCK_TURN)
            .on_hero_power_target(|state, t| cards::is_battlecry_minion(state.board[t].card_id))
            .with_activate_target(ActivateTargetKind::BoardAny),
    )]
}

/// Using it: the minion's Battlecry triggers.
fn snicker_snack(
    state: &mut TavernState,
    target: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if let Some(t) = target {
        cards::trigger_board_battlecry(state, t, pool, rng);
    }
}
