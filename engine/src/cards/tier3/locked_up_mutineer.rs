//! `Locked-up Mutineer` (`BG36_521`) — Tier 3 Pirate (`6/3`).
//! **Deathrattle:** Get a `Lockbox`. If you already have one, it opens `1` (`2` if Golden) turn(s) sooner instead.

use crate::cards::{tokens, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 322;
pub const NAME: &str = "Locked-up Mutineer";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 6, 3, 3)
        .with_tribe(Tribe::Pirate)
        .on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let accel = if dying.is_golden { 2 } else { 1 };
    if let Some(idx) = ctx
        .hand
        .iter()
        .position(|c| c.card_id == tokens::SPELL_LOCKBOX && c.lockbox_turns_left > 0)
    {
        ctx.hand[idx].lockbox_turns_left =
            ctx.hand[idx].lockbox_turns_left.saturating_sub(accel);
    } else {
        ctx.add_to_hand(tokens::make_lockbox());
    }
}
