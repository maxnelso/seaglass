//! `Big Brann Play` (`BGS_Treasures_030`) — Tier 2 Darkmoon Prize.
//!
//! Your Battlecries trigger an extra time this turn.
//!
//! A turn-long player effect (one stack per cast): every friendly Battlecry in the Tavern this
//! turn triggers once more per stack (`player_extra_battlecries`), on top of `Brann Bronzebeard`'s
//! and the trinkets' (engine decision: the extra triggers add).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, EffectDuration, PlayerEffect, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 66458;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast).on_player_extra_battlecries(extra))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.auras.add_effect(ID, 1, EffectDuration::Turn);
}

fn extra(effect: &PlayerEffect) -> u32 {
    effect.stacks
}
