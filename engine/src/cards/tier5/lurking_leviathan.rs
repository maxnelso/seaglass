//! `Lurking Leviathan` (`BG35_602`) — Tier 5 Beast (`3/9`).
//!
//! Whenever you summon a Beast, give it `+3` (`+6` if Golden) Attack and improve this permanently.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit, UnitId};

pub const ID: CardId = 532;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Lurking Leviathan", 3, 9, 5).with_tribe(Tribe::Beast)
}

pub fn on_beast_summoned_combat(board: &mut [Unit], summoned_id: UnitId, summoned: &mut Unit) {
    if !summoned.tribe.matches(Tribe::Beast) {
        return;
    }
    let mut total_atk = 0i32;
    for u in board.iter_mut() {
        if u.id != summoned_id && u.health > 0 && u.card_id == ID {
            let base = if u.is_golden { 6 } else { 3 };
            total_atk += base * (1 + u.leviathan_stacks as i32);
            u.leviathan_stacks += 1;
        }
    }
    if total_atk > 0 {
        summoned.add_stats(total_atk, 0);
    }
}

pub fn on_beast_summoned_tavern(board: &mut [Unit], summoned_pos: usize) {
    if summoned_pos >= board.len() || !board[summoned_pos].tribe.matches(Tribe::Beast) {
        return;
    }
    let mut total_atk = 0i32;
    for (idx, u) in board.iter_mut().enumerate() {
        if idx != summoned_pos && u.card_id == ID {
            let base = if u.is_golden { 6 } else { 3 };
            total_atk += base * (1 + u.leviathan_stacks as i32);
            u.leviathan_stacks += 1;
        }
    }
    if total_atk > 0 {
        board[summoned_pos].add_stats(total_atk, 0);
    }
}
