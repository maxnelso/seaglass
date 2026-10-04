//! `Very Hungry Winterfinner` (`BG29_300`) — Tier 2 Murloc (`2/6`).
//! **Taunt**. Whenever this takes damage, give a random minion in your hand `+2/+1` (`+4/+2` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe, Unit};
use crate::rng::Rng;

pub const ID: CardId = 233;
pub const NAME: &str = "Very Hungry Winterfinner";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 6, 2)
        .with_tribe(Tribe::Murloc)
        .with_keyword(Keyword::Taunt)
        .on_damage_taken(on_damage_taken)
}

pub fn on_damage_taken(unit: &Unit, hand: &mut [Unit], rng: &mut Rng) {
    let candidates: Vec<usize> = hand
        .iter()
        .enumerate()
        .filter(|(_, u)| !u.is_spell && !u.unplayable && u.lockbox_turns_left == 0)
        .map(|(i, _)| i)
        .collect();
    if candidates.is_empty() {
        return;
    }
    let idx = if candidates.len() == 1 {
        candidates[0]
    } else {
        candidates[rng.below(candidates.len())]
    };
    let mult = unit.golden_mult();
    hand[idx].add_stats(2 * mult, mult);
}
