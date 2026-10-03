//! `Expert Aviator` (`BG34_140`) — Tier 2 Murloc (`3/5`).
//! **Rally:** Summon the (`2` if Golden) highest-Attack Murloc(s) from your hand for this combat only.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 210;
pub const NAME: &str = "Expert Aviator";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 5, 2).with_tribe(Tribe::Murloc)
}

pub fn on_rally(
    attacker: &Unit,
    hand: &[Unit],
    hand_summoned: &mut [bool],
) -> Vec<Unit> {
    let count = if attacker.is_golden { 2 } else { 1 };
    let mut summons = Vec::with_capacity(count);
    for _ in 0..count {
        let best = hand
            .iter()
            .enumerate()
            .filter(|(idx, u)| {
                !u.is_spell
                    && u.tribe.matches(Tribe::Murloc)
                    && !hand_summoned.get(*idx).copied().unwrap_or(true)
            })
            .max_by_key(|(idx, u)| (u.attack, std::cmp::Reverse(*idx)))
            .map(|(idx, _)| idx);

        let Some(h_idx) = best else {
            break;
        };
        if let Some(slot) = hand_summoned.get_mut(h_idx) {
            *slot = true;
        }
        summons.push(hand[h_idx].clone());
    }
    summons
}
