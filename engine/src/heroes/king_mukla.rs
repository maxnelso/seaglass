//! `King Mukla` (`TB_BaconShop_HERO_38`) — Armor 16 (Duos 14).
//!
//! Hero Power: `Bananarama` (`TB_BaconShop_HP_038`), 0 Gold: At the start of your turn, get 2 Bananas and give everyone else one.
//!
//! Passive (`hero_turn_start`): at the start of each turn, rolls 2 Bananas for Mukla's hand (each
//! independently `Big Banana` `+2/+2` when `rng.below(2) == 0`, else `Bananas` `+1/+1`) and rolls
//! 1 Banana into `state.lobby_gifts` for each living opponent in `state.lobby.opponents`.

use crate::cards::hooks::SpellTarget;
use crate::cards::{tokens, CardFlags, CardHooks};
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 59814;
pub const POWER: CardId = 59815;

/// The Hero Power's hooks (and `Bananas` / `Big Banana`'s), for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (POWER, CardHooks::EMPTY.on_hero_turn_start(bananarama)),
        (
            tokens::SPELL_BANANAS,
            CardHooks::EMPTY
                .with_flags(CardFlags::NOT_IN_POOL | CardFlags::NOT_TAVERN_SPELL)
                .with_spell_target(SpellTarget::Minion)
                .on_cast_at(|state, _, target, _, _| {
                    if let Some(u) = state.minion_mut(target) {
                        u.add_stats(1, 1);
                    }
                }),
        ),
        (
            tokens::SPELL_BIG_BANANA,
            CardHooks::EMPTY
                .with_flags(CardFlags::NOT_IN_POOL | CardFlags::NOT_TAVERN_SPELL)
                .with_spell_target(SpellTarget::Minion)
                .on_cast_at(|state, _, target, _, _| {
                    if let Some(u) = state.minion_mut(target) {
                        u.add_stats(2, 2);
                    }
                }),
        ),
    ]
}

fn roll_banana(rng: &mut Rng) -> Unit {
    if rng.below(2) == 0 {
        tokens::make_big_banana()
    } else {
        tokens::make_bananas()
    }
}

/// Start of turn: get 2 Bananas and queue 1 Banana in `lobby_gifts` per living opponent.
fn bananarama(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    for _ in 0..2 {
        let banana = roll_banana(rng);
        state.add_to_hand(banana);
    }
    let living_opponents = state.lobby.opponents.iter().filter(|o| o.alive).count();
    for _ in 0..living_opponents {
        let banana = roll_banana(rng);
        state.lobby_gifts.push(banana);
    }
}
