//! `Time Management` — Tier 3 Tavern spell (`4` Gold).
//!
//! Choose One - give your minions +2/+2 now; or twice next turn.

use super::{record_effect, spell, SPELL_TIME_MANAGEMENT};
use crate::cards::tokens::{make_choice_option, CHOICE_TIME_MGMT_LATER, CHOICE_TIME_MGMT_NOW};
use crate::cards::CardHooks;
use crate::model::{EffectDuration, PlayerEffect, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast).on_player_turn_start(turn_start)
}

/// `Time Management`: Choose One - give your minions +2/+2 now; or twice next turn.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    let opt0 = make_choice_option(CHOICE_TIME_MGMT_NOW, "Hurry Up (+2/+2 now)", false);
    let opt1 = make_choice_option(
        CHOICE_TIME_MGMT_LATER,
        "Do It Later (+2/+2 twice next turn)",
        false,
    );
    state.resolve_choose_one(opt0, opt1, pool, rng);
}

/// `Time Management` (Do It Later): at the start of next turn, give your minions (board and
/// hand) +2/+2 per stack.
fn turn_start(state: &mut TavernState, effect: &mut PlayerEffect) {
    let stacks = std::mem::take(&mut effect.stacks);
    let (atk, hp) = state.auras.spell_stat_buff(2, 2);
    for _ in 0..stacks {
        for b in &mut state.board {
            b.add_stats(atk, hp);
        }
        for h in &mut state.hand {
            if !h.is_spell {
                h.add_stats(atk, hp);
            }
        }
    }
}

/// `Time Management` (Hurry Up): give your minions (board and hand) +2/+2 now.
pub fn choose_now(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(2, 2);
    for b in &mut state.board {
        b.add_stats(atk, hp);
    }
    for h in &mut state.hand {
        if !h.is_spell {
            h.add_stats(atk, hp);
        }
    }
}

/// `Time Management` (Do It Later): +2/+2 twice next turn.
pub fn choose_later(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    record_effect(state, SPELL_TIME_MANAGEMENT, 2, EffectDuration::Game);
}
