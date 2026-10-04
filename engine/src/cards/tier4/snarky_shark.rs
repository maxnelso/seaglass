//! `Snarky Shark` (`BG36_206`) — Tier 4 Beast (`4/5`).
//!
//! When you sell this, Refresh the Tavern with a (`Golden` if Golden) Fishbait.
//! Your left-most Beast attacks it.

use crate::cards::{self, tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 454;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Snarky Shark", 4, 5, 4).with_tribe(Tribe::Beast)
}

pub fn on_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    state.refresh_shop_free(pool, rng);
    let fishbait = tokens::make_fishbait(sold.is_golden);
    let fish_hp = fishbait.health;
    let fish_buff = if sold.is_golden { 10 } else { 5 };
    if let Some(beast_pos) = state
        .board
        .iter()
        .position(|u| u.tribe.matches(Tribe::Beast))
    {
        cards::trigger_tavern_rally(state, beast_pos, rng);
        let beast_atk = state.board[beast_pos].attack;
        if beast_atk >= fish_hp {
            state.board[beast_pos].add_stats(fish_buff, fish_buff);
        } else {
            let mut rem = fishbait;
            rem.health -= beast_atk.max(0);
            state.shop.push(rem);
        }
    } else {
        state.shop.push(fishbait);
    }
}
