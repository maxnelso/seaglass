//! `Tusked Camper` (`BG33_886`) — Tier 1 Quilboar (`2/3`).
//! **Rally:** This plays a **Blood Gem** (`2` if Golden) on itself.
//!
//! The Blood Gems it plays, in the Tavern too (`Sky-hatch Runaway`), aren't spells you cast
//! (`Felboar`, `Gatekeeper Amalgam`, ... ignore them; `docs/tavern.md` §7.4).

use crate::cards::CardTemplate;
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 117;
pub const NAME: &str = "Tusked Camper";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 3, 1)
        .with_tribe(Tribe::Quilboar)
        .on_rally(|c| {
            let count = if c.is_golden { 2 } else { 1 };
            crate::cards::play_blood_gems_on_board_unit(
                c.board,
                c.attacker_pos,
                count,
                c.auras,
                c.in_combat,
            );
            Vec::new()
        })
}

pub fn on_rally(attacker: &mut Unit, auras: &PlayerAuras) -> Vec<Unit> {
    let count = if attacker.is_golden { 2 } else { 1 };
    attacker.play_blood_gems(count, auras);
    Vec::new()
}
