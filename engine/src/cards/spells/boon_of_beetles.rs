//! `Boon of Beetles` — Tier 4 Tavern spell (`1` Gold).
//!
//! Summon 2 Taunt Beetles in combat, each as soon as there is room.

use super::{record_effect, spell, SPELL_BOON_OF_BEETLES};
use crate::cards::{apply_combat_summon_modifiers, tokens, BoardCtx, CardHooks};
use crate::combat::MAX_BOARD_SIZE;
use crate::events::Event;
use crate::model::{EffectDuration, Keyword, PlayerEffect, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast).on_player_combat_space(combat_space)
}

/// `Boon of Beetles`: summon 2 Taunt Beetles in combat, each as soon as there is room.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    record_effect(state, SPELL_BOON_OF_BEETLES, 2, EffectDuration::Game);
}

/// `Boon of Beetles`: whenever there is room on the board in combat, summon a Taunt Beetle at
/// the end of it (one per stack).
fn combat_space(ctx: &mut BoardCtx<'_>, effect: &mut PlayerEffect) {
    while effect.stacks > 0 && ctx.board.len() < MAX_BOARD_SIZE {
        effect.stacks -= 1;
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
