//! The Volumizers (`Blue`, `Green` and `Red Volumizer`): the first play or Magnetize of each one
//! improves all Volumizers this game, through one shared card counter. Other cards generate
//! random Volumizers (`Conveyor Construct`, `Auto Reveille`).

use crate::cards::minions;
use crate::model::{CardId, PlayerAuras, Unit};
use crate::rng::Rng;

/// Card counter shared by all Volumizers: the Attack/Health they have this game. Card counters
/// are keyed by a card, so this one uses `Blue Volumizer`'s id.
pub const VOLUMIZER_COUNTER: CardId = minions::blue_volumizer::ID;

/// The Attack/Health all Volumizers have this game ([`VOLUMIZER_COUNTER`]).
pub fn volumizer_bonus(auras: &PlayerAuras) -> (i32, i32) {
    auras.counter_pair(VOLUMIZER_COUNTER)
}

/// Give all Volumizers `+atk/+hp` this game.
pub fn add_volumizer_bonus(auras: &mut PlayerAuras, atk: i32, hp: i32) {
    auras.add_counter_pair(VOLUMIZER_COUNTER, (atk, hp));
}

/// The shared Volumizer bonus (the `aura_bonus` hook of all three Volumizers).
pub fn volumizer_aura_bonus(_: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    volumizer_bonus(auras)
}

/// Tripled: the Golden's first-play effect can trigger again (the `merge_golden` hook of all
/// three Volumizers).
pub fn merge_golden(_: &[Unit], golden: &mut Unit) {
    golden.threshold_triggered = false;
}

/// A uniformly random Volumizer.
pub fn draw_random_volumizer(rng: &mut Rng) -> Unit {
    let templates = [
        minions::blue_volumizer::template(),
        minions::green_volumizer::template(),
        minions::red_volumizer::template(),
    ];
    let pick = rng.below(templates.len());
    templates[pick].instantiate()
}
