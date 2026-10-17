//! `Incubation` (`BG36_MidGameEffect_000t4`) — turns 6–8, weight 100.
//!
//! +4/+4. In 2 turns, double this minion's stats.
//!
//! Given: +4/+4. At the 2nd start of turn after it was given, on the board or in the hand, it gains
//! its Attack and Health, once (`counter`: the turns left; `docs/dark_gifts.md` §11.2). Only on
//! minions with a type (§5.2). A stat gift.

use crate::cards::{CardFlags, CardHooks, CardTemplate};
use crate::dark_gifts::{OfferCtx, Zone};
use crate::model::{CardId, DarkGift, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 132202;

/// The turns until it doubles.
const TURNS: i32 = 2;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_GIVES_STATS)
            .on_gift_given(given)
            .on_gift_turn_start(turn_start)
            .on_gift_offerable(offerable),
    )]
}

fn given(unit: &mut Unit, gift: &mut DarkGift) {
    unit.add_stats(4, 4);
    gift.counter = TURNS;
}

fn turn_start(
    state: &mut TavernState,
    zone: Zone,
    idx: usize,
    gift_idx: usize,
    _: &mut CardPool,
    _: &mut Rng,
) {
    let unit = match zone {
        Zone::Board => &mut state.board[idx],
        Zone::Hand => &mut state.hand[idx],
    };
    let gift = &mut unit.dark_gifts[gift_idx];
    if gift.counter <= 0 {
        return;
    }
    gift.counter -= 1;
    if gift.counter == 0 {
        let (atk, hp) = (unit.attack.max(0), unit.health.max(0));
        unit.add_stats(atk, hp);
    }
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    super::has_type(t)
}
