//! `Selfless Sightseer` (`BGDUO31_205`) — Tier 5 Dragon (`6/2`), Duos only.
//! **Battlecry:** Increase your team's maximum Gold by (`1`) (Golden `BGDUO31_205_G`, `12/4`:
//! by (`2`)).
//!
//! For both players, as `Strike Oil`: `max_gold` and `base_max_gold_bonus` go up (above 10 too),
//! with no Gold this turn. Its player's at once, the teammate's through an `Effect` gift
//! (`docs/duos_tavern.md` §8.4).

use crate::cards::CardTemplate;
use crate::model::{CardId, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 10503;
pub const NAME: &str = "Selfless Sightseer";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 6, 2, 5)
        .with_tribe(Tribe::Dragon)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
        .on_teammate_gift(teammate_gift)
}

/// Both players' maximum Gold: `+1` (`+2` if Golden).
pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let mult = unit.golden_mult();
    raise_max_gold(state, mult);
    state.auras.give_teammate(TeammateGift::Effect {
        card_id: ID,
        amount: mult,
        team: false,
    });
}

/// The teammate's Tavern: maximum Gold `+amount`.
pub fn teammate_gift(state: &mut TavernState, amount: i32, _: &mut Rng) -> Vec<Unit> {
    raise_max_gold(state, amount);
    Vec::new()
}

/// Maximum Gold `+n` for the rest of the game.
fn raise_max_gold(state: &mut TavernState, n: i32) {
    let n = n.max(0) as u32;
    state.auras.base_max_gold_bonus += n;
    state.max_gold += n;
}
