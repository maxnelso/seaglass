//! `Curse of Flesh` (`TB_BaconShop_HERO_35_Buddy_t3`) — Wheel of Yogg-Saron, 19%.
//!
//! A random friendly minion gains the stats of another friendly minion.
//!
//! A random board minion (`rng.below(n)`) gains the current Attack and Health of another random
//! board minion (`rng.below(n - 1)` among the others, none for one), added on top (confirmed); the
//! receiver is drawn first (engine decision). Nothing with fewer than 2 minions (confirmed;
//! `docs/wheel_of_yogg.md` §4).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 80435;

/// The outcome's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, rng: &mut Rng) {
    let n = state.board.len();
    if n < 2 {
        return;
    }
    let receiver = rng.below(n);
    let others: Vec<usize> = (0..n).filter(|&i| i != receiver).collect();
    let giver = crate::trinkets::pick_tied(&others, rng);
    let (attack, health) = (state.board[giver].attack, state.board[giver].health);
    state.board[receiver].add_stats(attack, health);
}
