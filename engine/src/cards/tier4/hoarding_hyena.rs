//! `Hoarding Hyena` (`BG36_210`) — Tier 4 Beast (`4/6`).
//!
//! Rally: Summon a (`Golden` if Golden) Tasty Lobster.

use crate::cards::{tier3, CardTemplate};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 429;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Hoarding Hyena", 4, 6, 4)
        .with_tribe(Tribe::Beast)
        .on_rally(|c| on_rally(&c.board[c.attacker_pos]))
}

pub fn on_rally(attacker: &Unit) -> Vec<Unit> {
    let mut lobster = tier3::tasty_lobster::template().instantiate();
    if attacker.is_golden {
        lobster.name = format!("Golden {}", lobster.name);
        lobster.attack *= 2;
        lobster.health *= 2;
        lobster.base_attack *= 2;
        lobster.base_health *= 2;
        lobster.max_attack = lobster.attack;
        lobster.max_health = lobster.health;
        lobster.is_golden = true;
    }
    vec![lobster]
}
