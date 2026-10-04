//! Player-level effects: persistent effects recorded in [`PlayerAuras`] (by spells and
//! "this game" card text) that act on their own rather than through a minion's hooks.
//!
//! The dispatch functions in the parent module call the phase entry points here; the Tavern
//! and Combat engines never name these effects.

use crate::cards::{apply_combat_summon_modifiers, tokens, BoardCtx};
use crate::combat::MAX_BOARD_SIZE;
use crate::events::Event;
use crate::model::{Keyword, PlayerAuras, Tribe, Unit};

/// Apply the player-wide "wherever this is" stat auras to `unit` (the Undead Attack bonus from
/// `Nerubian Deathswarmer`).
pub fn sync_unit_auras(unit: &mut Unit, auras: &PlayerAuras) {
    if unit.tribe.matches(Tribe::Undead) {
        let diff = auras.undead_bonus_attack - unit.undead_attack_applied;
        if diff != 0 {
            unit.undead_attack_applied = auras.undead_bonus_attack;
            unit.add_stats(diff, 0);
        }
    }
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
