//! `Bru'kan` (`BG22_HERO_001`) — Armor 15 (Duos 12).
//!
//! Hero Power: `Embrace the Elements` (`BG22_HERO_001p`), 0 Gold: Choose an Element. Start of Combat: Call upon that Element.
//!
//! Active, untargeted, once per turn. Using it offers a Choose One of the four Elements (their
//! spell cards, `BG22_HERO_001p_t1_s` ...); the chosen Element becomes the Hero Power (its
//! `BG22_HERO_001p_tN` card) for the next combat only, and the power reverts at the start of the
//! next turn (confirmed). At Start of Combat (`docs/heroes.md` §11):
//!
//! - **Earth Invocation**: 4 different random friendly minions (fewer if fewer; confirmed) gain
//!   "Deathrattle: Summon a 1/1 Elemental" (`Stone Elemental`, an extra Deathrattle);
//! - **Fire Invocation**: the left-most minion's Attack doubles;
//! - **Water Invocation**: the right-most minion gains Divine Shield and Taunt;
//! - **Lightning Invocation**: 1 damage to 5 different random enemy minions (fewer if fewer;
//!   confirmed).
//!
//! A random pick of `n` among `k` candidates draws only if `k > n`: `rng.below` over the
//! candidates left, once per pick.

use crate::cards::{tokens, BoardCtx, CardHooks, HeroCombatCtx};
use crate::heroes::power_mut_for;
use crate::model::{CardId, HeroId, HeroState, Keyword, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 79719;
pub const POWER: CardId = 79720;

/// The Elements' Hero Power cards (`BG22_HERO_001p_t1` .. `t4`).
pub const EARTH: CardId = 79721;
pub const FIRE: CardId = 79722;
pub const WATER: CardId = 79723;
pub const LIGHTNING: CardId = 79724;

/// The Choose One options (the Elements' spell cards, `BG22_HERO_001p_t1_s` .. `t4_s`).
const OPTIONS: [(CardId, &str); 4] = [
    (80867, "Earth Invocation"),
    (80868, "Fire Invocation"),
    (80869, "Water Invocation"),
    (80870, "Lightning Invocation"),
];

/// Earth Invocation's Deathrattle (`BG22_HERO_001p_t1e`) and its Elemental (`..._t1et`).
pub const EARTH_DEATHRATTLE: CardId = 79727;
pub const STONE_ELEMENTAL: CardId = 79728;

/// The Hero Power's hooks, its Elements', their options' and Earth's Deathrattle's.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    let element = |start_of_combat: fn(&mut HeroCombatCtx<'_>)| {
        CardHooks::EMPTY
            .on_hero_start_of_combat(start_of_combat)
            .on_hero_turn_reset(revert)
    };
    vec![
        (POWER, CardHooks::EMPTY.on_hero_power(offer_elements)),
        (EARTH, element(earth)),
        (FIRE, element(fire)),
        (WATER, element(water)),
        (LIGHTNING, element(lightning)),
        (OPTIONS[0].0, CardHooks::EMPTY.on_chosen(choose_earth)),
        (OPTIONS[1].0, CardHooks::EMPTY.on_chosen(choose_fire)),
        (OPTIONS[2].0, CardHooks::EMPTY.on_chosen(choose_water)),
        (OPTIONS[3].0, CardHooks::EMPTY.on_chosen(choose_lightning)),
        (
            EARTH_DEATHRATTLE,
            CardHooks::EMPTY.on_deathrattle(stone_elemental),
        ),
    ]
}

/// Using it: choose an Element.
fn offer_elements(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    let options = OPTIONS
        .iter()
        .map(|&(id, name)| tokens::make_choice_option(id, name, false))
        .collect();
    state.push_discover(options);
}

/// The chosen Element becomes the Hero Power.
fn become_element(state: &mut TavernState, element: CardId) {
    if let Some(power) = power_mut_for(&mut state.auras, POWER) {
        power.card_id = element;
    }
}

fn choose_earth(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    become_element(state, EARTH);
}

fn choose_fire(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    become_element(state, FIRE);
}

fn choose_water(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    become_element(state, WATER);
}

fn choose_lightning(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    become_element(state, LIGHTNING);
}

/// Start of turn: the Element lasted one combat; the power is Embrace the Elements again.
fn revert(hero: &mut HeroState) {
    hero.power.card_id = POWER;
    hero.power.uses_left = 1;
}

/// `n` different random picks among `candidates` (all of them if there are no more than `n`).
fn pick_different<T: Copy>(mut candidates: Vec<T>, n: usize, rng: &mut Rng) -> Vec<T> {
    if candidates.len() <= n {
        return candidates;
    }
    (0..n)
        .map(|_| candidates.remove(rng.below(candidates.len())))
        .collect()
}

/// Earth: 4 different random friendly minions gain "Deathrattle: Summon a 1/1 Elemental".
fn earth(ctx: &mut HeroCombatCtx<'_>) {
    let own = &mut ctx.own;
    let picked = pick_different((0..own.board.len()).collect(), 4, own.rng);
    for i in picked {
        own.board[i].extra_deathrattles.push(EARTH_DEATHRATTLE);
    }
}

/// Fire: the left-most minion's Attack doubles.
fn fire(ctx: &mut HeroCombatCtx<'_>) {
    if let Some(attack) = ctx.own.board.first().map(|u| u.attack.max(0)) {
        ctx.own.buff_unit(0, attack, 0, "Fire Invocation");
    }
}

/// Water: the right-most minion gains Divine Shield and Taunt.
fn water(ctx: &mut HeroCombatCtx<'_>) {
    if let Some(unit) = ctx.own.board.last_mut() {
        unit.apply_keyword(Keyword::DivineShield, false);
        unit.apply_keyword(Keyword::Taunt, false);
    }
}

/// Lightning: 1 damage to 5 different random enemy minions.
fn lightning(ctx: &mut HeroCombatCtx<'_>) {
    let enemies = ctx
        .enemy_board
        .iter()
        .filter(|u| u.health > 0)
        .map(|u| u.id)
        .collect();
    for id in pick_different(enemies, 5, ctx.own.rng) {
        ctx.enemy_damage.push((id, 1));
    }
}

/// Earth's Deathrattle: summon a 1/1 Elemental.
fn stone_elemental(dying: &Unit, ctx: &mut BoardCtx<'_>) {
    let elemental = Unit::new("Stone Elemental", 1, 1)
        .with_card_id(STONE_ELEMENTAL)
        .with_tribe(Tribe::Elemental);
    ctx.summon(dying.id, elemental);
}
