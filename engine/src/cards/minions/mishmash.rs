//! `Mishmash` (`TB_BaconShop_HERO_33_Buddy`) — Tier 3 All (`4/4`), in no pool.
//! Whenever your Amalgam gains stats, this gains them too (`twice` if Golden).
//!
//! Only `Curator Sticker` gives it (`docs/trinkets.md` §11.9). An `after_action` observer
//! **(engine decision)**: after each Tavern action it compares the friendly `Amalgam`s on the
//! board with what it saw after the last one, and gains the Attack and Health they gained since
//! (losses are not gains). While their number changes (an Amalgam summoned, sold or Magnetized
//! onto), it only takes note. It watches in the Tavern only: an Amalgam's gains in combat are
//! lost after it anyway. What it saw is kept on it (`counter`: their Attack, `stacks`: their
//! Health, `charges`: their number plus one; 0 before its first look).

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10808;
pub const NAME: &str = "Mishmash";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 4, 3)
        .with_tribe(Tribe::All)
        .on_after_action(after_action)
}

pub fn after_action(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let (mut attack, mut health, mut count) = (0i32, 0i32, 0u32);
    for amalgam in state
        .board
        .iter()
        .filter(|u| u.card_id == tokens::TOKEN_AMALGAM)
    {
        attack = attack.saturating_add(amalgam.attack);
        health = health.saturating_add(amalgam.health);
        count += 1;
    }
    let mishmash = &mut state.board[self_idx];
    if mishmash.charges == count + 1 {
        let mult = mishmash.golden_mult();
        let gained_attack = attack.saturating_sub(mishmash.counter).max(0);
        let gained_health = health.saturating_sub(mishmash.stacks).max(0);
        if gained_attack > 0 || gained_health > 0 {
            mishmash.add_stats(
                gained_attack.saturating_mul(mult),
                gained_health.saturating_mul(mult),
            );
        }
    }
    mishmash.counter = attack;
    mishmash.stacks = health;
    mishmash.charges = count + 1;
}
