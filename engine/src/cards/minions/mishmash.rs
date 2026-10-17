//! `Mishmash` (`TB_BaconShop_HERO_33_Buddy`) — Tier 3 All (`4/4`), in no pool.
//! Whenever your Amalgam gains stats, this gains them too (`twice` if Golden).
//!
//! Only `Curator Sticker` gives it (`docs/trinkets.md` §11.9). It looks at the friendly living
//! `Amalgam`s on the board **(engine decision)**: in the Tavern after each action (`after_action`),
//! in combat at each combat checkpoint (`combat_checkpoint`: before and after Start of Combat, and
//! after each attack turn), and gains the Attack and Health they gained since its last look
//! (losses are not gains). While their number changes (an Amalgam summoned, sold, killed), it
//! only takes note. It grows in combat too **(confirmed)**, for that combat. What it saw is kept on
//! it (`counter`: their Attack, `stacks`: their Health, `charges`: their number plus one; 0 before
//! its first look).

use crate::cards::{tokens, BoardCtx, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10808;
pub const NAME: &str = "Mishmash";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 4, 3)
        .with_tribe(Tribe::All)
        .on_after_action(after_action)
        .on_combat_checkpoint(combat_checkpoint)
}

pub fn after_action(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let (attack, health) = look(&mut state.board, self_idx);
    if attack > 0 || health > 0 {
        state.board[self_idx].add_stats(attack, health);
    }
}

pub fn combat_checkpoint(ctx: &mut BoardCtx<'_>, self_idx: usize) {
    let (attack, health) = look(ctx.board, self_idx);
    if attack > 0 || health > 0 {
        ctx.buff_unit(self_idx, attack, health, NAME);
    }
}

/// Look at the friendly living Amalgams on `board`: what `board[self_idx]` gains since its last
/// look (doubled if Golden), and remember what it saw.
fn look(board: &mut [Unit], self_idx: usize) -> (i32, i32) {
    let (mut attack, mut health, mut count) = (0i32, 0i32, 0u32);
    for amalgam in board
        .iter()
        .filter(|u| u.card_id == tokens::TOKEN_AMALGAM && u.health > 0)
    {
        attack = attack.saturating_add(amalgam.attack);
        health = health.saturating_add(amalgam.health);
        count += 1;
    }
    let mishmash = &mut board[self_idx];
    let mut gained = (0, 0);
    if mishmash.charges == count + 1 {
        let mult = mishmash.golden_mult();
        gained = (
            attack
                .saturating_sub(mishmash.counter)
                .max(0)
                .saturating_mul(mult),
            health
                .saturating_sub(mishmash.stacks)
                .max(0)
                .saturating_mul(mult),
        );
    }
    mishmash.counter = attack;
    mishmash.stacks = health;
    mishmash.charges = count + 1;
    gained
}
