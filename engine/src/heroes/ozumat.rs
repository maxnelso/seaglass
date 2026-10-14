//! `Ozumat` (`BG23_HERO_201`) — Armor 15 (Duos 13).
//!
//! Hero Power: `Tentacular` (`BG23_HERO_201p`), 0 Gold: When you have space in combat, summon a 2/2 Tentacle with Taunt. (Gains +1/+1 after you sell a minion!)
//!
//! Passive. Once per combat (engine decision), as soon as there is room on the board
//! (`hero_combat_space`: at Start of Combat or after a round of deaths), an `Ozumat's Tentacle`
//! (`BG23_HERO_201pt`, a Beast with Taunt) of `2 + stacks` / `2 + stacks` is summoned at the right
//! end. Each minion the player sells adds a stack (`hero_after_sell`). `armed` remembers that this
//! combat's Tentacle came (reset at the start of each turn).

use crate::cards::{BoardCtx, CardHooks};
use crate::combat::MAX_BOARD_SIZE;
use crate::heroes::{power_mut, HERO_SOURCE};
use crate::model::{CardId, HeroId, HeroState, Keyword, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 86013;
pub const POWER: CardId = 86014;

/// `Ozumat's Tentacle` (`BG23_HERO_201pt`).
pub const TENTACLE: CardId = 86227;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_turn_reset(new_turn)
            .on_hero_after_sell(grow)
            .on_hero_combat_space(summon_tentacle),
    )]
}

/// Start of turn: the coming combat has its Tentacle to come.
fn new_turn(hero: &mut HeroState) {
    hero.power.armed = 0;
}

/// A minion was sold: the Tentacle gains +1/+1.
fn grow(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.stacks += 1;
    }
}

/// Room opened: this combat's Tentacle, once.
fn summon_tentacle(ctx: &mut BoardCtx<'_>) {
    let Some(power) = power_mut(ctx.auras) else {
        return;
    };
    if power.armed > 0 || ctx.board.len() >= MAX_BOARD_SIZE {
        return;
    }
    power.armed = 1;
    let size = 2 + power.stacks;
    let tentacle = Unit::new("Ozumat's Tentacle", size, size)
        .with_card_id(TENTACLE)
        .with_tribe(Tribe::Beast)
        .with_keyword(Keyword::Taunt);
    ctx.cursor = ctx.board.len();
    ctx.summon_as(HERO_SOURCE, tentacle, "Tentacular");
}
