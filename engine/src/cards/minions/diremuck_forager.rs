//! `Diremuck Forager` (`BG27_556`) — Tier 3 Murloc (`4/5`).
//! **Start of Combat:** When you have space, summon the highest-Attack (`two highest-Attack` if Golden) Murloc(s) from your hand for this combat only.
//!
//! Each summon takes the highest-Attack Murloc in hand not summoned yet this combat, picked at
//! random among those tied for it (none left: nothing). With room at Start of Combat it goes to
//! the right of this; otherwise it waits and happens at the right end of the board as soon as a
//! slot opens later in the combat, one per open slot, even if this has died by then.

use crate::cards::{BoardCtx, CardTemplate};
use crate::combat::MAX_BOARD_SIZE;
use crate::model::{CardId, EffectDuration, PlayerEffect, Tribe, UnitId};

pub const ID: CardId = 311;
pub const NAME: &str = "Diremuck Forager";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 5, 3)
        .with_tribe(Tribe::Murloc)
        .on_start_of_combat(on_start_of_combat)
        .on_player_combat_space(summon_waiting)
        // Summons still waiting when the combat ends are lost.
        .on_player_after_combat(|_, effect, _| effect.stacks = 0)
}

/// Summon to the right of this while there is room; the summons that find the board full wait
/// for a slot (this card's player effect, one stack per summon).
pub fn on_start_of_combat(ctx: &mut BoardCtx<'_>, source_id: UnitId, is_golden: bool) {
    let count = if is_golden { 2 } else { 1 };
    let Some(source_pos) = ctx.board.iter().position(|u| u.id == source_id) else {
        return;
    };
    let mut insert_pos = source_pos + 1;
    let mut waiting = 0;
    for _ in 0..count {
        if ctx.board.len() >= MAX_BOARD_SIZE {
            waiting += 1;
        } else if let Some(pos) = summon_highest_murloc(ctx, source_id, insert_pos) {
            insert_pos = pos + 1;
        }
    }
    ctx.auras.add_effect(ID, waiting, EffectDuration::Turn);
}

/// Room opened on the board in combat: the waiting summons happen at its right end, one per open
/// slot.
fn summon_waiting(ctx: &mut BoardCtx<'_>, effect: &mut PlayerEffect) {
    while effect.stacks > 0 && ctx.board.len() < MAX_BOARD_SIZE {
        effect.stacks -= 1;
        // The Forager may be gone: the summon is logged as its own source.
        let source_id = *ctx.next_id;
        let end = ctx.board.len();
        summon_highest_murloc(ctx, source_id, end);
    }
}

/// Summon a copy of the highest-Attack Murloc in hand not summoned yet this combat (picked at
/// random among those tied for it) at board slot `pos`. Returns its slot, or `None` if there is
/// no such Murloc or no room.
fn summon_highest_murloc(ctx: &mut BoardCtx<'_>, source_id: UnitId, pos: usize) -> Option<usize> {
    let murlocs: Vec<usize> = (0..ctx.hand.len())
        .filter(|&i| {
            let card = &ctx.hand[i];
            !ctx.hand_summoned.get(i).copied().unwrap_or(true)
                && !card.is_spell
                && card.tribe.matches(Tribe::Murloc)
        })
        .collect();
    let best_attack = murlocs.iter().map(|&i| ctx.hand[i].attack).max()?;
    let tied: Vec<usize> = murlocs
        .into_iter()
        .filter(|&i| ctx.hand[i].attack == best_attack)
        .collect();
    let hand_idx = if tied.len() == 1 {
        tied[0]
    } else {
        tied[ctx.rng.below(tied.len())]
    };
    let copy = ctx.hand[hand_idx].clone();
    ctx.cursor = pos;
    let slot = ctx.summon_as(source_id, copy, NAME)?;
    ctx.hand_summoned[hand_idx] = true;
    Some(slot)
}
