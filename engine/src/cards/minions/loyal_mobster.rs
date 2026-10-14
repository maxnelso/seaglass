//! `Loyal Mobster` (`BGDUO31_202`) — Tier 6 Quilboar (`6/5`), Duos only.
//! At the end of your turn, this plays a Blood Gem on all your teammate's minions (Golden
//! `BGDUO31_202_G`, `12/10`: 2 Blood Gems).
//!
//! An `Effect` gift: played Blood Gems (`docs/tavern.md` §7.4: not spells cast) on the minions on
//! the teammate's board, of its own player's Blood Gem size. Its card counter holds that size,
//! which the teammate's Tavern reads in `teammate_counters` (`docs/duos_tavern.md` §7.1, §8.4).

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10602;
pub const NAME: &str = "Loyal Mobster";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 6, 5, 6)
        .with_tribe(Tribe::Quilboar)
        .on_end_of_turn(end_of_turn)
        .on_teammate_gift(teammate_gift)
}

/// End of turn: record its player's Blood Gem size, then play a Blood Gem (2 if Golden) on the
/// teammate's minions.
pub fn end_of_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.blood_gem_stats();
    let (held_atk, held_hp) = state.auras.counter_pair(ID);
    state
        .auras
        .add_counter_pair(ID, (atk - held_atk, hp - held_hp));
    state.auras.give_teammate(TeammateGift::Effect {
        card_id: ID,
        amount: state.board[self_idx].golden_mult(),
        team: false,
    });
}

/// The teammate's Tavern: `amount` Blood Gems, of the giver's size, played on each minion.
pub fn teammate_gift(state: &mut TavernState, amount: i32, rng: &mut Rng) -> Vec<Unit> {
    let (atk, hp) = state
        .auras
        .teammate_counters
        .get(&ID)
        .copied()
        .unwrap_or((1, 1));
    let mut gem_auras = state.auras.clone();
    gem_auras.blood_gem_bonus_atk = atk - 1;
    gem_auras.blood_gem_bonus_hp = hp - 1;
    for minion in &mut state.board {
        minion.play_blood_gems(amount.max(0) as u32, &gem_auras);
    }
    cards::resolve_pending_effects(&mut state.board, &state.auras, rng);
    Vec::new()
}
