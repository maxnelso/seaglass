//! `Doting Dracthyr` (`BGDUO_107`) — Tier 3 Dragon (`4/3`), Duos only.
//! At the end of your turn, give your teammate's minions `+1` Attack (Golden `BGDUO_107_G`,
//! `8/6`: `+2` Attack).
//!
//! An `Effect` gift: the minions on the teammate's board gain it, permanently, whether or not the
//! teammate has ended their turn (`docs/duos_tavern.md` §8.4).

use crate::cards::CardTemplate;
use crate::model::{CardId, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10302;
pub const NAME: &str = "Doting Dracthyr";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 3, 3)
        .with_tribe(Tribe::Dragon)
        .on_end_of_turn(end_of_turn)
        .on_teammate_gift(teammate_gift)
}

/// End of turn: the teammate's minions gain `+1` Attack (`+2` if Golden).
pub fn end_of_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    state.auras.give_teammate(TeammateGift::Effect {
        card_id: ID,
        amount: state.board[self_idx].golden_mult(),
        team: false,
    });
}

/// The teammate's Tavern: their minions gain `amount` Attack.
pub fn teammate_gift(state: &mut TavernState, amount: i32, _: &mut Rng) -> Vec<Unit> {
    for minion in &mut state.board {
        minion.add_stats(amount, 0);
    }
    Vec::new()
}
