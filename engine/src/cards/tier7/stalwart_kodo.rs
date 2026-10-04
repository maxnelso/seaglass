//! `Stalwart Kodo` (`BG34_322`) — Tier 7 Beast (`16/32`).
//!
//! After you summon a minion in combat, give it this minion's maximum stats (double if Golden).
//! (`3` times per combat.)

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit, UnitId};

pub const ID: CardId = 709;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Stalwart Kodo", 16, 32, 7).with_tribe(Tribe::Beast)
}

pub fn on_minion_summoned_in_combat(
    board: &mut [Unit],
    summoned_id: UnitId,
    token: &mut Unit,
) {
    for u in board.iter_mut() {
        if u.health > 0 && u.id != summoned_id && u.card_id == ID && u.kodo_triggers_left > 0 {
            u.kodo_triggers_left -= 1;
            let mult = if u.is_golden { 2 } else { 1 };
            let atk = u.max_attack.max(0) * mult;
            let hp = u.max_health.max(0) * mult;
            token.add_stats(atk, hp);
        }
    }
}
