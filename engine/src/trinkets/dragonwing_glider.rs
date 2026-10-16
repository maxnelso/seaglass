//! `Dragonwing Glider` (`BG30_MagicItem_900`) — Lesser, 1 Gold, Dragon.
//!
//! Whenever you play a card, give a friendly Dragon +4/+4.
//!
//! After each card played from the hand (a minion, a Magnetization, or a spell: `trinket_after_play`
//! and `trinket_after_spell` from the hand), a random Dragon on the board (one `rng.below(k)`)
//! gains +4/+4; nothing without one. Cards cast or summoned by effects don't count (confirmed).

use crate::cards::{CardHooks, Played};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

use super::SpellCast;

pub const ID: CardId = 110985;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_after_play(after_play)
            .on_trinket_after_spell(after_spell),
    )]
}

fn after_play(state: &mut TavernState, played: &Played, _: &mut CardPool, rng: &mut Rng) {
    if played.from_hand {
        buff_a_dragon(state, rng);
    }
}

fn after_spell(state: &mut TavernState, spell: &SpellCast, _: &mut CardPool, rng: &mut Rng) {
    if spell.from_hand {
        buff_a_dragon(state, rng);
    }
}

fn buff_a_dragon(state: &mut TavernState, rng: &mut Rng) {
    let dragons: Vec<usize> = (0..state.board.len())
        .filter(|&i| state.board[i].tribe.matches(Tribe::Dragon))
        .collect();
    if !dragons.is_empty() {
        let pick = dragons[rng.below(dragons.len())];
        state.board[pick].add_stats(4, 4);
    }
}
