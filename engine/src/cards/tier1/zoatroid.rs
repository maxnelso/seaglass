//! `Zoatroid` (`BG36_098`) — Tier 1 Aberration (`3/2`).
//! When you sell this, get a `0/2` Tentacle with **Taunt** (two if Golden).

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 102;
pub const NAME: &str = "Zoatroid";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 2, 1).with_tribe(Tribe::Aberration)
}

pub fn on_sell(state: &mut TavernState, sold: &Unit) {
    let count = if sold.is_golden { 2 } else { 1 };
    for _ in 0..count {
        if state.hand.len() < 10 {
            let tentacle = tokens::make_aberrant_tentacle();
            let cid = tentacle.card_id;
            state.hand.push(tentacle);
            state.check_and_resolve_triple(cid);
        }
    }
}
