//! `Lionfish Portrait` (`BG36_MagicItem_201`) — Lesser, 1 Gold, Beast.
//!
//! Get a Lurking Lionfish. Whenever a friendly Beast attacks, give it +4/+2.
//!
//! Picked, it gives a plain `Lurking Lionfish` from the catalog (`docs/trinkets.md` §5).
//! `trinket_friendly_attack`: a friendly Beast that attacks gains +4/+2 before it strikes, in
//! combat or in the Tavern (a Fishbait attack).

use crate::cards::minions::lurking_lionfish;
use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Tribe, UnitId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133388;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_friendly_attack(friendly_attack),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, lurking_lionfish::ID);
}

fn friendly_attack(ctx: &mut BoardCtx<'_>, attacker_id: UnitId) {
    if let Some(pos) = ctx.board.iter().position(|u| u.id == attacker_id) {
        if ctx.board[pos].tribe.matches(Tribe::Beast) {
            ctx.buff_unit(pos, 4, 2, "Lionfish Portrait");
        }
    }
}
