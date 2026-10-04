//! `Fearless Foodie` (`BG30_123`) — Tier 3 Quilboar (`2/4`).
//! **Choose One -** Your **Blood Gems** give an extra `+1/+1` (`+2/+2` if Golden) this game;
//! or Get `4` (`8` if Golden) **Blood Gems**.

use crate::cards::tokens::{
    self, make_choice_option, CHOICE_FOODIE_BUFF_GEMS, CHOICE_FOODIE_GET_GEMS,
};
use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 314;
pub const NAME: &str = "Fearless Foodie";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 4, 3)
        .with_tribe(Tribe::Quilboar)
        .on_choose_one(|state, unit, _, pool, rng| on_battlecry(state, unit, pool, rng))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    let g = unit.is_golden;
    let opt0 = make_choice_option(CHOICE_FOODIE_BUFF_GEMS, "Culinary Creation", g);
    let opt1 = make_choice_option(CHOICE_FOODIE_GET_GEMS, "Side of Gems", g);
    state.resolve_choose_one(opt0, opt1, pool, rng);
}

/// Culinary Creation: your Blood Gems give an extra +1/+1 (+2/+2 if Golden) this game.
pub fn choose_buff_gems(state: &mut TavernState, chosen: &Unit, _: &mut CardPool, _: &mut Rng) {
    let mult = chosen.golden_mult();
    state.auras.blood_gem_bonus_atk += mult;
    state.auras.blood_gem_bonus_hp += mult;
}

/// Side of Gems: get 4 (8 if Golden) Blood Gems.
pub fn choose_get_gems(state: &mut TavernState, chosen: &Unit, _: &mut CardPool, _: &mut Rng) {
    for _ in 0..4 * chosen.golden_mult() {
        state.add_to_hand(tokens::make_blood_gem());
    }
}
