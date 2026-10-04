//! `The Shadow of Doubt` (`BG36_109`) — Tier 6 Aberration (`6/8`).
//!
//! Whenever a card is added to your hand, give your Deity `+4/+5` (`+8/+10` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 623;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "The Shadow of Doubt", 6, 8, 6)
        .with_tribe(Tribe::Aberration)
        .on_card_added_to_hand(on_card_added_to_hand)
}

pub fn on_card_added_to_hand(unit: &Unit, auras: &mut PlayerAuras) {
    let mult = unit.golden_mult();
    auras.deity.attack += 4 * mult;
    auras.deity.health += 5 * mult;
}
