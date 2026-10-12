//! `Lurking Lionfish` (`BG36_201`) — Tier 2 Beast (`3/4`).
//! **Activate (2):** Choose a card in the Tavern. Replace it with a (`Golden` if Golden) `Fishbait` for your left-most Beast to attack.
//!
//! The Beast's attack is a regular attack (`docs/tavern.md` §7.7): its Rally and the attack
//! observers trigger, and its damage counts (`Treasure Parrot`). A Fishbait it kills gives it
//! +5/+5 (+10/+10 if Golden); one it doesn't kill stays in the Tavern.

use crate::cards::{tokens, ActivateTargetKind, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 217;
pub const NAME: &str = "Lurking Lionfish";
pub const ACTIVATE_COST: u32 = 2;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 4, 2)
        .with_tribe(Tribe::Beast)
        .with_activate_cost(ACTIVATE_COST)
        .with_activate_target(ActivateTargetKind::ShopCard)
        .on_activate(|state, source_pos, target_pos, pool, rng| {
            on_activate(state, source_pos, target_pos, pool, rng)
        })
}

pub fn on_activate(
    state: &mut TavernState,
    source_pos: usize,
    target_pos: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(shop_idx) = target_pos else {
        return;
    };
    if shop_idx >= state.shop.len() {
        return;
    }
    let is_golden = state.board[source_pos].is_golden;
    let old = std::mem::replace(&mut state.shop[shop_idx], tokens::make_fishbait(is_golden));
    if !old.is_spell {
        pool.return_unit(&old);
    }
    tokens::left_most_beast_attacks_fishbait(state, shop_idx, pool, rng);
}
