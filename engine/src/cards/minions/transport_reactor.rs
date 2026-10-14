//! `Transport Reactor` (`BGDUO31_211`) — Tier 6 Mech (`3/3`), Duos only.
//! **Magnetic**. Has `+3/+3` for each time your team has **Passed** this game *(wherever this
//! is)* (Golden `BGDUO31_211_G`, `6/6`: `+6/+6`).
//!
//! It counts `team_passes_this_game` (`docs/duos_tavern.md` §7.1), the Passes before it was
//! bought too. Magnetized onto a Mech, the Mech keeps the bonus as its own "wherever this is"
//! aura (`Unit::pass_aura`), which grows with the team's later Passes.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 10603;
pub const NAME: &str = "Transport Reactor";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 6)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::Magnetic)
        .on_aura_bonus(aura_bonus)
        .on_magnetize_transfer(on_magnetize_transfer)
}

/// `+3/+3` (`+6/+6` if Golden) for each of the team's Passes this game.
pub fn aura_bonus(unit: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    let bonus = 3 * unit.golden_mult() * auras.team_passes_this_game as i32;
    (bonus, bonus)
}

/// Magnetized: the target took this card's stats, its aura's included. The target now has this
/// card's per-Pass bonus (and any it carried) as its own aura, with those stats already applied.
pub fn on_magnetize_transfer(source: &Unit, target: &mut Unit) {
    target.aura_applied.0 += source.aura_applied.0;
    target.aura_applied.1 += source.aura_applied.1;
    target.pass_aura += 3 * source.golden_mult() + source.pass_aura;
}
