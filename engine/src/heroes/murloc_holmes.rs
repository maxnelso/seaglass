//! `Murloc Holmes` (`BG23_HERO_303`) — Armor 12 (Duos 10).
//!
//! Hero Power: `Detective for Hire` (`BG23_HERO_303p2`), 0 Gold: Look at 2 minions. Guess which one your next opponent had last combat for a Tavern Coin.
//!
//! Active, untargeted, once per turn: if `state.next_opponent_warband()` is non-empty, picks one
//! random minion from it (`rng.below`) and offers its plain copy as a 1-option Discover; if empty,
//! offers a 1-option `No Minions` (`BG23_HERO_303pt`, dbfId `92974`) card. Choosing the option
//! gives a `Tavern Coin` (`docs/heroes.md` §11.2, engine decision).

use crate::cards::{self, tokens, CardHooks};
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 84867;
pub const POWER: CardId = 90403;
pub const NO_MINIONS: CardId = 92974;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(detective_for_hire)
            .on_hero_option_chosen(option_chosen),
    )]
}

/// Using it: offer the next opponent's minion (or `No Minions` if their warband was empty).
fn detective_for_hire(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, rng: &mut Rng) {
    let warband = state.next_opponent_warband();
    let mut opt = if warband.is_empty() {
        let mut u = Unit::new("No Minions", 0, 1).with_card_id(NO_MINIONS);
        u.is_spell = true;
        u
    } else {
        let pick = match warband.len() {
            1 => 0,
            k => rng.below(k),
        };
        let mut u = cards::instantiate_plain_copy(&warband[pick]);
        state.apply_global_unit_auras(&mut u);
        u
    };
    opt.to_hero_power = true;
    opt.taught_spell_id = Some(POWER);
    state.push_discover(vec![opt]);
}

/// Choosing the option: get a `Tavern Coin` (the minion itself is not added to hand).
fn option_chosen(state: &mut TavernState, _: Unit, _: Vec<Unit>, _: &mut CardPool, _: &mut Rng) {
    state.add_to_hand(tokens::make_tavern_coin());
}
