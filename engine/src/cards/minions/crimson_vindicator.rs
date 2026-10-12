//! `Crimson Vindicator` (`BG36_241`) — Tier 6 Dragon (`8/9`, Divine Shield).
//!
//! Divine Shield. Rally: Cast `Mighty Dragonbreath` (`twice` if Golden).
//!
//! Each cast gets your spell power: the permanent bonuses plus the spell auras of the friendly
//! minions alive when it is cast (`docs/combat.md` §2).

use crate::cards::{self, CardTemplate};
use crate::events::Event;
use crate::model::{CardId, Keyword, PlayerAuras, Side, Tribe, Unit};

pub const ID: CardId = 604;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Crimson Vindicator", 8, 9, 6)
        .with_tribe(Tribe::Dragon)
        .with_keyword(Keyword::DivineShield)
        .on_rally(|c| {
            on_rally(c.side, c.board, c.is_golden, c.auras, c.events);
            Vec::new()
        })
}

pub fn on_rally(
    side: Side,
    board: &mut [Unit],
    is_golden: bool,
    auras: &PlayerAuras,
    events: &mut Vec<Event>,
) {
    let repeats = if is_golden { 2 } else { 1 };
    for _ in 0..repeats {
        let (atk, hp) = cards::spell_stat_buff_on_board(board, auras, 3, 2);
        for u in board.iter_mut() {
            if u.health <= 0 {
                continue;
            }
            let mut mult = 1i32;
            if u.tribe.matches(Tribe::Dragon) {
                mult += 1;
            }
            if u.divine_shield {
                mult += 1;
            }
            let d_atk = atk * mult;
            let d_hp = hp * mult;
            u.add_stats(d_atk, d_hp);
            events.push(Event::StatBuff {
                side,
                unit: u.id,
                atk_delta: d_atk,
                hp_delta: d_hp,
                attack: u.attack,
                health: u.health,
                reason: "Crimson Vindicator",
            });
        }
    }
}
