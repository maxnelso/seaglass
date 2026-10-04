//! `Bilgewater Breakout` (`BG36_520`) — Tier 2 Pirate (`3/2`).
//! **Battlecry:** Get a `Lockbox`. If you already have one, it opens `1` (`2` if Golden) turn(s) sooner instead.

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 201;
pub const NAME: &str = "Bilgewater Breakout";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 2, 2).with_tribe(Tribe::Pirate)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, rng: &mut Rng) {
    let accel = if unit.is_golden { 2 } else { 1 };
    if let Some(idx) = state
        .hand
        .iter()
        .position(|c| c.card_id == tokens::SPELL_LOCKBOX && c.lockbox_turns_left > 0)
    {
        state.hand[idx].lockbox_turns_left =
            state.hand[idx].lockbox_turns_left.saturating_sub(accel);
        if state.hand[idx].lockbox_turns_left == 0 {
            state.open_lockbox_at(idx, rng);
        }
    } else {
        state.add_to_hand(tokens::make_lockbox());
    }
}
