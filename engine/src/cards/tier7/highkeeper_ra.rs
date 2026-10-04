//! `Highkeeper Ra` (`BG34_319`) — Tier 7 Neutral (`6/6`).
//!
//! Battlecry, Deathrattle, and Rally: Get a (`2` if Golden) random Tier 6 minion(s).

use crate::cards::{self, tier6, CardTemplate, DeathrattleContext};
use crate::model::{CardId, PlayerAuras, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 704;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Highkeeper Ra", 6, 6, 7).with_tribe(Tribe::None)
}

fn draw_random_tier6_minion(auras: &PlayerAuras, rng: &mut Rng) -> Unit {
    let pool = tier6::catalog();
    let idx = rng.below(pool.len());
    let mut u = pool[idx].instantiate();
    cards::sync_unit_auras(&mut u, auras);
    u
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, rng: &mut Rng) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        if state.hand.len() >= 10 {
            break;
        }
        let u = draw_random_tier6_minion(&state.auras, rng);
        state.add_to_hand(u);
    }
}

pub fn on_deathrattle(ctx: &mut DeathrattleContext<'_>, is_golden: bool) {
    let count = if is_golden { 2 } else { 1 };
    for _ in 0..count {
        if ctx.hand.len() >= 10 {
            break;
        }
        let u = draw_random_tier6_minion(ctx.auras, ctx.rng);
        ctx.add_to_hand(u);
    }
}

pub fn on_rally(
    is_golden: bool,
    auras: &PlayerAuras,
    generated_hand: &mut Vec<Unit>,
    rng: &mut Rng,
) {
    let count = if is_golden { 2 } else { 1 };
    for _ in 0..count {
        let u = draw_random_tier6_minion(auras, rng);
        generated_hand.push(u);
    }
}
