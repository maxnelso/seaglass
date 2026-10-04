//! `Razorfen Vineweaver` (`BG33_883`) — Tier 5 Quilboar (`4/4`).
//!
//! Rally: This plays 4 (`8` if Golden) permanent Blood Gems on itself.

use crate::cards::CardTemplate;
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 540;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Razorfen Vineweaver", 4, 4, 5).with_tribe(Tribe::Quilboar)
}

pub fn on_rally(attacker: &mut Unit, auras: &PlayerAuras, in_combat: bool) {
    let gems = if attacker.is_golden { 8 } else { 4 };
    let (g_atk, g_hp) = auras.blood_gem_stats();
    attacker.play_blood_gems(gems, auras);
    if in_combat {
        attacker.perm_atk_gained += g_atk * (gems as i32);
        attacker.perm_hp_gained += g_hp * (gems as i32);
        attacker.perm_blood_gems_gained += gems;
    }
}
