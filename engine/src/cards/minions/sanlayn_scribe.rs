//! `San'layn Scribe` (`BGDUO31_208`) — Tier 4 Undead (`4/4`), Duos only.
//! Has `+4/+4` for each of your team's San'layn Scribes that died this game *(wherever this is)*
//! (Golden `BGDUO31_208_G`, `8/8`: `+8/+8`).
//!
//! As `Eternal Knight`: each Scribe that dies (Golden or not, in combat or in the Tavern) counts
//! 1 for its player (this card's counter); the bonus counts the team's, the player's own plus
//! their teammate's (`PlayerAuras::team_counter`, `docs/duos_tavern.md` §7.1).

use crate::cards::CardTemplate;
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 10406;
pub const NAME: &str = "San'layn Scribe";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 4, 4)
        .with_tribe(Tribe::Undead)
        .on_died(on_died)
        .on_aura_bonus(aura_bonus)
}

/// `+4/+4` (`+8/+8` if Golden) for each of the team's Scribes that died this game.
pub fn aura_bonus(unit: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    let stacks = auras.team_counter(ID) * unit.golden_mult();
    (4 * stacks, 4 * stacks)
}

/// Count this death towards every Scribe's aura.
pub fn on_died(_unit: &Unit, auras: &mut PlayerAuras) {
    auras.add_counter(ID, 1);
}
