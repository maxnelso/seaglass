//! `Dark Dazzler` (`BGDUO33_150`) — Tier 6 Demon (`4/7`), Duos only.
//! After your teammate sells a minion, gain its stats. (Once per turn.) (Golden `BGDUO33_150_G`,
//! `8/14`: twice per turn.)
//!
//! An `after_teammate_sell` observer (`docs/duos_tavern.md` §7.3): it gains the sold minion's
//! current Attack and Health, permanently; its `charges` are its uses left this turn (1, or 2 if
//! Golden; `+1` when made Golden), `docs/duos_tavern.md` §8.4.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10601;
pub const NAME: &str = "Dark Dazzler";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 7, 6)
        .with_tribe(Tribe::Demon)
        .on_reset_turn_charges(reset_turn_charges)
        .on_made_golden(|dazzler| dazzler.charges += 1)
        .on_after_teammate_sell(after_teammate_sell)
}

/// Its uses this turn: 1, or 2 if Golden.
pub fn reset_turn_charges(dazzler: &mut Unit) {
    dazzler.charges = dazzler.golden_mult() as u32;
}

/// The teammate sold `sold`: gain its stats, if a use is left this turn.
pub fn after_teammate_sell(
    state: &mut TavernState,
    self_idx: usize,
    sold: &Unit,
    _: &mut CardPool,
    _: &mut Rng,
) {
    let dazzler = &mut state.board[self_idx];
    if dazzler.charges == 0 {
        return;
    }
    dazzler.charges -= 1;
    dazzler.add_stats(sold.attack, sold.health);
}
