//! `Illidan Stormrage` (`TB_BaconShop_HERO_08`) — Armor 18 (Duos 16).
//!
//! Hero Power: `Wingmen` (`TB_BaconShop_HP_069`), 0 Gold: Start of Combat: Your left- and right-most minions gain +2/+1 and attack immediately.
//!
//! Passive. At Start of Combat the left-most minion gains +2/+1, then the right-most; then the
//! left-most attacks, then the right-most (immediate attacks: they don't move the attack pointer,
//! `docs/combat.md` §5.4). With one minion, it gains +2/+1 once and attacks once (confirmed).

use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::{CardId, HeroId};

pub const ID: HeroId = 57634;
pub const POWER: CardId = 61851;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY.on_hero_start_of_combat(start_of_combat),
    )]
}

/// Start of Combat: the left- and right-most minions gain +2/+1 and attack immediately.
fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let n = ctx.own.board.len();
    let wingmen: &[usize] = match n {
        0 => return,
        1 => &[0],
        _ => &[0, n - 1],
    };
    for &i in wingmen {
        ctx.own.buff_unit(i, 2, 1, "Wingmen");
    }
    for &i in wingmen {
        let id = ctx.own.board[i].id;
        ctx.own.pending_attacks.push(id);
    }
}
