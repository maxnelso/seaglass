//! `Plunder Pal` (`BGDUO_118`) — Tier 3 Pirate (`2/2`), Duos only.
//! At the start of your turn, you and your teammate each gain `1` Gold (Golden `BGDUO_118_G`,
//! `4/4`: `2` Gold).
//!
//! A `turn_start` observer: its player gains the Gold at once; the teammate's is an `Effect`
//! gift, delivered after both players' start of turn, so their own Gold for the turn doesn't
//! replace it (`docs/duos_tavern.md` §8.4).

use crate::cards::CardTemplate;
use crate::model::{CardId, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10305;
pub const NAME: &str = "Plunder Pal";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 2, 3)
        .with_tribe(Tribe::Pirate)
        .on_turn_start(turn_start)
        .on_teammate_gift(teammate_gift)
}

/// Start of turn: its player and their teammate each gain 1 Gold (2 if Golden).
pub fn turn_start(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let gold = state.board[self_idx].golden_mult();
    state.gold += gold as u32;
    state.auras.give_teammate(TeammateGift::Effect {
        card_id: ID,
        amount: gold,
        team: false,
    });
}

/// The teammate's Tavern: gain `amount` Gold.
pub fn teammate_gift(state: &mut TavernState, amount: i32, _: &mut Rng) -> Vec<Unit> {
    state.gold += amount.max(0) as u32;
    Vec::new()
}
