//! `Surveyor Portrait` (`BG30_MagicItem_943`) — Greater, 1 Gold, Quilboar.
//!
//! Get a Hot-Air Surveyor. Blood Gems played from your hand give an extra +6/+6.
//!
//! Picked: a plain `Hot-Air Surveyor`. `trinket_after_spell`: a Blood Gem played from the hand on a
//! minion still on the board gives it +6/+6 more for each of its casts that resolved (the extra
//! casts too: `Hot-Air Surveyor`'s) **(engine decision**: added once the casts are done**)**.

use crate::cards::minions::hot_air_surveyor;
use crate::cards::{self, CardFlags, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};
use crate::trinkets::SpellCast;

pub const ID: CardId = 117856;

/// The extra stats each cast gives.
const STATS: i32 = 6;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_after_spell(after_spell),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, hot_air_surveyor::ID);
}

fn after_spell(state: &mut TavernState, cast: &SpellCast, _: &mut CardPool, _: &mut Rng) {
    if !cast.from_hand || !cards::hooks(cast.card.card_id).has(CardFlags::BLOOD_GEM) {
        return;
    }
    let Some(MinionSlot::Board(pos)) = cast.target else {
        return;
    };
    let stats = STATS.saturating_mul(cast.casts.min(i32::MAX as u32) as i32);
    if let Some(unit) = state.board.get_mut(pos) {
        unit.add_stats(stats, stats);
    }
}
