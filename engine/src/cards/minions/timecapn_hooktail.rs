//! `Timecap'n Hooktail` (`BG27_005`) — Tier 3 Dragon/Pirate (`1/4`).
//! Whenever you cast a Tavern spell, give your minions `+1` Attack (`+1` Attack twice if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 335;
pub const NAME: &str = "Timecap'n Hooktail";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 4, 3)
        .with_tribe(Tribe::DragonPirate)
        .on_spell_cast(on_spell_cast)
}

pub fn on_spell_cast(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let bonus_atk = state.board[self_idx].golden_mult();
    for u in &mut state.board {
        u.add_stats(bonus_atk, 0);
    }
}
