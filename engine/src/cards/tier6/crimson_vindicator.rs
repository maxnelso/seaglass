//! `Crimson Vindicator` (`BG36_241`) — Tier 6 Dragon (`8/9`, Divine Shield).
//!
//! Divine Shield. Rally: Cast `Mighty Dragonbreath` (`twice` if Golden).

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Keyword, PlayerAuras, Side, Tribe, Unit};

pub const ID: CardId = 604;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Crimson Vindicator", 8, 9, 6)
        .with_tribe(Tribe::Dragon)
        .with_keyword(Keyword::DivineShield)
}

pub fn on_rally(
    side: Side,
    board: &mut [Unit],
    is_golden: bool,
    auras: &PlayerAuras,
    events: &mut Vec<Event>,
) {
    let repeats = if is_golden { 2 } else { 1 };
    let (atk, hp) = auras.spell_stat_buff(3, 2);
    for _ in 0..repeats {
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
