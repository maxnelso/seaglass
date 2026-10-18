//! Player-level effects: persistent effects recorded in [`PlayerAuras`] (by spells and
//! "this game" card text) that act on their own rather than through a minion's hooks.
//!
//! Most are [`PlayerEffect`]s, which act through their card's `player_*` hooks; the entry points
//! here invoke those hooks in the order the effects were recorded. The rest are generic auras
//! (Tavern buffs on `Refresh`, the Undead stat bonus).
//!
//! The dispatch functions in the parent module call the phase entry points here; the Tavern
//! and Combat engines never name these effects.

use crate::cards::{hooks, BoardCtx, CardHooks, CombatSides};
use crate::events::Event;
use crate::model::{CombatResult, PlayerAuras, PlayerEffect, Side, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Stats the player-wide "wherever this is" auras give `unit` (the Undead bonus from
/// `Nerubian Deathswarmer` and `Butchering`; in Duos, the team's Passes for a unit a `Transport
/// Reactor` was Magnetized onto).
pub fn aura_bonus(unit: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    let passes = unit.pass_aura * auras.team_passes_this_game as i32;
    if unit.tribe.matches(Tribe::Undead) {
        (
            auras.undead_bonus_attack + passes,
            auras.undead_bonus_health + passes,
        )
    } else {
        (passes, passes)
    }
}

/// Hook contexts that carry the player's auras (and with them the player's effects).
trait HasAuras {
    fn player_auras(&mut self) -> &mut PlayerAuras;
}

impl HasAuras for TavernState {
    fn player_auras(&mut self) -> &mut PlayerAuras {
        &mut self.auras
    }
}

impl HasAuras for BoardCtx<'_> {
    fn player_auras(&mut self) -> &mut PlayerAuras {
        self.auras
    }
}

/// Invoke the player-effect hook selected by `get` for each of the player's effects whose card
/// has it, in the order the effects were recorded. Each hook works on a copy of its effect that is
/// written back afterwards, so hooks still see every effect (their own included) through `ctx`;
/// effects left without stacks then end.
fn notify_effects<C: HasAuras, F>(
    ctx: &mut C,
    get: impl Fn(&CardHooks) -> Option<F>,
    mut call: impl FnMut(&mut C, &mut PlayerEffect, F),
) {
    for i in 0..ctx.player_auras().effects.len() {
        let mut effect = ctx.player_auras().effects[i];
        if let Some(f) = get(hooks(effect.card_id)) {
            call(ctx, &mut effect, f);
            ctx.player_auras().effects[i] = effect;
        }
    }
    ctx.player_auras().effects.retain(|e| e.stacks > 0);
}

/// Start of turn: the player's effects' `player_turn_start` hooks.
pub fn turn_start(state: &mut TavernState) {
    notify_effects(
        state,
        |h| h.player_turn_start,
        |state, effect, f| f(state, effect),
    );
}

/// After a shop `Refresh` (including the start-of-turn one): buff the new shop, invoke the
/// player's effects' `player_after_refresh` hooks, then count down the effects that last a number
/// of Refreshes.
pub fn after_shop_refresh(state: &mut TavernState, rng: &mut Rng) {
    buff_refreshed_shop(state, rng);
    notify_effects(
        state,
        |h| h.player_after_refresh,
        |state, effect, f| f(state, effect, rng),
    );
    state.auras.count_down_refresh_effects();
}

/// Start of Combat for `side`, before any minion's trigger: the player's effects'
/// `player_start_of_combat` hooks.
pub fn start_of_combat(
    side: Side,
    board: &mut [Unit],
    auras: &mut PlayerAuras,
    enemy_board: &mut [Unit],
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    let mut sides = CombatSides {
        side,
        board,
        enemy_board,
        rng,
        events,
    };
    for effect in &mut auras.effects {
        if let Some(start_of_combat) = hooks(effect.card_id).player_start_of_combat {
            start_of_combat(&mut sides, effect);
        }
    }
    auras.effects.retain(|e| e.stacks > 0);
}

/// Room may have opened on the board in combat (at Start of Combat and after each round of
/// deaths): the player's effects' `player_combat_space` hooks.
pub fn combat_space(ctx: &mut BoardCtx<'_>) {
    notify_effects(
        ctx,
        |h| h.player_combat_space,
        |ctx, effect, f| f(ctx, effect),
    );
}

/// `true` if one of the player's effects acts at Start of Combat (a `player_start_of_combat` or
/// `player_combat_space` hook).
pub fn has_start_of_combat(auras: &PlayerAuras) -> bool {
    auras.effects.iter().any(acts_at_start_of_combat)
}

/// `true` if `effect` acts at Start of Combat: its card has a `player_start_of_combat` or
/// `player_combat_space` hook.
pub fn acts_at_start_of_combat(effect: &PlayerEffect) -> bool {
    let card = hooks(effect.card_id);
    card.player_start_of_combat.is_some() || card.player_combat_space.is_some()
}

/// A friendly minion is being summoned in combat: the player's effects' `player_combat_summon`
/// hooks.
pub fn combat_summon(auras: &PlayerAuras, summoned: &mut Unit) {
    for effect in &auras.effects {
        if let Some(combat_summon) = hooks(effect.card_id).player_combat_summon {
            combat_summon(effect, summoned);
        }
    }
}

/// After combat: the player's effects' `player_after_combat` hooks.
pub fn after_combat(state: &mut TavernState, result: CombatResult) {
    notify_effects(
        state,
        |h| h.player_after_combat,
        |state, effect, f| f(state, effect, result),
    );
}

/// The price of a minion in the Tavern, given `price` so far: the player's effects'
/// `player_minion_cost` hooks, in the order they were recorded.
pub fn minion_cost(auras: &PlayerAuras, price: u32) -> u32 {
    auras.effects.iter().fold(price, |price, effect| {
        hooks(effect.card_id)
            .player_minion_cost
            .map_or(price, |cost| cost(effect, price))
    })
}

/// The price of a spell in the Tavern, given `cost` so far: the player's effects'
/// `player_spell_cost` hooks, in the order they were recorded.
pub fn spell_cost(auras: &PlayerAuras, cost: u32) -> u32 {
    auras.effects.iter().fold(cost, |cost, effect| {
        hooks(effect.card_id)
            .player_spell_cost
            .map_or(cost, |spell_cost| spell_cost(effect, cost))
    })
}

/// The minions a rolled shop holds, given `slots` so far: the player's effects'
/// `player_shop_slots` hooks, in the order they were recorded.
pub fn shop_slots(state: &TavernState, slots: usize) -> usize {
    state.auras.effects.iter().fold(slots, |slots, effect| {
        hooks(effect.card_id)
            .player_shop_slots
            .map_or(slots, |shop_slots| shop_slots(effect, state, slots))
    })
}

/// The extra triggers the player's effects add to a friendly Battlecry in the Tavern (their
/// `player_extra_battlecries` hooks).
pub fn extra_battlecries(auras: &PlayerAuras) -> u32 {
    auras
        .effects
        .iter()
        .filter_map(|effect| {
            hooks(effect.card_id)
                .player_extra_battlecries
                .map(|f| f(effect))
        })
        .sum()
}

/// The player ends their turn: the player's effects' `player_end_turn` hooks.
pub fn end_turn(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    notify_effects(
        state,
        |h| h.player_end_turn,
        |state, effect, f| f(state, effect, pool, rng),
    );
}

/// Apply the player's generic `Refresh` auras to the new shop: buffs to a random minion
/// (`refresh_random_buffs`, in the order they were gained), then Blood Gems on every minion
/// (`refresh_blood_gems`).
fn buff_refreshed_shop(state: &mut TavernState, rng: &mut Rng) {
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
    let gems = state.auras.refresh_blood_gems;
    if gems > 0 {
        for u in &mut state.shop {
            if !u.is_spell {
                u.play_blood_gems(gems, &state.auras);
            }
        }
    }
}
