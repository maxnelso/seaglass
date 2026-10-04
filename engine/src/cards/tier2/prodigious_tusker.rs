//! `Prodigious Tusker` (`BG33_430`) — Tier 2 Quilboar (`2/5`).
//! Whenever another friendly minion attacks, this plays a (`2` if Golden) **Blood Gem(s)** on it.

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
    let Some(attacker) = ctx.board.iter_mut().find(|u| u.id == attacker_id) else {
        return;
    };
    let (pre_atk, pre_hp) = (attacker.attack, attacker.health);
    attacker.play_blood_gems(gems, ctx.auras);
    if attacker.attack != pre_atk || attacker.health != pre_hp {
        ctx.events.push(Event::StatBuff {
            side: ctx.side,
            unit: attacker_id,
            atk_delta: attacker.attack - pre_atk,
            hp_delta: attacker.health - pre_hp,
            attack: attacker.attack,
            health: attacker.health,
            reason: "ProdigiousTusker",
        });
    }
}
