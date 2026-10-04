//! `Sewer Escapee` (`BG36_700`) — Tier 5 Murloc (`7/7`).
//!
//! Activate (1): Give another Murloc `+7/+7` (`+14/+14` if Golden) and a random Bonus Keyword.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, BONUS_KEYWORDS};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 544;
pub const ACTIVATE_COST: u32 = 1;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Sewer Escapee", 7, 7, 5)
        .with_tribe(Tribe::Murloc)
        .with_activate_cost(ACTIVATE_COST)
}

pub fn on_activate(
    state: &mut TavernState,
    source_pos: usize,
    target_pos: Option<usize>,
    rng: &mut Rng,
) {
    let Some(t_pos) = target_pos else {
        return;
    };
    if t_pos >= state.board.len() || t_pos == source_pos {
        return;
    }
    let buff = if state.board[source_pos].is_golden { 14 } else { 7 };
    let target = &mut state.board[t_pos];
    target.add_stats(buff, buff);
    let missing: Vec<_> = BONUS_KEYWORDS
        .iter()
        .copied()
        .filter(|&kw| !target.has_keyword(kw))
        .collect();
    let kw = if !missing.is_empty() {
        if missing.len() == 1 {
            missing[0]
        } else {
            missing[rng.below(missing.len())]
        }
    } else {
        BONUS_KEYWORDS[rng.below(BONUS_KEYWORDS.len())]
    };
    target.apply_keyword(kw, false);
}
