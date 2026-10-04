//! `Holy Vanguard` (`BG36_372`) — Tier 4 Neutral (`10/10`, Divine Shield).
//!
//! Divine Shield. Has `+30/+30` (`+60/+60` if Golden) if you have 15 or less Health.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, PlayerAuras, Unit};

pub const ID: CardId = 430;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Holy Vanguard", 10, 10, 4).with_keyword(Keyword::DivineShield)
}

pub fn sync_unit(unit: &mut Unit, auras: &PlayerAuras) {
    if unit.card_id != ID {
        return;
    }
    let mult = if unit.is_golden { 2 } else { 1 };
    let target = if auras.hero_low_health {
        (30 * mult, 30 * mult)
    } else {
        (0, 0)
    };
    let (app_atk, app_hp) = unit.holy_vanguard_buff_applied;
    let d_atk = target.0 - app_atk;
    let d_hp = target.1 - app_hp;
    if d_atk != 0 || d_hp != 0 {
        unit.holy_vanguard_buff_applied = target;
        unit.add_stats(d_atk, d_hp);
    }
}
