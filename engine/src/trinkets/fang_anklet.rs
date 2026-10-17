//! `Fang Anklet` (`BG35_MagicItem_701`) — Greater, 3 Gold, Beast.
//!
//! Start of Combat: Your Beasts have +1/+1 this combat. After you summon a Beast, improve this permanently.
//!
//! The bonus is +1/+1 and +1/+1 more per improvement (`stacks`) **(engine decision)**.
//! `trinket_start_of_combat`: the friendly Beasts get it for the combat (the combat's copies);
//! `trinket_combat_summon`: a Beast summoned later in the combat gets it too, then improves it.
//! `trinket_tavern_summon`: a Beast summoned in the Tavern (played, or summoned by a card)
//! improves it.

use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::{CardId, Tribe, TrinketState, Unit};

pub const ID: CardId = 130896;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_start_of_combat(start_of_combat)
            .on_trinket_combat_summon(combat_summon)
            .on_trinket_tavern_summon(tavern_summon),
    )]
}

fn bonus(trinket: &TrinketState) -> i32 {
    1 + trinket.stacks
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let Some(trinket) = super::state(ctx.own.auras, ID) else {
        return;
    };
    let bonus = bonus(trinket);
    for pos in 0..ctx.own.board.len() {
        let unit = &ctx.own.board[pos];
        if unit.health > 0 && unit.tribe.matches(Tribe::Beast) {
            ctx.own.buff_unit(pos, bonus, bonus, "Fang Anklet");
        }
    }
}

fn combat_summon(trinket: &mut TrinketState, summoned: &mut Unit) {
    if summoned.tribe.matches(Tribe::Beast) {
        let bonus = bonus(trinket);
        summoned.add_stats(bonus, bonus);
        trinket.stacks += 1;
    }
}

fn tavern_summon(trinket: &mut TrinketState, summoned: &mut Unit) {
    if summoned.tribe.matches(Tribe::Beast) {
        trinket.stacks += 1;
    }
}
