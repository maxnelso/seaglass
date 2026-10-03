//! `Tarecgosa` (`BG21_015`) — Tier 2 Dragon (`4/4`).
//! Permanently keeps Bonus Keywords and (`double` if Golden) stats gained in combat.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe, Unit, BONUS_KEYWORDS};

pub const ID: CardId = 231;
pub const NAME: &str = "Tarecgosa";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 4, 2).with_tribe(Tribe::Dragon)
}

pub fn apply_post_combat_persistence(
    pre_combat: &Unit,
    post_combat_board: &[Unit],
    tavern_unit: &mut Unit,
) {
    if tavern_unit.card_id != ID {
        return;
    }
    let Some(survivor) = post_combat_board.iter().find(|u| u.id == pre_combat.id) else {
        return;
    };
    let mult = if tavern_unit.is_golden { 2 } else { 1 };
    let gained_atk = (survivor.attack - pre_combat.attack).max(0) * mult;
    let gained_hp = (survivor.max_health - pre_combat.max_health).max(0) * mult;
    if gained_atk > 0 || gained_hp > 0 {
        tavern_unit.add_stats(gained_atk, gained_hp);
    }
    for kw in BONUS_KEYWORDS {
        if survivor.has_keyword(kw) {
            tavern_unit.apply_keyword(kw, kw == Keyword::DivineShield);
        }
    }
}
