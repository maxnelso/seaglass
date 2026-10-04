//! `Persistent Poet` (`BG29_813`) — Tier 4 Dragon (`2/3`, Divine Shield).
//!
//! Divine Shield. Adjacent Dragons permanently keep Bonus Keywords and (`double` if Golden) stats gained in combat.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe, Unit, BONUS_KEYWORDS};

pub const ID: CardId = 445;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Persistent Poet", 2, 3, 4)
        .with_tribe(Tribe::Dragon)
        .with_keyword(Keyword::DivineShield)
}

pub fn on_post_combat_adjacent_dragon(
    pre_board: &[Unit],
    idx: usize,
    post_units: &[Unit],
    tavern_unit: &mut Unit,
) {
    let Some(pre) = pre_board.get(idx) else {
        return;
    };
    if !pre.tribe.matches(Tribe::Dragon) {
        return;
    }
    let mut poet_mult = 0i32;
    if idx > 0 && pre_board[idx - 1].card_id == ID {
        poet_mult = poet_mult.max(if pre_board[idx - 1].is_golden { 2 } else { 1 });
    }
    if idx + 1 < pre_board.len() && pre_board[idx + 1].card_id == ID {
        poet_mult = poet_mult.max(if pre_board[idx + 1].is_golden { 2 } else { 1 });
    }
    if poet_mult == 0 {
        return;
    }
    let Some(post) = post_units.iter().find(|u| u.id == pre.id) else {
        return;
    };
    let atk_gain = (post.max_attack - pre.attack).max(0) * poet_mult;
    let hp_gain = (post.max_health - pre.health).max(0) * poet_mult;
    if atk_gain > 0 || hp_gain > 0 {
        tavern_unit.add_stats(atk_gain, hp_gain);
    }
    for kw in BONUS_KEYWORDS {
        if post.has_keyword(kw) {
            tavern_unit.apply_keyword(kw, false);
        }
    }
}
