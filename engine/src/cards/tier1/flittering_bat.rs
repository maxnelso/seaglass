//! `Flittering Bat` (`BG36_200`) — Tier 1 Beast (`1/4`).
//! **Rally:** Summon a `1/1` (`2/2` if Golden) Beast.

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 104;
pub const NAME: &str = "Flittering Bat";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 4, 1).with_tribe(Tribe::Beast)
}

pub fn on_rally(attacker: &mut Unit) -> Vec<Unit> {
    vec![tokens::make_flittering_beast(attacker.is_golden)]
}
