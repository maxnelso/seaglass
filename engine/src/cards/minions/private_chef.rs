//! `Private Chef` (`BGDUO31_209`) — Tier 4 Naga (`5/4`), Duos only.
//! **Spellcraft:** Choose a minion. Get a different random minion of its type, then Pass it
//! (Golden `BGDUO31_209_G`, `10/8`: 2 different random minions).
//!
//! In no pool: Naga is in no lobby (`docs/duos_tavern.md` §8). Its Spellcraft spell is
//! `Private Selection` (Golden if it is), given when it is played and at the start of each turn
//! (`docs/duos_tavern.md` §8.3).

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 10405;
pub const NAME: &str = "Private Chef";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 5, 4, 4)
        .with_tribe(Tribe::Naga)
        .on_spellcraft(spellcraft)
}

/// Its Spellcraft spell: `Private Selection`, Golden if it is.
pub fn spellcraft(chef: &Unit) -> Unit {
    let mut spell = spells::make_private_selection();
    spell.is_golden = chef.is_golden;
    spell
}
