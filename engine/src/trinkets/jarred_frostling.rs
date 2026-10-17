//! `Jarred Frostling` (`BG30_MagicItem_952`) — Greater, 2 Gold, Elemental.
//!
//! Start of Combat: Give 3 friendly Elementals "Deathrattle: Summon a Flourishing Frostling".
//!
//! Step 1c: 3 different random friendly Elementals (every one if there are 3 or fewer) gain the
//! extra Deathrattle *Frosty Globe* (`BG30_MagicItem_952e`, `Unit::extra_deathrattles`) for the
//! combat: it summons a plain `Flourishing Frostling` (a minion in no pool, `docs/trinkets.md`
//! §11.9) in the dying minion's place.

use crate::cards::minions::flourishing_frostling;
use crate::cards::{self, BoardCtx, CardHooks, HeroCombatCtx};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 111500;
/// *Frosty Globe*: the Deathrattle the trinket gives.
pub const FROSTY_GLOBE: CardId = 111501;

/// The Elementals that get it.
const COUNT: usize = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            ID,
            CardHooks::EMPTY.on_trinket_start_of_combat(start_of_combat),
        ),
        (FROSTY_GLOBE, CardHooks::EMPTY.on_deathrattle(frosty_globe)),
    ]
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let own = &mut ctx.own;
    let mut elementals: Vec<usize> = (0..own.board.len())
        .filter(|&i| own.board[i].health > 0 && own.board[i].tribe.matches(Tribe::Elemental))
        .collect();
    while elementals.len() > COUNT {
        elementals.remove(own.rng.below(elementals.len()));
    }
    for i in elementals {
        own.board[i].extra_deathrattles.push(FROSTY_GLOBE);
    }
}

fn frosty_globe(dying: &Unit, ctx: &mut BoardCtx<'_>) {
    if let Some(template) = cards::template(flourishing_frostling::ID) {
        ctx.summon_as(dying.id, template.instantiate(), "Frosty Globe");
    }
}
