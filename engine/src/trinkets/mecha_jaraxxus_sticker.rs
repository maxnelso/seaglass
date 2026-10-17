//! `Mecha-Jaraxxus Sticker` (`BG30_MagicItem_942`) — Greater, 3 Gold, Demon, Mech.
//!
//! Get 2 random Magnetic Mecha-Demons. At the start of each turn, get 2 more.
//!
//! Picked, and at each turn start: 2 of the 4 Mecha-Demons (`Rusted Reggie`, `Magtheridon
//! Prime`, `Baltharak`, `Shield of the Legion`: minions in no pool, `docs/trinkets.md` §11.9),
//! each one at random (the same one can come twice), to the hand or waiting for room.

use crate::cards::minions::{baltharak, magtheridon_prime, rusted_reggie, shield_of_the_legion};
use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 117855;

/// The Mecha-Demons it gives each time.
const COUNT: usize = 2;

/// The 4 Mecha-Demons.
const MECHA_DEMONS: [CardId; 4] = [
    rusted_reggie::ID,
    magtheridon_prime::ID,
    baltharak::ID,
    shield_of_the_legion::ID,
];

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(give)
            .on_trinket_turn_start(give),
    )]
}

fn give(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    for _ in 0..COUNT {
        let id = MECHA_DEMONS[rng.below(MECHA_DEMONS.len())];
        super::give_minion(state, id);
    }
}
