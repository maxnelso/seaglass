//! `Puddle Prancer` (`BGDUO_117`) — Tier 3 Murloc (`4/4`), Duos only.
//! After this is Passed, gain `+4/+4` (Golden `BGDUO_117_G`, `8/8`: `+8/+8`).
//!
//! Its `passed` hook, each time it is Passed, by its player or by a card
//! (`docs/duos_tavern.md` §8.3).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 10306;
pub const NAME: &str = "Puddle Prancer";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 4, 3)
        .with_tribe(Tribe::Murloc)
        .on_passed(passed)
}

/// Passed: `+4/+4` (`+8/+8` if Golden).
pub fn passed(prancer: &mut Unit) {
    let buff = 4 * prancer.golden_mult();
    prancer.add_stats(buff, buff);
}
