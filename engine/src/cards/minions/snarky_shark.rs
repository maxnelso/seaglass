//! `Snarky Shark` (`BG36_206`) — Tier 4 Beast (`4/5`).
//!
//! When you sell this, Refresh the Tavern with a (`Golden` if Golden) Fishbait.
//! Your left-most Beast attacks it.
//!
//! The Fishbait joins the refreshed Tavern, and the Beast's attack is a regular attack
//! (`docs/tavern.md` §7.7), as for `Lurking Lionfish`.

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 454;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Snarky Shark", 4, 5, 4)
        .with_tribe(Tribe::Beast)
        .on_sell(on_sell)
}

pub fn on_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    state.refresh_shop_free(pool, rng);
    state.shop.push(tokens::make_fishbait(sold.is_golden));
    let bait_idx = state.shop.len() - 1;
    tokens::left_most_beast_attacks_fishbait(state, bait_idx, pool, rng);
}
