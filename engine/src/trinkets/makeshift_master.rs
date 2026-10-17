//! `Makeshift Master` (`BG36_MagicItem_417`) — Greater, 3 Gold.
//!
//! Spellcraft: Choose a minion. After it gains stats outside combat this turn, your Deity also gains them.
//!
//! Picked, and at each turn start: its Spellcraft spell, `Makeshift Master` (`BG36_MagicItem_417t`),
//! a temporary token spell (`docs/trinkets.md` §11.6). Cast on a friendly minion: the trinket marks
//! it (`Unit::trinket_marks`; a new cast moves the mark) and remembers its stats (`counter`,
//! `stacks`). `trinket_after_action`: after each Tavern action this turn, the Attack and the Health
//! the marked minion gained since (each one's rise, auras too) go to the Deity (`auras.deity`)
//! **(engine decision**: measured after each action**)**. The turn start removes the mark.

use crate::cards::spells::{targeted, SPELLCRAFT};
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134758;
/// Its Spellcraft spell.
pub const SPELL: CardId = 134770;

/// The trinket's and its spell's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            ID,
            CardHooks::EMPTY
                .on_trinket_bought(give_spell)
                .on_trinket_turn_start(new_turn)
                .on_trinket_after_action(after_action),
        ),
        (SPELL, targeted(cast).with_flags(SPELLCRAFT)),
    ]
}

fn give_spell(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.add_to_hand_or_wait(super::spellcraft_spell(SPELL, "Makeshift Master"));
}

fn new_turn(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    super::unmark(state, ID);
    give_spell(state, pool, rng);
}

fn cast(state: &mut TavernState, _: &Unit, pos: usize, _: &mut CardPool, _: &mut Rng) {
    let Some((attack, health)) = state.board.get(pos).map(|u| (u.attack, u.health)) else {
        return;
    };
    super::mark(state, pos, ID);
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.counter = attack;
        t.stacks = health;
    }
}

fn after_action(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let Some((attack, health)) =
        super::marked(state, ID).map(|p| (state.board[p].attack, state.board[p].health))
    else {
        return;
    };
    let Some(t) = super::state_mut(&mut state.auras, ID) else {
        return;
    };
    let gained = (
        attack.saturating_sub(t.counter).max(0),
        health.saturating_sub(t.stacks).max(0),
    );
    t.counter = attack;
    t.stacks = health;
    state.auras.deity.attack = state.auras.deity.attack.saturating_add(gained.0);
    state.auras.deity.health = state.auras.deity.health.saturating_add(gained.1);
}
