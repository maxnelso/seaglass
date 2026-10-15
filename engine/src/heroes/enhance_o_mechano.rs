//! `Enhance-o Mechano` (`BG24_HERO_204`) — Armor 14 (Duos 14).
//!
//! Hero Power: `Enhancification` (`BG24_HERO_204p`), 0 Gold: After the Tavern is Refreshed, give a random minion in it a random Bonus Keyword, twice.
//!
//! Passive. After each Refresh (`hero_after_refresh`), twice: a random minion in the shop that
//! lacks one of the Bonus Keywords (Taunt, Divine Shield, Windfury, Venomous, Reborn: engine
//! decision) gains one it lacks, at random (one `rng.below(k)` for the minion, one for the
//! keyword); the two can be the same minion.

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId, Keyword, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 96870;
pub const POWER: CardId = 96872;

/// The Bonus Keywords it gives.
const BONUS_KEYWORDS: [Keyword; 5] = [
    Keyword::Taunt,
    Keyword::DivineShield,
    Keyword::Windfury,
    Keyword::Venomous,
    Keyword::Reborn,
];

/// Times it gives one.
const TIMES: usize = 2;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_after_refresh(enhancify))]
}

/// `true` if `unit` has `keyword`.
fn has(unit: &Unit, keyword: Keyword) -> bool {
    match keyword {
        Keyword::Taunt => unit.taunt,
        Keyword::DivineShield => unit.divine_shield,
        Keyword::Windfury => unit.windfury,
        Keyword::Venomous => unit.venomous,
        Keyword::Reborn => unit.reborn,
        _ => true,
    }
}

/// The Bonus Keywords `unit` lacks.
fn lacking(unit: &Unit) -> Vec<Keyword> {
    BONUS_KEYWORDS
        .into_iter()
        .filter(|&k| !has(unit, k))
        .collect()
}

/// After a Refresh: twice, a random minion gains a random Bonus Keyword.
fn enhancify(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    for _ in 0..TIMES {
        let minions: Vec<usize> = (0..state.shop.len())
            .filter(|&i| !state.shop[i].is_spell && !lacking(&state.shop[i]).is_empty())
            .collect();
        let pick = match minions.len() {
            0 => return,
            1 => minions[0],
            k => minions[rng.below(k)],
        };
        let keywords = lacking(&state.shop[pick]);
        let keyword = match keywords.len() {
            1 => keywords[0],
            k => keywords[rng.below(k)],
        };
        state.shop[pick].apply_keyword(keyword, false);
    }
}
