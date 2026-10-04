//! `Banana Slamma` (`BG26_802`) — Tier 4 Beast (`3/6`).
//!
//! After you summon a Beast in combat, double (`triple` if Golden) its Attack.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit, UnitId};

pub const ID: CardId = 403;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Banana Slamma", 3, 6, 4).with_tribe(Tribe::Beast)
}

pub fn on_beast_summoned(board: &[Unit], summoned_id: UnitId, summoned: &mut Unit) {
    if !summoned.tribe.matches(Tribe::Beast) {
        return;
    }
    for u in board {
        if u.id != summoned_id && u.health > 0 && u.card_id == ID {
            let extra = if u.is_golden {
                summoned.attack * 2
            } else {
                summoned.attack
            };
            summoned.add_stats(extra, 0);
        }
    }
}
