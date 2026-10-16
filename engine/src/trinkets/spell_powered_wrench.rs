//! `Spell-powered Wrench` (`BG32_MagicItem_170`) — Lesser, 0 Gold, Mech.
//!
//! After you play a Magnetic minion, cast Repair Job on a random friendly Mech.
//!
//! `trinket_after_play`: after each Magnetic minion played from the hand (Magnetized or placed),
//! `Repair Job` is cast by an effect (`spells::cast_spell_by_effect`: a Tavern spell cast, which
//! the spell-cast observers see) on a random Mech on the board (one `rng.below(k)`). Nothing
//! without a Mech.

use crate::cards::{self, CardHooks, Played};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 120864;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_play(after_play))]
}

fn after_play(state: &mut TavernState, played: &Played, pool: &mut CardPool, rng: &mut Rng) {
    let magnetic = cards::template(played.card_id).is_some_and(|t| t.instantiate().magnetic);
    if !played.from_hand || !magnetic {
        return;
    }
    let mechs: Vec<usize> = (0..state.board.len())
        .filter(|&i| state.board[i].tribe.matches(Tribe::Mech))
        .collect();
    if !mechs.is_empty() {
        let pos = mechs[rng.below(mechs.len())];
        super::cast_repair_job(state, pos, pool, rng);
    }
}
