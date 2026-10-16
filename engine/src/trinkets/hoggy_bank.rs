//! `Hoggy Bank` (`BG30_MagicItem_411`) — Lesser, 0 Gold, Quilboar.
//!
//! Start of Combat: Give your Quilboar "Deathrattle: Get 2 Blood Gems."
//!
//! Step 1c (`docs/trinkets.md` §7.2): every friendly Quilboar gains the extra Deathrattle *Gem in
//! the Bank* (`BG30_MagicItem_411e`, `Unit::extra_deathrattles`, as Hero Powers give them:
//! `docs/heroes.md` §4), for the combat. When it dies, 2 `Blood Gem`s go to the combat hand (if
//! it has room), and come back with it.

use crate::cards::{tokens, BoardCtx, CardHooks, HeroCombatCtx};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 111514;

/// *Gem in the Bank*: the Deathrattle the trinket gives.
pub const GEM_IN_THE_BANK: CardId = 111516;

/// Blood Gems the Deathrattle gives.
const GEMS: usize = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            ID,
            CardHooks::EMPTY.on_trinket_start_of_combat(start_of_combat),
        ),
        (GEM_IN_THE_BANK, CardHooks::EMPTY.on_deathrattle(gems)),
    ]
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    for unit in ctx.own.board.iter_mut() {
        if unit.tribe.matches(Tribe::Quilboar) {
            unit.extra_deathrattles.push(GEM_IN_THE_BANK);
        }
    }
}

fn gems(_: &Unit, ctx: &mut BoardCtx<'_>) {
    for _ in 0..GEMS {
        ctx.add_to_hand(tokens::make_blood_gem());
    }
}
