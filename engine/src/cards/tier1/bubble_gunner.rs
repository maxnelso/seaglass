//! `Bubble Gunner` (`BG31_149`) — Tier 1 Murloc (`2/3`).
//! **Battlecry:** Gain a random **Bonus Keyword** (`2` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe, Unit, BONUS_KEYWORDS};
use crate::rng::Rng;

pub const ID: CardId = 112;
pub const NAME: &str = "Bubble Gunner";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 3, 1)
        .with_tribe(Tribe::Murloc)
        .on_battlecry(|_, unit, _, _, rng| on_battlecry(unit, rng))
}

pub fn on_battlecry(unit: &mut Unit, rng: &mut Rng) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let missing: Vec<Keyword> = BONUS_KEYWORDS
            .iter()
            .copied()
            .filter(|&kw| !unit.has_keyword(kw))
            .collect();
        if missing.is_empty() {
            break;
        }
        let idx = rng.below(missing.len());
        unit.apply_keyword(missing[idx], false);
    }
}
