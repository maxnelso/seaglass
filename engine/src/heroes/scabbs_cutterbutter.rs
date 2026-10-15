//! `Scabbs Cutterbutter` (`BG21_HERO_010`) — Armor 18 (Duos 16).
//!
//! Hero Power: `I Spy` (`BG21_HERO_010p`), 2 Gold: Discover a plain copy of a minion from your next opponent's warband.
//!
//! Active, untargeted, once per turn: requires `state.next_opponent_warband()` to be non-empty and
//! room in the hand (`hand.len() < 10`). Samples up to 3 minions without replacement from
//! `state.next_opponent_warband()` via `rng.below`, converts each into a plain copy
//! (`cards::instantiate_plain_copy`), applies the player's global auras, and offers them as a
//! Discover without touching `CardPool`.

use crate::cards::{self, CardHooks};
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 76562;
pub const POWER: CardId = 76563;

/// Discover options.
const OPTIONS: usize = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(i_spy)
            .on_hero_power_usable(|state| {
                !state.next_opponent_warband().is_empty() && super::hand_has_room(state)
            })
            .on_hero_option_chosen(option_chosen),
    )]
}

/// Using it: Discover up to 3 plain copies from the next opponent's warband.
fn i_spy(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, rng: &mut Rng) {
    let mut candidates: Vec<Unit> = state.next_opponent_warband().to_vec();
    let mut options = Vec::new();
    while options.len() < OPTIONS && !candidates.is_empty() {
        let pick = match candidates.len() {
            1 => 0,
            k => rng.below(k),
        };
        let mut opt = cards::instantiate_plain_copy(&candidates.remove(pick));
        state.apply_global_unit_auras(&mut opt);
        opt.to_hero_power = true;
        opt.taught_spell_id = Some(POWER);
        options.push(opt);
    }
    state.push_discover(options);
}

/// Choosing an option: add the plain copy to hand (without returning unchosen options to `pool`).
fn option_chosen(
    state: &mut TavernState,
    mut chosen: Unit,
    _: Vec<Unit>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if chosen.taught_spell_id == Some(POWER) {
        chosen.taught_spell_id = None;
    }
    if super::hand_has_room(state) {
        let obtained = chosen.clone();
        state.add_to_hand(chosen);
        cards::on_obtained(state, &obtained, pool, rng);
    }
    cards::on_card_discovered(state, pool, rng);
}
