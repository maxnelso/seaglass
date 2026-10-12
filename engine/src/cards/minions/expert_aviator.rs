//! `Expert Aviator` (`BG34_140`) — Tier 2 Murloc (`3/5`).
//! **Rally:** Summon the (`2` if Golden) highest-Attack Murloc(s) from your hand for this combat only.
//! A tie for the highest Attack is broken at random.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;

pub const ID: CardId = 210;
pub const NAME: &str = "Expert Aviator";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 5, 2)
        .with_tribe(Tribe::Murloc)
        .on_rally(|c| on_rally(&c.board[c.attacker_pos], c.hand, c.hand_summoned, c.rng))
}

pub fn on_rally(
    attacker: &Unit,
    hand: &[Unit],
    hand_summoned: &mut [bool],
    rng: &mut Rng,
) -> Vec<Unit> {
    let count = if attacker.is_golden { 2 } else { 1 };
    let mut summons = Vec::with_capacity(count);
    for _ in 0..count {
        let murlocs: Vec<usize> = hand
            .iter()
            .enumerate()
            .filter(|(idx, u)| {
                !u.is_spell
                    && u.tribe.matches(Tribe::Murloc)
                    && !hand_summoned.get(*idx).copied().unwrap_or(true)
            })
            .map(|(idx, _)| idx)
            .collect();
        let Some(best_attack) = murlocs.iter().map(|&i| hand[i].attack).max() else {
            break;
        };
        let tied: Vec<usize> = murlocs
            .into_iter()
            .filter(|&i| hand[i].attack == best_attack)
            .collect();
        let h_idx = if tied.len() == 1 {
            tied[0]
        } else {
            tied[rng.below(tied.len())]
        };
        if let Some(slot) = hand_summoned.get_mut(h_idx) {
            *slot = true;
        }
        summons.push(hand[h_idx].clone());
    }
    summons
}
