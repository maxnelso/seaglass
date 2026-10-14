//! `Flobbidinous Floop` (`BGDUO_HERO_101`) — Duos Armor 15.
//!
//! Hero Power: `Glorious Gloop` (`BGDUO_HERO_101p`), 0 Gold: Choose a friendly minion. Start of Combat: Transform it into your teammate's highest-Tier minion.
//!
//! Active, a friendly minion, once per turn; Duos only. Using it marks the minion
//! (`Unit::marked`, until the next combat; confirmed). At Start of Combat the marked minion, if it
//! is on the board, transforms into a copy of the minion with the highest Tier in the teammate's
//! warband as it entered the combat (`docs/duos.md` §2; a tie at random, one draw), as `Sandy`
//! copies: its current stats, keywords and Golden status, the marked minion's place and id, the
//! player's "wherever this is" auras, a `UnitTransformed` event. With no teammate minion (Solo),
//! nothing happens. After combat the minion is itself again (`docs/combat.md` §1).

use crate::cards::{self, ActivateTargetKind, CardHooks, HeroCombatCtx};
use crate::events::Event;
use crate::heroes::mark_minion;
use crate::model::{CardId, HeroId};

pub const ID: HeroId = 104671;
pub const POWER: CardId = 104672;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(mark_minion)
            .with_activate_target(ActivateTargetKind::BoardAny)
            .on_hero_start_of_combat(start_of_combat),
    )]
}

/// Start of Combat: the marked minion transforms into the teammate's highest-Tier minion.
fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let ctx = &mut ctx.own;
    let Some(pos) = ctx.board.iter().position(|u| u.marked && u.health > 0) else {
        return;
    };
    let Some(top) = ctx.teammate_board.iter().map(|u| u.tavern_tier).max() else {
        return;
    };
    let tied: Vec<usize> = (0..ctx.teammate_board.len())
        .filter(|&i| ctx.teammate_board[i].tavern_tier == top)
        .collect();
    let pick = if tied.len() == 1 {
        tied[0]
    } else {
        tied[ctx.rng.below(tied.len())]
    };
    let id = ctx.board[pos].id;
    let mut copy = ctx.teammate_board[pick].clone();
    copy.id = id;
    copy.marked = false;
    if let Some(combat_start) = cards::hooks(copy.card_id).combat_start {
        combat_start(&mut copy);
    }
    cards::sync_unit_auras(&mut copy, ctx.auras);
    copy.sync_max_stats();
    cards::check_stat_thresholds(&mut copy);
    ctx.events.push(Event::UnitTransformed {
        side: ctx.side,
        unit: id,
        name: copy.name.clone(),
        attack: copy.attack,
        health: copy.health,
    });
    ctx.board[pos] = copy;
}
