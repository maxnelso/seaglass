//! `Roadboar` (`BG20_101`) — Tier 2 Quilboar (`2/4`).
//! **Rally:** Get a (`2` if Golden) **Blood Gem(s)**.

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 224;
pub const NAME: &str = "Roadboar";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 4, 2)
        .with_tribe(Tribe::Quilboar)
        .on_rally(|c| on_rally(&c.board[c.attacker_pos], c.generated_hand))
}

pub fn on_rally(attacker: &Unit, generated_hand: &mut Vec<Unit>) -> Vec<Unit> {
    let count = if attacker.is_golden { 2 } else { 1 };
    for _ in 0..count {
        generated_hand.push(tokens::make_blood_gem());
    }
    Vec::new()
}
