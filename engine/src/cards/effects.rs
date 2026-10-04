//! Player-level effects: persistent effects recorded in [`PlayerAuras`] (by spells and
//! "this game" card text) that act on their own rather than through a minion's hooks.
//!
//! The dispatch functions in the parent module call the phase entry points here; the Tavern
//! and Combat engines never name these effects.

use crate::cards::{apply_combat_summon_modifiers, tokens, BoardCtx};
use crate::combat::MAX_BOARD_SIZE;
use crate::events::Event;
use crate::model::{Keyword, PlayerAuras, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

/// Stats the player-wide "wherever this is" auras give `unit` (the Undead Attack bonus from
/// `Nerubian Deathswarmer`).
pub fn aura_bonus(unit: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    if unit.tribe.matches(Tribe::Undead) {
        (auras.undead_bonus_attack, 0)
    } else {
        (0, 0)
    }
}

/// After a shop `Refresh` (including the start-of-turn one): buff the new shop and add Fodder.
pub fn after_shop_refresh(state: &mut TavernState, rng: &mut Rng) {
    buff_refreshed_shop(state, rng);
    spawn_refresh_fodder(state, rng);
}

/// Start of Combat, after the copies summoned from hand and before minion triggers.
pub fn start_of_combat(ctx: &mut BoardCtx<'_>) {
    summon_boon_of_beetles(ctx);
}

/// After a round of combat deaths resolved for one side.
pub fn after_combat_deaths(ctx: &mut BoardCtx<'_>) {
    summon_boon_of_beetles(ctx);
}

/// `Boon of Beetles`: summon a Taunt Beetle at the end of the board for each remaining charge,
/// while there is room.
fn summon_boon_of_beetles(ctx: &mut BoardCtx<'_>) {
    while ctx.auras.boon_of_beetles_charges > 0 && ctx.board.len() < MAX_BOARD_SIZE {
        ctx.auras.boon_of_beetles_charges -= 1;
        let mut beetle = tokens::make_beetle(false, ctx.auras).with_keyword(Keyword::Taunt);
        beetle.id = *ctx.next_id;
        *ctx.next_id += 1;
        apply_combat_summon_modifiers(ctx.board, ctx.auras, *ctx.beast_bonus_atk, &mut beetle);
        ctx.events.push(Event::UnitSummoned {
            side: ctx.side,
            source: beetle.id,
            unit: beetle.id,
            name: beetle.name.clone(),
            attack: beetle.attack,
            health: beetle.health,
            reason: "Boon of Beetles",
        });
        ctx.board.push(beetle);
    }
}

/// Resolve `Waveling`, `Easterly Winds`, `En-Djinn Blazer`, and `Blood Gem Barrage` buffs on shop `Refresh`.
fn buff_refreshed_shop(state: &mut TavernState, rng: &mut Rng) {
    for _ in 0..state.auras.waveling_stacks {
        let minion_indices: Vec<usize> = state
            .shop
            .iter()
            .enumerate()
            .filter(|(_, u)| !u.is_spell)
            .map(|(i, _)| i)
            .collect();
        if !minion_indices.is_empty() {
            let idx = if minion_indices.len() == 1 {
                minion_indices[0]
            } else {
                minion_indices[rng.below(minion_indices.len())]
            };
            state.shop[idx].add_stats(4, 4);
        }
    }
    let random_buffs = state.auras.refresh_random_buffs.clone();
    for (atk, hp) in random_buffs {
        let minion_indices: Vec<usize> = state
            .shop
            .iter()
            .enumerate()
            .filter(|(_, u)| !u.is_spell)
            .map(|(i, _)| i)
            .collect();
        if !minion_indices.is_empty() {
            let idx = if minion_indices.len() == 1 {
                minion_indices[0]
            } else {
                minion_indices[rng.below(minion_indices.len())]
            };
            state.shop[idx].add_stats(atk, hp);
        }
    }
    if state.auras.blood_gem_barrage_stacks > 0 {
        let gems = 2 * state.auras.blood_gem_barrage_stacks;
        for u in &mut state.shop {
            if !u.is_spell {
                u.play_blood_gems(gems, &state.auras);
            }
        }
    }
}

/// Resolve `Laboratory Assistant` / `Trapped Clapper` Fodder spawns on a shop `Refresh`.
fn spawn_refresh_fodder(state: &mut TavernState, rng: &mut Rng) {
    let fodder_count = state.auras.fodder_per_refresh[0];
    state.auras.fodder_per_refresh = [
        state.auras.fodder_per_refresh[1],
        state.auras.fodder_per_refresh[2],
        0,
    ];
    if fodder_count == 0 {
        return;
    }

    for _ in 0..fodder_count {
        let mut fodder = tokens::make_demon_fodder(false);
        state.apply_shop_auras(&mut fodder);
        if state.auras.blood_gem_barrage_stacks > 0 {
            fodder.play_blood_gems(2 * state.auras.blood_gem_barrage_stacks, &state.auras);
        }

        let demon_indices: Vec<usize> = state
            .board
            .iter()
            .enumerate()
            .filter(|(_, u)| u.tribe.matches(Tribe::Demon))
            .map(|(i, _)| i)
            .collect();

        if !demon_indices.is_empty() {
            let idx = if demon_indices.len() == 1 {
                demon_indices[0]
            } else {
                demon_indices[rng.below(demon_indices.len())]
            };
            state.board[idx].add_stats(fodder.attack, fodder.health);
        } else {
            state.shop.push(fodder);
        }
    }
}
