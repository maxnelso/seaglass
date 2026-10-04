//! `Amber Guardian` (`BG24_500`) — Tier 3 Dragon (`3/2`).
//! **Taunt**. **Start of Combat:** Give another (`two other` if Golden) friendly Dragon(s) `+2/+2` and **Divine Shield**.

use crate::cards::CardTemplate;
use crate::events::Event;
use crate::model::{CardId, Keyword, Side, Tribe, Unit, UnitId};
use crate::rng::Rng;

pub const ID: CardId = 303;
pub const NAME: &str = "Amber Guardian";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 2, 3)
        .with_tribe(Tribe::Dragon)
        .with_keyword(Keyword::Taunt)
}

pub fn on_start_of_combat(
    side: Side,
    board: &mut [Unit],
    source_id: UnitId,
    is_golden: bool,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    let mut candidates: Vec<usize> = board
        .iter()
        .enumerate()
        .filter(|(_, u)| u.id != source_id && u.health > 0 && u.tribe.matches(Tribe::Dragon))
        .map(|(i, _)| i)
        .collect();
    let count = if is_golden { 2 } else { 1 };
    let n = candidates.len().min(count);
    for _ in 0..n {
        let pick = if candidates.len() == 1 {
            0
        } else {
            rng.below(candidates.len())
        };
        let idx = candidates.remove(pick);
        let u = &mut board[idx];
        u.add_stats(2, 2);
        u.apply_keyword(Keyword::DivineShield, true);
        events.push(Event::StatBuff {
            side,
            unit: u.id,
            atk_delta: 2,
            hp_delta: 2,
            attack: u.attack,
            health: u.health,
            reason: NAME,
        });
    }
}
