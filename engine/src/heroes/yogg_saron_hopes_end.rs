//! `Yogg-Saron, Hope's End` (`TB_BaconShop_HERO_35`) — Armor 10 (Duos 7).
//!
//! Hero Power: `Puzzle Box` (`TB_BaconShop_HP_039t`), 0 Gold: At the start of your turn, cast a random Tavern spell. (Unlocks on Turn 3.)
//!
//! Passive. At the start of each turn from the 3rd on (`hero_turn_start`), a Tavern spell drawn as
//! the shop draws one is cast (`spells::cast_spell_by_effect`); a targeted one on a random valid
//! target (a friendly minion, or a minion in the Tavern for a spell that only targets those; one
//! `rng.below(k)`), and with none it is not cast (engine decision). The cast is
//! `spells::cast_random_tavern_spell`, which the Wheel of Yogg-Saron's `Mindflayer Goggles` shares.

use crate::cards::{spells, CardHooks};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 59805;
pub const POWER: CardId = 122958;

/// The turn it unlocks on.
const UNLOCK_TURN: u32 = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_turn_start(puzzle_box))]
}

/// Start of turn: cast a random Tavern spell.
fn puzzle_box(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if state.turn < UNLOCK_TURN {
        return;
    }
    spells::cast_random_tavern_spell(state, pool, rng);
}
