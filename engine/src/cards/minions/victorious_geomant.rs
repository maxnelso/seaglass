//! `Victorious Geomant` (`BG36_370`) — Tier 6 Quilboar (`10/10`).
//!
//! Activate (2): Get 6 (`12` if Golden) Blood Gems. Cast any that don't fit on your left-most minion.
//!
//! The Blood Gems that don't fit are cast one at a time, each on the left-most minion at that
//! moment: each is a spell you cast (`Felboar` counts it, `Geomagus Roogug` triggers on it,
//! `Balinda Stonehearth` casts it twice), but not one played from your hand, so `Hot-Air Surveyor`
//! doesn't cast it again (`docs/tavern.md` §7.4).

use crate::cards::{spells, tokens, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

pub const ID: CardId = 631;
pub const ACTIVATE_COST: u32 = 2;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Victorious Geomant", 10, 10, 6)
        .with_tribe(Tribe::Quilboar)
        .with_activate_cost(ACTIVATE_COST)
        .on_activate(|state, source_pos, _, pool, rng| on_activate(state, source_pos, pool, rng))
}

pub fn on_activate(state: &mut TavernState, source_pos: usize, pool: &mut CardPool, rng: &mut Rng) {
    if source_pos >= state.board.len() {
        return;
    }
    let count = if state.board[source_pos].is_golden {
        12
    } else {
        6
    };
    let mut overflow = 0u32;
    for _ in 0..count {
        if state.hand.len() < 10 {
            state.add_to_hand(tokens::make_blood_gem());
        } else {
            overflow += 1;
        }
    }
    for _ in 0..overflow {
        if state.board.is_empty() {
            break;
        }
        let gem = tokens::make_blood_gem();
        spells::cast_spell_by_effect(state, gem, MinionSlot::Board(0), pool, rng);
    }
}
