//! `Hired Mount` (`BG36_240`) — Tier 3 Dragon (`3/5`).
//! **Activate (2):** Get a (`2` if Golden) random **Chromadrake**(s).

use crate::cards::tokens::{
    make_chromadrake, CHROMADRAKE_IDS, TOKEN_BLACK_CHROMADRAKE, TOKEN_BLUE_CHROMADRAKE,
    TOKEN_BRONZE_CHROMADRAKE, TOKEN_GREEN_CHROMADRAKE, TOKEN_RED_CHROMADRAKE,
};
use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 320;
pub const NAME: &str = "Hired Mount";
pub const ACTIVATE_COST: u32 = 2;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 5, 3)
        .with_tribe(Tribe::Dragon)
        .with_activate_cost(ACTIVATE_COST)
        .on_activate(|state, source_pos, _, _, rng| on_activate(state, source_pos, rng))
}

pub fn on_activate(state: &mut TavernState, source_pos: usize, rng: &mut Rng) {
    let count = if state.board[source_pos].is_golden {
        2
    } else {
        1
    };
    for _ in 0..count {
        if state.hand.len() < 10 {
            let cid = CHROMADRAKE_IDS[rng.below(CHROMADRAKE_IDS.len())];
            let mut drake = make_chromadrake(cid, false);
            state.apply_global_unit_auras(&mut drake);
            state.add_to_hand(drake);
        }
    }
}

/// Resolve the Battlecry of one of the 5 `Chromadrake` tokens when played from hand.
pub fn on_chromadrake_battlecry(state: &mut TavernState, unit: &Unit, rng: &mut Rng) {
    let mult = if unit.is_golden { 2 } else { 1 };
    match unit.card_id {
        TOKEN_BLUE_CHROMADRAKE => {
            for _ in 0..mult {
                if state.hand.len() < 10 {
                    let spell = spells::draw_random_cost_tavern_spell(2, rng);
                    state.add_to_hand(spell);
                }
            }
        }
        TOKEN_BLACK_CHROMADRAKE => {
            state.auras.spell_bonus_hp += mult;
        }
        TOKEN_GREEN_CHROMADRAKE => {
            for u in &mut state.board {
                if u.tribe.matches(Tribe::Dragon) {
                    u.add_stats(mult, 3 * mult);
                }
            }
        }
        TOKEN_BRONZE_CHROMADRAKE => {
            for u in &mut state.board {
                if u.tribe.matches(Tribe::Dragon) {
                    u.add_stats(3 * mult, mult);
                }
            }
        }
        TOKEN_RED_CHROMADRAKE => {
            state.auras.spell_bonus_atk += mult;
        }
        _ => {}
    }
}
