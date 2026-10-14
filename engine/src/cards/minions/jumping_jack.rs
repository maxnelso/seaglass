//! `Jumping Jack` (`BGDUO_115`) — Tier 3 All (`3/4`), Duos only.
//! After the first time this is sold, Pass it (Golden `BGDUO_115_G`, `6/8`: the same).
//!
//! Its `sell` hook: sold, it gives its Gold and its sell triggers as usual, then is Passed with
//! its stats instead of going back to the pool, a free Pass that counts as the team's
//! (`docs/duos_tavern.md` §8.3). Its `counter` remembers it was sold: sold again, it goes back to
//! the pool. Made Golden (`Golden Touch`, a triple), it forgets.

use crate::cards::CardTemplate;
use crate::model::{CardId, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10303;
pub const NAME: &str = "Jumping Jack";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 4, 3)
        .with_tribe(Tribe::All)
        .on_sell(on_sell)
        .on_made_golden(|jack| jack.counter = 0)
        .on_merge_golden(|_, golden| golden.counter = 0)
}

/// Sold for the first time: take it back out of the pool and Pass it, remembering the sale.
pub fn on_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, _: &mut Rng) {
    if sold.counter != 0 {
        return;
    }
    pool.take_unit(sold);
    let mut jack = sold.clone();
    jack.counter = 1;
    state.auras.give_teammate(TeammateGift::Pass(jack));
}
