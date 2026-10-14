//! `Support System` (`BGDUO_109`) — Tier 5 Mech (`4/5`), Duos only.
//! At the end of your turn, give a minion in your teammate's warband Divine Shield (Golden
//! `BGDUO_109_G`, `8/10`: 2 minions).
//!
//! An `Effect` gift: random different minions on the teammate's board that don't have Divine
//! Shield get it, permanently; fewer if fewer lack it (`docs/duos_tavern.md` §8.4).

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10505;
pub const NAME: &str = "Support System";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 5, 5)
        .with_tribe(Tribe::Mech)
        .on_end_of_turn(end_of_turn)
        .on_teammate_gift(teammate_gift)
}

/// End of turn: a minion of the teammate's (2 if Golden) gets Divine Shield.
pub fn end_of_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    state.auras.give_teammate(TeammateGift::Effect {
        card_id: ID,
        amount: state.board[self_idx].golden_mult(),
        team: false,
    });
}

/// The teammate's Tavern: `amount` random minions without Divine Shield get it.
pub fn teammate_gift(state: &mut TavernState, amount: i32, rng: &mut Rng) -> Vec<Unit> {
    let mut candidates: Vec<usize> = (0..state.board.len())
        .filter(|&i| !state.board[i].divine_shield)
        .collect();
    for _ in 0..amount.max(0) {
        let pick = match candidates.len() {
            0 => break,
            1 => 0,
            n => rng.below(n),
        };
        let idx = candidates.remove(pick);
        state.board[idx].apply_keyword(Keyword::DivineShield, false);
    }
    Vec::new()
}
