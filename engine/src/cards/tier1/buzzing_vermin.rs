//! `Buzzing Vermin` (`BG31_803`) — Tier 1 Beast (`1/1`).
//! **Taunt**. **Deathrattle:** Summon a `2/2` (`4/4` if Golden) Beetle.

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Keyword, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 103;
pub const NAME: &str = "Buzzing Vermin";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 1, 1)
        .with_tribe(Tribe::Beast)
        .with_keyword(Keyword::Taunt)
}

pub fn on_deathrattle(dying: &Unit, auras: &PlayerAuras) -> Vec<Unit> {
    vec![tokens::make_beetle(dying.is_golden, auras)]
}
