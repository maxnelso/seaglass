//! `Tide Oracle Morgl` (`BG27_513`) — Tier 7 Murloc (`1/10`), in no pool.
//! **Poisonous**. When this attacks and kills a minion, give its maximum stats (`double` its
//! maximum stats if Golden) to a minion in your hand.
//!
//! Only `Morgl Portrait` gives it (`docs/trinkets.md` §11.9). Poisonous is Venomous that is never
//! used up (`CardFlags::POISONOUS`). Its `attack_kill` hook: when the minion it attacks dies of
//! the attack, the minion's highest Attack and Health (`max_attack`, `max_health`) go to a random
//! minion in the combat hand, as `Very Hungry Winterfinner`'s buff (kept after the combat).

use crate::cards::{BoardCtx, CardFlags, CardTemplate};
use crate::model::{CardId, Keyword, Tribe, Unit};

pub const ID: CardId = 10807;
pub const NAME: &str = "Tide Oracle Morgl";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 10, 7)
        .with_tribe(Tribe::Murloc)
        .with_keyword(Keyword::Venomous)
        .with_flags(CardFlags::POISONOUS)
        .on_attack_kill(attack_kill)
}

pub fn attack_kill(ctx: &mut BoardCtx<'_>, self_idx: usize, victim: &Unit) {
    let candidates: Vec<usize> = ctx
        .hand
        .iter()
        .enumerate()
        .filter(|(_, u)| !u.is_spell && !u.unplayable && u.lockbox_turns_left == 0)
        .map(|(i, _)| i)
        .collect();
    if candidates.is_empty() {
        return;
    }
    let idx = if candidates.len() == 1 {
        candidates[0]
    } else {
        candidates[ctx.rng.below(candidates.len())]
    };
    let mult = ctx.board[self_idx].golden_mult();
    let attack = victim.max_attack.max(victim.attack).saturating_mul(mult);
    let health = victim.max_health.saturating_mul(mult);
    ctx.hand[idx].add_stats(attack, health);
}
