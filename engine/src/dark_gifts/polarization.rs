//! `Polarization` (`BG36_MidGameEffect_000t65`) — turns 5–9, weight 200.
//!
//! At the end of your turn, Magnetize a random Mech to this.
//!
//! `gift_end_of_turn`: on the board, at the end of each turn (each of its repeats), a random
//! Magnetic Mech of the player's Tier or lower from the game's catalog (one draw) is Magnetized
//! onto it by an effect (`cards::magnetize`). Only on Mechs, and not below Tavern Tier 3
//! (`docs/dark_gifts.md` §5.2).

use crate::cards::{self, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133344;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_gift_end_of_turn(end_of_turn)
            .on_gift_offerable(offerable),
    )]
}

fn end_of_turn(
    state: &mut TavernState,
    pos: usize,
    _: usize,
    repeats: u32,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let card_id = state.board[pos].card_id;
    for _ in 0..repeats {
        if state.board.get(pos).is_none_or(|u| u.card_id != card_id) {
            return;
        }
        let tier = state.tavern_tier;
        let mechs: Vec<&CardTemplate> = cards::game_templates(&state.auras)
            .iter()
            .filter(|t| t.tavern_tier <= tier && t.magnetic && t.tribe.matches(Tribe::Mech))
            .collect();
        if mechs.is_empty() {
            return;
        }
        let mut mech = mechs[rng.below(mechs.len())].instantiate();
        cards::magnetize(state, &mut mech, pos, pool, rng);
    }
}

fn offerable(ctx: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    t.tribe.matches(Tribe::Mech) && ctx.tavern_tier >= 3
}
