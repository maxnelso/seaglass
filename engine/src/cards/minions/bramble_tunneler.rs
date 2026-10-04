//! `Bramble Tunneler` (`BG36_331`) — Tier 4 Quilboar (`3/6`).
//!
//! Rally: Get a (`2` if Golden) random Choose One card(s).

use crate::cards::{minions, spells, tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;

pub const ID: CardId = 408;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Bramble Tunneler", 3, 6, 4)
        .with_tribe(Tribe::Quilboar)
        .on_rally(|c| {
            on_rally(&c.board[c.attacker_pos], c.generated_hand, c.rng);
            Vec::new()
        })
}

pub fn draw_random_choose_one_card(rng: &mut Rng) -> Unit {
    let mut pool: Vec<Unit> = vec![
        minions::crater_miner::template().instantiate(),
        minions::intrepid_botanist::template().instantiate(),
        minions::fearless_foodie::template().instantiate(),
        minions::sly_infiltrator::template().instantiate(),
        minions::sprightly_scarab::template().instantiate(),
        minions::snare_trapper::template().instantiate(),
        tokens::make_gem_day(),
    ];
    for s in spells::spells_up_to_tier(4) {
        if matches!(
            s.card_id,
            spells::SPELL_ALLIANCE_FLAG
                | spells::SPELL_TIME_MANAGEMENT
                | spells::SPELL_BOUNDLESS_POTENTIAL
        ) {
            pool.push(s);
        }
    }
    let idx = rng.below(pool.len());
    pool.swap_remove(idx)
}

pub fn on_rally(attacker: &Unit, generated_hand: &mut Vec<Unit>, rng: &mut Rng) {
    let count = if attacker.is_golden { 2 } else { 1 };
    for _ in 0..count {
        generated_hand.push(draw_random_choose_one_card(rng));
    }
}
