//! `Felboar` (`BG28_633`) — Tier 5 Demon / Quilboar (`2/6`).
//!
//! After you cast 3 spells, consume a minion in the Tavern to gain (`double` if Golden) its stats.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 519;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Felboar", 2, 6, 5)
        .with_tribe(Tribe::DemonQuilboar)
        .on_after_spell_cast(after_spell_cast)
}

pub fn after_spell_cast(
    state: &mut TavernState,
    self_idx: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let felboar = &mut state.board[self_idx];
    felboar.felboar_spell_progress += 1;
    if felboar.felboar_spell_progress < 3 {
        return;
    }
    felboar.felboar_spell_progress -= 3;
    let mult = felboar.golden_mult();
    let shop_minions: Vec<usize> = state
        .shop
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.is_spell)
        .map(|(i, _)| i)
        .collect();
    if shop_minions.is_empty() {
        return;
    }
    let pick = if shop_minions.len() == 1 {
        shop_minions[0]
    } else {
        shop_minions[rng.below(shop_minions.len())]
    };
    let consumed = state.shop.remove(pick);
    pool.return_unit(&consumed);
    state.board[self_idx].add_stats(consumed.attack * mult, consumed.health * mult);
}
