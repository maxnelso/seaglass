//! `Dr. Boom's Monster` (`BG31_176`) — Tier 6 Mech (`2/2`), in no pool.
//! **Magnetic**. Has `+2/+2` (`+4/+4` if Golden) for each time you've **Magnetized** this game
//! *(wherever this is)*.
//!
//! Only `Boom's Monster Portrait` gives it (`docs/trinkets.md` §11.9). Its aura counts the
//! player's Magnetizations onto their minions this game (`PlayerAuras::magnetizations`, extra
//! ones such as `Drone Duplicator`'s included). Magnetized itself, it gives its stats as they are
//! then; they stop growing on the minion it joins **(engine decision)**.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 10804;
pub const NAME: &str = "Dr. Boom's Monster";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 2, 6)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::Magnetic)
        .on_aura_bonus(aura_bonus)
}

/// `+2/+2` (`+4/+4` if Golden) per Magnetization this game.
pub fn aura_bonus(unit: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    let stacks = (auras.magnetizations as i32).saturating_mul(2 * unit.golden_mult());
    (stacks, stacks)
}
