//! `Token of the Old Gods` (`BG30_MagicItem_416`) — Lesser, 2 Gold.
//!
//! Spellcraft: Choose a minion. Then choose a minion from a Tier higher to transform it into.
//!
//! Picked, and at each turn start: its Spellcraft spell, `Token of the Old Gods`
//! (`BG30_MagicItem_416t`), a temporary token spell (`docs/trinkets.md` §11.6). Cast on a friendly
//! minion: the trinket marks it (`Unit::trinket_marks`) and a Discover of 3 minions of its Tier
//! plus one (6 at most) from the pool follows, whose options the trinket keeps (`stored`, as
//! `Portable Factory`'s). Resolved (`trinket_after_discover`), the chosen minion leaves the hand
//! and the marked minion transforms into it, in its place (its copy back to the pool); a chosen
//! card that already made a Triple in the hand stays there **(engine decision)**.

use crate::cards::spells::{targeted, SPELLCRAFT};
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 111556;
/// Its Spellcraft spell.
pub const SPELL: CardId = 112813;

/// The Discover's options.
const OPTIONS: usize = 3;
/// The highest Tier it offers.
const MAX_TIER: u32 = 6;

/// The trinket's and its spell's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            ID,
            CardHooks::EMPTY
                .on_trinket_bought(give_spell)
                .on_trinket_turn_start(give_spell)
                .on_trinket_after_discover(after_discover),
        ),
        (SPELL, targeted(cast).with_flags(SPELLCRAFT)),
    ]
}

fn give_spell(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.add_to_hand_or_wait(super::spellcraft_spell(SPELL, "Token of the Old Gods"));
}

fn cast(state: &mut TavernState, _: &Unit, pos: usize, pool: &mut CardPool, rng: &mut Rng) {
    let Some(tier) = state
        .board
        .get(pos)
        .map(|u| (u.tavern_tier + 1).min(MAX_TIER))
    else {
        return;
    };
    super::mark(state, pos, ID);
    let options = pool.draw_discover_options(tier, OPTIONS, rng);
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.stored = options.clone();
        t.armed = u32::from(!options.is_empty());
    }
    state.push_discover(options);
}

fn after_discover(
    state: &mut TavernState,
    chosen: &Unit,
    offered: &[CardId],
    pool: &mut CardPool,
    _: &mut Rng,
) {
    if !super::own_discover(state, ID, offered) {
        return;
    }
    let Some(pos) = super::marked(state, ID) else {
        return;
    };
    super::unmark(state, ID);
    let Some(h) = state
        .hand
        .iter()
        .rposition(|u| u.card_id == chosen.card_id && !u.is_golden)
    else {
        return;
    };
    let new = state.hand.remove(h);
    let old = std::mem::replace(&mut state.board[pos], new);
    pool.return_unit(&old);
    state.sync_all_auras();
}
