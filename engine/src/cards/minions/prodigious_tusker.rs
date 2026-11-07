//! `Prodigious Tusker` (`BG33_430`) — Tier 2 Quilboar (`2/5`).
//! Whenever another friendly minion attacks, this plays a (`2` if Golden) **Blood Gem(s)** on it.
//!
//! The Blood Gems it plays, in combat or when your minion attacks a `Fishbait` in the Tavern,
//! aren't spells you cast (`Felboar`, `Glambot`, ... ignore them; `docs/tavern.md` §7.4).

use crate::cards::{BoardCtx, CardTemplate};
use crate::events::Event;
use crate::model::{CardId, Tribe, UnitId};

pub const ID: CardId = 222;
pub const NAME: &str = "Prodigious Tusker";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 5, 2)
        .with_tribe(Tribe::Quilboar)
        .on_friendly_attack(on_friendly_attack)
}

pub fn on_friendly_attack(ctx: &mut BoardCtx<'_>, self_idx: usize, attacker_id: UnitId) {
    if ctx.board[self_idx].id == attacker_id {
        return;
    }
    let gems = ctx.board[self_idx].golden_mult() as u32;
    let Some(pos) = ctx.board.iter().position(|u| u.id == attacker_id) else {
        return;
    };
    let (atk_delta, hp_delta) =
        crate::cards::play_blood_gems_on_board_unit(ctx.board, pos, gems, ctx.auras, ctx.in_combat);
    if atk_delta != 0 || hp_delta != 0 {
        let attacker = &ctx.board[pos];
        ctx.events.push(Event::StatBuff {
            side: ctx.side,
            unit: attacker_id,
            atk_delta,
            hp_delta,
            attack: attacker.attack,
            health: attacker.health,
            reason: "ProdigiousTusker",
        });
    }
}
