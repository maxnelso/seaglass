//! `Sandy` (`BGDUO_125`) — Tier 7 Neutral (`1/1`), Duos only.
//! **Start of Combat:** Transform into a copy of your teammate's highest-Health minion (Golden
//! `BGDUO_125_G`, `2/2`: into a Golden copy).
//!
//! It reads the teammate's warband as it entered the combat (`BoardCtx::teammate_board`), even if
//! it has fought, and copies the minion with the most Health there (a tie: one at random). The
//! copy has its current stats, keywords, card state and Golden status (a Golden Sandy makes it
//! Golden), and takes Sandy's place and combat id; the player's "wherever this is" auras replace
//! the teammate's. It is not a summon, and its own Start of Combat doesn't trigger. With no
//! minion to copy, Sandy stays as it is. After the combat it is Sandy again: a minion that
//! transformed in combat keeps nothing from it (`docs/duos_tavern.md` §8.2).

use crate::cards::{self, BoardCtx, CardTemplate};
use crate::events::Event;
use crate::model::{CardId, UnitId};

pub const ID: CardId = 10701;
pub const NAME: &str = "Sandy";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 1, 7).on_start_of_combat(on_start_of_combat)
}

pub fn on_start_of_combat(ctx: &mut BoardCtx<'_>, self_id: UnitId, is_golden: bool) {
    let Some(pos) = ctx.board.iter().position(|u| u.id == self_id) else {
        return;
    };
    let Some(most) = ctx.teammate_board.iter().map(|u| u.health).max() else {
        return;
    };
    let tied: Vec<usize> = (0..ctx.teammate_board.len())
        .filter(|&i| ctx.teammate_board[i].health == most)
        .collect();
    let pick = if tied.len() == 1 {
        tied[0]
    } else {
        tied[ctx.rng.below(tied.len())]
    };
    let mut copy = ctx.teammate_board[pick].clone();
    copy.id = self_id;
    if is_golden && !copy.is_golden {
        copy.make_golden();
    }
    if let Some(combat_start) = cards::hooks(copy.card_id).combat_start {
        combat_start(&mut copy);
    }
    cards::sync_unit_auras(&mut copy, ctx.auras);
    copy.sync_max_stats();
    cards::check_stat_thresholds(&mut copy);
    ctx.events.push(Event::UnitTransformed {
        side: ctx.side,
        unit: self_id,
        name: copy.name.clone(),
        attack: copy.attack,
        health: copy.health,
    });
    ctx.board[pos] = copy;
}
