//! `Deathly Phylactery` (`BG30_MagicItem_700`) — Lesser, 0 Gold, Beast, Undead.
//!
//! Discover a Deathrattle minion. Your first Deathrattle each combat triggers an extra time.
//!
//! As picked, a Discover of Deathrattle minions of the player's Tier or lower, from the pool.
//! `trinket_extra_deathrattles`: the first friendly minion each combat whose Deathrattles trigger
//! (on its death, or triggered by `Deathstrider` or `Herald Sticker`) triggers them once more
//! (`armed`, which starts again every combat).

use crate::cards::CardHooks;
use crate::model::{CardId, TrinketState, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 117794;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_combat_start(combat_start)
            .on_trinket_extra_deathrattles(extra),
    )]
}

fn bought(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let tier = state.tavern_tier;
    let mut opts = pool.draw_discover_filtered(tier, 3, crate::cards::is_deathrattle_minion, rng);
    for opt in &mut opts {
        state.apply_global_unit_auras(opt);
    }
    state.push_discover(opts);
}

fn combat_start(trinket: &mut TrinketState) {
    trinket.armed = 0;
}

fn extra(trinket: &mut TrinketState, unit: &Unit) -> u32 {
    if trinket.armed > 0 || !super::has_deathrattle(unit) {
        return 0;
    }
    trinket.armed = 1;
    1
}
