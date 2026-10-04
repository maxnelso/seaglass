//! `Bonker` (`BG20_104`) — Tier 4 Quilboar (`2/7`, Windfury).
//!
//! Windfury. Rally: This plays a (`2` if Golden) Blood Gem(s) on all your other minions.

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Keyword, PlayerAuras, Side, Tribe, Unit};

pub const ID: CardId = 406;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Bonker", 2, 7, 4)
        .with_tribe(Tribe::Quilboar)
        .with_keyword(Keyword::Windfury)
        .on_rally(|c| {
            on_rally(c.side, c.board, c.attacker_pos, c.auras, c.events);
            Vec::new()
        })
}

pub fn on_rally(
    side: Side,
    board: &mut [Unit],
    attacker_pos: usize,
    auras: &PlayerAuras,
    events: &mut Vec<Event>,
) {
    let count = if board[attacker_pos].is_golden { 2 } else { 1 };
    for (idx, u) in board.iter_mut().enumerate() {
        if idx != attacker_pos {
            let old_atk = u.attack;
            let old_hp = u.health;
            u.play_blood_gems(count, auras);
            events.push(Event::StatBuff {
                side,
                unit: u.id,
                atk_delta: u.attack - old_atk,
                hp_delta: u.health - old_hp,
                attack: u.attack,
                health: u.health,
                reason: "Bonker",
            });
        }
    }
}
