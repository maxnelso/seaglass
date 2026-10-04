//! `Deft Deserter` (`BG36_621`) — Tier 5 Demon (`8/8`).
//!
//! Activate (1): Give all minions in the Tavern `+8/+8` (`+16/+16` if Golden) and Taunt, Divine Shield, or Windfury.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 509;
pub const ACTIVATE_COST: u32 = 1;

const DESERTER_KEYWORDS: [Keyword; 3] = [Keyword::Taunt, Keyword::DivineShield, Keyword::Windfury];

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Deft Deserter", 8, 8, 5)
        .with_tribe(Tribe::Demon)
        .with_activate_cost(ACTIVATE_COST)
}

pub fn on_activate(state: &mut TavernState, source_pos: usize, rng: &mut Rng) {
    let buff = if state.board[source_pos].is_golden { 16 } else { 8 };
    for u in &mut state.shop {
        if !u.is_spell {
            u.add_stats(buff, buff);
            let missing: Vec<Keyword> = DESERTER_KEYWORDS
                .iter()
                .copied()
                .filter(|&kw| !u.has_keyword(kw))
                .collect();
            let kw = if !missing.is_empty() {
                if missing.len() == 1 {
                    missing[0]
                } else {
                    missing[rng.below(missing.len())]
                }
            } else {
                DESERTER_KEYWORDS[rng.below(DESERTER_KEYWORDS.len())]
            };
            u.apply_keyword(kw, false);
        }
    }
}
