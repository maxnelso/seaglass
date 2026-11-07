//! `Bonker` (`BG20_104`) — Tier 4 Quilboar (`2/7`, Windfury).
//!
//! Windfury. Rally: This plays a (`2` if Golden) Blood Gem(s) on all your other minions.
//!
//! The Blood Gems it plays, in the Tavern too (`Sky-hatch Runaway`), aren't spells you cast:
//! `Felboar` doesn't count them, and the "whenever you cast a spell on" minions (`Glambot`, ...)
//! don't react (`docs/tavern.md` §7.4).

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
    let in_combat = crate::cards::in_combat_resolution();
    for idx in 0..board.len() {
        if idx != attacker_pos {
            let (atk_delta, hp_delta) =
                crate::cards::play_blood_gems_on_board_unit(board, idx, count, auras, in_combat);
            let u = &board[idx];
            events.push(Event::StatBuff {
                side,
                unit: u.id,
                atk_delta,
                hp_delta,
                attack: u.attack,
                health: u.health,
                reason: "Bonker",
            });
        }
    }
}
