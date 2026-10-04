//! `Thorned Trailblazer` (`BG31_327`) — Tier 3 Quilboar (`4/5`).
//! One (`Two` if Golden) **Choose One** card(s) each turn has both effects combined.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 334;
pub const NAME: &str = "Thorned Trailblazer";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 5, 3)
        .with_tribe(Tribe::Quilboar)
        .on_reset_turn_charges(reset_turn_charges)
        .on_made_golden(|u| u.trailblazer_charges_left += 1)
}

pub fn reset_turn_charges(unit: &mut Unit) {
    unit.trailblazer_charges_left = if unit.is_golden { 2 } else { 1 };
}
