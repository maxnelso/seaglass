//! `Private Selection` (`BGDUO31_209t`) — Spellcraft token spell of `Private Chef`, Duos only.
//!
//! Choose a minion. Get a different random minion of its type, then Pass it (Golden: 2
//! different random minions, then Pass them).
//!
//! It is cast on a minion with a type, on the board or in the Tavern. The minions are drawn from
//! the pool, as `Chef's Choice`'s: of one of the target's types (a two-type target has both), of
//! the player's Tavern Tier or lower, another card than the target, all different cards. Each is
//! Passed at once, a free Pass that counts as the team's (`docs/duos_tavern.md` §8.3). A
//! temporary token spell, not a Tavern spell.

use super::{targeted_minion, SPELLCRAFT};
use crate::cards::CardHooks;
use crate::model::{TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted_minion(cast)
        .on_spell_target_filter(|unit| unit.tribe != Tribe::None)
        .with_flags(SPELLCRAFT)
}

/// `Private Selection`: Pass different random minions of the target's type (2 if Golden).
pub fn cast(
    state: &mut TavernState,
    spell: &Unit,
    target: MinionSlot,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(minion) = state.minion(target) else {
        return;
    };
    let (tribe, card_id) = (minion.tribe, minion.card_id);
    let count = spell.golden_mult() as usize;
    let drawn =
        pool.draw_discover_by_tribe_except(tribe, Some(card_id), state.tavern_tier, count, rng);
    for minion in drawn {
        state.auras.give_teammate(TeammateGift::Pass(minion));
    }
}
