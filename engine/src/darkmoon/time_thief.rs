//! `Time Thief` (`BGS_Treasures_010`) — Tier 2 Darkmoon Prize.
//!
//! Discover a minion from your last opponent's warband.
//!
//! A Discover of up to 3 different minions of the last opponent's last warband
//! (`TavernState::last_opponent_warband`), as plain copies (`cards::instantiate_plain_copy`) that
//! aren't from the pool (`Unit::not_from_pool`): one `rng.below(k)` each among the warband's
//! minions whose card isn't offered yet (none for the last one) (engine decision, as Scabbs
//! Cutterbutter's I Spy). Without a last opponent or warband, nothing.

use crate::cards::spells::spell;
use crate::cards::{self, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65248;

/// Options in the Discover.
const OPTIONS: usize = 3;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, rng: &mut Rng) {
    let mut candidates: Vec<Unit> = state.last_opponent_warband().to_vec();
    let mut options: Vec<Unit> = Vec::with_capacity(OPTIONS);
    while options.len() < OPTIONS && !candidates.is_empty() {
        let pick = match candidates.len() {
            1 => 0,
            k => rng.below(k),
        };
        let minion = candidates.remove(pick);
        candidates.retain(|u| u.card_id != minion.card_id);
        let mut option = cards::instantiate_plain_copy(&minion);
        option.not_from_pool = true;
        state.apply_global_unit_auras(&mut option);
        options.push(option);
    }
    state.push_discover(options);
}
