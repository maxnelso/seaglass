//! `Tusked Camper` (`BG33_886`) — Tier 1 Quilboar (`2/3`).
//! **Rally:** This plays a **Blood Gem** (`2` if Golden) on itself.
//!
//! Triggered in the Tavern (`Sky-hatch Runaway`), each Blood Gem it plays counts as a spell cast
//! (`Felboar`; `docs/tavern.md` §7.4).

use crate::cards::CardTemplate;
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 117;
pub const NAME: &str = "Tusked Camper";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 3, 1)
        .with_tribe(Tribe::Quilboar)
        .on_rally(|c| on_rally(&mut c.board[c.attacker_pos], c.auras))
}

pub fn on_rally(attacker: &mut Unit, auras: &PlayerAuras) -> Vec<Unit> {
    let count = if attacker.is_golden { 2 } else { 1 };
    attacker.play_blood_gems_by_minion(count, auras);
    Vec::new()
}
